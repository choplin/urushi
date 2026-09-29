//! The asynchronous boundary around synchronous physical presentation.

use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
#[cfg(feature = "graphics")]
use urushi::ResolvedView;
use urushi::View;
#[cfg(feature = "graphics")]
use urushi_graphics::GraphicsSelection;
#[cfg(feature = "graphics")]
use urushi_graphics::kitty::KittyLifecycle;
#[cfg(feature = "graphics")]
use urushi_graphics::sixel::SixelLifecycle;
use urushi_terminal::CommandWriter;
use urushi_tui::Screen;

use super::evaluator::Evaluator;
use super::renderer;
use super::source::Surface;

pub(crate) trait Presentation {
    type Error;

    fn submit(&mut self, view: View) -> Result<(), Self::Error>;

    /// Waits for the submitted draw.
    ///
    /// This future must be cancellation-safe because the runtime recreates it
    /// whenever another `select!` branch wins. With no draw submitted, it must
    /// remain pending rather than manufacture a completion.
    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>>;

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>>;
}

pub(crate) enum DrawResult {
    Completed,
    Failed { error: io::Error },
}

enum Command {
    Draw {
        view: Box<View>,
        surface: Option<Surface>,
    },
    Shutdown,
}

/// Runs one synchronous presenter on Tokio's blocking pool.
pub(crate) struct BlockingPresentation {
    commands: mpsc::Sender<Command>,
    results: mpsc::Receiver<DrawResult>,
    worker: Option<JoinHandle<io::Result<()>>>,
    frame_surface: Option<Box<dyn Fn() -> Surface + Send + Sync>>,
}

#[derive(Debug)]
pub(crate) enum BlockingPresentationError {
    Closed,
    Panicked(tokio::task::JoinError),
    Cleanup(io::Error),
}

impl fmt::Display for BlockingPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Closed => formatter.write_str("presentation worker closed"),
            Self::Panicked(error) => write!(formatter, "presentation worker failed: {error}"),
            Self::Cleanup(error) => {
                write!(formatter, "presentation cleanup failed: {error}")
            }
        }
    }
}

impl std::error::Error for BlockingPresentationError {}

impl BlockingPresentation {
    /// Spawns the ordinary stateless terminal presenter.
    pub(crate) fn spawn_terminal<W>(screen: Screen<W>) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_terminal_with(screen, Evaluator::default())
    }

    /// Spawns a terminal presenter that retains core evaluation across frames.
    pub(crate) fn spawn_retained_terminal<W>(screen: Screen<W>) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_terminal_with(screen, Evaluator::retained())
    }

    /// Spawns a presenter that snapshots its frame size when a draw is admitted.
    #[cfg(not(feature = "graphics"))]
    pub(crate) fn spawn_sized_terminal<W>(
        screen: Screen<W>,
        frame_surface: impl Fn() -> Surface + Send + Sync + 'static,
    ) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_sized_terminal_with(screen, Evaluator::default(), frame_surface)
    }

    fn spawn_terminal_with<W>(mut screen: Screen<W>, mut evaluator: Evaluator) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn(move |view| screen.draw(|frame| renderer::render(&view, frame, &mut evaluator)))
    }

    #[cfg(not(feature = "graphics"))]
    fn spawn_sized_terminal_with<W>(
        mut screen: Screen<W>,
        mut evaluator: Evaluator,
        frame_surface: impl Fn() -> Surface + Send + Sync + 'static,
    ) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_with(
            move |view, surface| {
                let surface = surface.expect("a sized presenter snapshots every draw");
                if screen.size() != surface.size {
                    screen.resize(surface.size)?;
                }
                screen.draw(|frame| renderer::render(&view, frame, &mut evaluator))
            },
            Some(Box::new(frame_surface)),
        )
    }

    pub(crate) fn spawn(mut present: impl FnMut(View) -> io::Result<()> + Send + 'static) -> Self {
        Self::spawn_with(move |view, _surface| present(view), None)
    }

    fn spawn_with(
        present: impl FnMut(View, Option<Surface>) -> io::Result<()> + Send + 'static,
        frame_surface: Option<Box<dyn Fn() -> Surface + Send + Sync>>,
    ) -> Self {
        Self::spawn_presenter(ClosurePresenter { present }, frame_surface)
    }

    fn spawn_presenter(
        mut presenter: impl SynchronousPresenter,
        frame_surface: Option<Box<dyn Fn() -> Surface + Send + Sync>>,
    ) -> Self {
        let (commands, mut command_rx) = mpsc::channel(1);
        let (result_tx, results) = mpsc::channel(1);
        let worker = tokio::task::spawn_blocking(move || {
            while let Some(command) = command_rx.blocking_recv() {
                match command {
                    Command::Draw { view, surface } => {
                        let result = match presenter.present(*view, surface) {
                            Ok(()) => DrawResult::Completed,
                            Err(error) => DrawResult::Failed { error },
                        };
                        if result_tx.blocking_send(result).is_err() {
                            break;
                        }
                    }
                    Command::Shutdown => return presenter.shutdown(),
                }
            }
            Ok(())
        });
        Self {
            commands,
            results,
            worker: Some(worker),
            frame_surface,
        }
    }
}

trait SynchronousPresenter: Send + 'static {
    fn present(&mut self, view: View, surface: Option<Surface>) -> io::Result<()>;

    fn shutdown(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ClosurePresenter<F> {
    present: F,
}

impl<F> SynchronousPresenter for ClosurePresenter<F>
where
    F: FnMut(View, Option<Surface>) -> io::Result<()> + Send + 'static,
{
    fn present(&mut self, view: View, surface: Option<Surface>) -> io::Result<()> {
        (self.present)(view, surface)
    }
}

#[cfg(feature = "graphics")]
pub(crate) fn spawn_graphics_terminal<W>(
    screen: Screen<W>,
    selection: GraphicsSelection,
    frame_surface: impl Fn() -> Surface + Send + Sync + 'static,
) -> BlockingPresentation
where
    W: CommandWriter + Send + 'static,
{
    BlockingPresentation::spawn_presenter(
        GraphicsPresenter {
            screen,
            evaluator: Evaluator::default(),
            graphics: GraphicsLifecycle::new(selection),
            enabled: selection != GraphicsSelection::Text,
            cleanup_pending: false,
        },
        Some(Box::new(frame_surface)),
    )
}

#[cfg(feature = "graphics")]
enum GraphicsLifecycle {
    Kitty(KittyLifecycle),
    Sixel(SixelLifecycle),
    Text,
}

#[cfg(feature = "graphics")]
impl GraphicsLifecycle {
    fn new(selection: GraphicsSelection) -> Self {
        match selection {
            GraphicsSelection::Kitty => Self::Kitty(KittyLifecycle::new()),
            GraphicsSelection::Sixel => Self::Sixel(SixelLifecycle::new()),
            GraphicsSelection::Text => Self::Text,
        }
    }

    fn selection(&self) -> GraphicsSelection {
        match self {
            Self::Kitty(_) => GraphicsSelection::Kitty,
            Self::Sixel(_) => GraphicsSelection::Sixel,
            Self::Text => GraphicsSelection::Text,
        }
    }

    fn present<W: CommandWriter>(
        &mut self,
        view: &View,
        resolved: &ResolvedView,
        surface: Surface,
        writer: &mut W,
    ) -> io::Result<()> {
        match self {
            Self::Kitty(lifecycle) => {
                lifecycle.present(view, resolved, surface.cell_pixels, writer)
            }
            Self::Sixel(lifecycle) => lifecycle.present(
                view,
                resolved,
                surface.cell_pixels.ok_or_else(|| {
                    io::Error::other(
                        "Sixel presentation lost uniform character-cell pixel geometry",
                    )
                })?,
                writer,
            ),
            Self::Text => Ok(()),
        }
    }

    fn clear<W: CommandWriter>(&mut self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Kitty(lifecycle) => lifecycle.clear(writer),
            Self::Sixel(lifecycle) => lifecycle.clear(writer),
            Self::Text => Ok(()),
        }
    }
}

#[cfg(feature = "graphics")]
struct GraphicsPresenter<W> {
    screen: Screen<W>,
    evaluator: Evaluator,
    graphics: GraphicsLifecycle,
    enabled: bool,
    cleanup_pending: bool,
}

#[cfg(feature = "graphics")]
impl<W> GraphicsPresenter<W>
where
    W: CommandWriter + Send + 'static,
{
    fn clear_graphics(&mut self) -> io::Result<()> {
        let graphics = &mut self.graphics;
        self.screen.modify_surface(|writer| graphics.clear(writer))
    }

    fn present_graphics(&mut self, view: &View, surface: Surface) -> io::Result<()> {
        if self.screen.size() != surface.size {
            if let Err(error) = self.clear_graphics() {
                self.screen.resize(surface.size)?;
                return self.recover(
                    view,
                    surface,
                    graphics_error(self.graphics.selection(), error),
                );
            }
            self.screen.resize(surface.size)?;
        }

        let resolved = renderer::resolve(view, surface.size, &mut self.evaluator);
        if matches!(self.graphics, GraphicsLifecycle::Sixel(_)) {
            self.screen.invalidate();
        }
        let graphics = &mut self.graphics;
        let selection = graphics.selection();
        let result = self.screen.draw_with(
            |frame| renderer::render_resolved(&resolved, frame),
            |writer| {
                graphics
                    .present(view, &resolved, surface, writer)
                    .map_err(|error| graphics_error(selection, error))
            },
        );
        match result {
            Err(error)
                if error.get_ref().is_some_and(|source| {
                    source.downcast_ref::<GraphicsOutputFailure>().is_some()
                }) =>
            {
                self.recover(view, surface, error)
            }
            result => result,
        }
    }

    fn recover(&mut self, view: &View, surface: Surface, original: io::Error) -> io::Result<()> {
        let cleanup = self.clear_graphics().err();
        self.enabled = false;
        self.cleanup_pending = cleanup.is_some();
        if self.screen.size() != surface.size {
            self.screen.resize(surface.size)?;
        }
        self.screen.invalidate();
        let resolved = renderer::resolve(view, surface.size, &mut self.evaluator);
        let fallback = self
            .screen
            .draw(|frame| renderer::render_resolved(&resolved, frame))
            .err();
        let kind = original.kind();
        Err(io::Error::new(
            kind,
            GraphicsRecoveryFailure {
                original,
                cleanup,
                fallback,
            },
        ))
    }
}

#[cfg(feature = "graphics")]
impl<W> SynchronousPresenter for GraphicsPresenter<W>
where
    W: CommandWriter + Send + 'static,
{
    fn present(&mut self, view: View, surface: Option<Surface>) -> io::Result<()> {
        let surface = surface.expect("a graphics presenter snapshots every draw");
        if self.enabled {
            self.present_graphics(&view, surface)
        } else {
            let cleanup = if self.cleanup_pending {
                self.clear_graphics().err()
            } else {
                None
            };
            self.cleanup_pending = cleanup.is_some();
            if self.screen.size() != surface.size {
                self.screen.resize(surface.size)?;
            }
            let resolved = renderer::resolve(&view, surface.size, &mut self.evaluator);
            let fallback = self
                .screen
                .draw(|frame| renderer::render_resolved(&resolved, frame));
            match (cleanup, fallback) {
                (None, result) => result,
                (Some(cleanup), Ok(())) => Err(io::Error::new(
                    cleanup.kind(),
                    format!("graphics cleanup retry failed: {cleanup}"),
                )),
                (Some(cleanup), Err(fallback)) => Err(io::Error::new(
                    cleanup.kind(),
                    format!(
                        "graphics cleanup retry failed: {cleanup}; text fallback redraw also failed: {fallback}"
                    ),
                )),
            }
        }
    }

    fn shutdown(&mut self) -> io::Result<()> {
        self.clear_graphics()
    }
}

#[cfg(feature = "graphics")]
#[derive(Debug)]
struct GraphicsOutputFailure {
    selection: GraphicsSelection,
    source: io::Error,
}

#[cfg(feature = "graphics")]
impl fmt::Display for GraphicsOutputFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} graphics output failed: {}",
            self.selection, self.source
        )
    }
}

#[cfg(feature = "graphics")]
impl std::error::Error for GraphicsOutputFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

#[cfg(feature = "graphics")]
fn graphics_error(selection: GraphicsSelection, source: io::Error) -> io::Error {
    io::Error::new(source.kind(), GraphicsOutputFailure { selection, source })
}

#[cfg(feature = "graphics")]
#[derive(Debug)]
struct GraphicsRecoveryFailure {
    original: io::Error,
    cleanup: Option<io::Error>,
    fallback: Option<io::Error>,
}

#[cfg(feature = "graphics")]
impl fmt::Display for GraphicsRecoveryFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.original)?;
        if let Some(error) = &self.cleanup {
            write!(formatter, "; graphics cleanup also failed: {error}")?;
        }
        if let Some(error) = &self.fallback {
            write!(formatter, "; text fallback redraw also failed: {error}")?;
        }
        Ok(())
    }
}

#[cfg(feature = "graphics")]
impl std::error::Error for GraphicsRecoveryFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.original)
    }
}

impl Presentation for BlockingPresentation {
    type Error = BlockingPresentationError;

    fn submit(&mut self, view: View) -> Result<(), Self::Error> {
        let surface = self
            .frame_surface
            .as_ref()
            .map(|frame_surface| frame_surface());
        self.commands
            .try_send(Command::Draw {
                view: Box::new(view),
                surface,
            })
            .map_err(|_| BlockingPresentationError::Closed)
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(async {
            match self.results.recv().await {
                Some(result) => Ok(result),
                None => Err(BlockingPresentationError::Closed),
            }
        })
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        Box::pin(async {
            let _ = self.commands.send(Command::Shutdown).await;
            let Some(worker) = self.worker.take() else {
                return Ok(());
            };
            worker
                .await
                .map_err(BlockingPresentationError::Panicked)?
                .map_err(BlockingPresentationError::Cleanup)
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use urushi::{
        BlockStyle, Canvas, CanvasContext, CanvasItem, Length, Position, Projection,
        ProjectionBoundary, Size, TextStyle, Viewport,
    };
    use urushi_terminal::{Command, TerminalOutput, TerminalSize};

    #[cfg(feature = "graphics")]
    use urushi_graphics::{CellSize, Image, ImagePresentation, PixelSize};
    #[cfg(feature = "graphics")]
    use urushi_terminal::{ClearRegion, PixelSize as CellPixels};

    use super::*;

    struct NullWriter;

    impl TerminalOutput for NullWriter {
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl CommandWriter for NullWriter {
        fn write_command(&mut self, _command: Command<'_>) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn synchronous_presentation_runs_off_the_runtime_thread() {
        let runtime_thread = std::thread::current().id();
        let presentation_thread = Arc::new(Mutex::new(None));
        let observed = Arc::clone(&presentation_thread);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let mut presentation = BlockingPresentation::spawn(move |_view| {
                *observed.lock().unwrap() = Some(std::thread::current().id());
                Ok(())
            });
            presentation
                .submit(View::text("frame", TextStyle::new()))
                .unwrap();
            assert!(matches!(
                presentation.completed().await.unwrap(),
                DrawResult::Completed
            ));
            presentation.shutdown().await.unwrap();
        });

        assert_ne!(*presentation_thread.lock().unwrap(), Some(runtime_thread));
    }

    #[test]
    fn default_terminal_presentation_uses_stateless_evaluation() {
        let draws = Arc::new(AtomicUsize::new(0));
        let view = viewport_canvas(1, Arc::clone(&draws));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let screen = Screen::new(NullWriter, TerminalSize::new(2, 1)).unwrap();
            let mut presentation = BlockingPresentation::spawn_terminal(screen);
            for _ in 0..2 {
                presentation.submit(view.clone()).unwrap();
                assert!(matches!(
                    presentation.completed().await.unwrap(),
                    DrawResult::Completed
                ));
            }
            presentation.shutdown().await.unwrap();
        });

        assert_eq!(draws.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn retained_terminal_presentation_reuses_viewport_content_across_frames() {
        let draws = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&draws);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let screen = Screen::new(NullWriter, TerminalSize::new(2, 1)).unwrap();
            let mut presentation = BlockingPresentation::spawn_retained_terminal(screen);
            for origin in [0, 1, 0] {
                presentation
                    .submit(viewport_canvas(origin, Arc::clone(&observed)))
                    .unwrap();
                assert!(matches!(
                    presentation.completed().await.unwrap(),
                    DrawResult::Completed
                ));
            }
            presentation.shutdown().await.unwrap();
        });

        assert_eq!(draws.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "graphics")]
    #[derive(Debug, PartialEq, Eq)]
    enum GraphicsRecorded {
        Clear,
        Print(String),
        Kitty,
        Sixel,
        Flush,
    }

    #[cfg(feature = "graphics")]
    #[derive(Default)]
    struct RecordingGraphicsWriter {
        commands: Vec<GraphicsRecorded>,
        extension_failures: usize,
    }

    #[cfg(feature = "graphics")]
    impl RecordingGraphicsWriter {
        fn failing(extension_failures: usize) -> Self {
            Self {
                commands: Vec::new(),
                extension_failures,
            }
        }

        fn record_extension(&mut self, command: GraphicsRecorded) -> io::Result<()> {
            self.commands.push(command);
            if self.extension_failures > 0 {
                self.extension_failures -= 1;
                Err(io::Error::other("planned graphics failure"))
            } else {
                Ok(())
            }
        }
    }

    #[cfg(feature = "graphics")]
    impl TerminalOutput for RecordingGraphicsWriter {
        fn flush(&mut self) -> io::Result<()> {
            self.commands.push(GraphicsRecorded::Flush);
            Ok(())
        }
    }

    #[cfg(feature = "graphics")]
    impl CommandWriter for RecordingGraphicsWriter {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            match command {
                Command::Clear(ClearRegion::Screen) => {
                    self.commands.push(GraphicsRecorded::Clear);
                }
                Command::Print(text) => {
                    self.commands
                        .push(GraphicsRecorded::Print(text.as_str().to_owned()));
                }
                Command::ApplicationProgram(_) => {
                    return self.record_extension(GraphicsRecorded::Kitty);
                }
                Command::DeviceControl(_) => {
                    return self.record_extension(GraphicsRecorded::Sixel);
                }
                _ => {}
            }
            Ok(())
        }
    }

    #[cfg(feature = "graphics")]
    fn graphics_surface() -> Surface {
        Surface {
            size: TerminalSize::new(8, 1),
            cell_pixels: Some(CellPixels::new(8, 16)),
        }
    }

    #[cfg(feature = "graphics")]
    fn image_view(key: &'static str, fallback: &str) -> View {
        let image = Image::rgba(key, key, PixelSize::new(1, 1), [255, 0, 0, 255])
            .unwrap()
            .fallback(fallback);
        ImagePresentation::new().compose(&image, CellSize::new(8, 1))
    }

    #[cfg(feature = "graphics")]
    fn graphics_presenter(
        selection: GraphicsSelection,
        writer: RecordingGraphicsWriter,
    ) -> GraphicsPresenter<RecordingGraphicsWriter> {
        GraphicsPresenter {
            screen: Screen::new(writer, graphics_surface().size).unwrap(),
            evaluator: Evaluator::default(),
            graphics: GraphicsLifecycle::new(selection),
            enabled: selection != GraphicsSelection::Text,
            cleanup_pending: false,
        }
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn kitty_output_is_part_of_the_cell_commit_and_disappearing_images_are_deleted() {
        let mut presenter =
            graphics_presenter(GraphicsSelection::Kitty, RecordingGraphicsWriter::default());
        presenter
            .present(image_view("first", "fallback"), Some(graphics_surface()))
            .unwrap();
        let after_image = presenter.screen.writer().commands.len();

        presenter
            .present(
                View::text("text", TextStyle::new()),
                Some(graphics_surface()),
            )
            .unwrap();

        assert!(
            presenter.screen.writer().commands[..after_image]
                .iter()
                .any(|command| matches!(command, GraphicsRecorded::Kitty))
        );
        assert!(
            presenter.screen.writer().commands[after_image..]
                .iter()
                .any(|command| matches!(command, GraphicsRecorded::Kitty)),
            "the empty desired graphics scene deletes the stale Kitty image"
        );
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn sixel_frames_clear_then_redraw_cells_then_present_the_complete_scene() {
        let mut presenter =
            graphics_presenter(GraphicsSelection::Sixel, RecordingGraphicsWriter::default());
        for key in ["first", "second"] {
            presenter
                .present(image_view(key, "fallback"), Some(graphics_surface()))
                .unwrap();
        }

        let commands = &presenter.screen.writer().commands;
        let clears = commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                matches!(command, GraphicsRecorded::Clear).then_some(index)
            })
            .collect::<Vec<_>>();
        let sixels = commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                matches!(command, GraphicsRecorded::Sixel).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(clears.len(), 2);
        assert_eq!(sixels.len(), 2);
        for (clear, sixel) in clears.into_iter().zip(sixels) {
            assert!(clear < sixel);
            assert!(
                commands[clear..sixel]
                    .iter()
                    .any(|command| matches!(command, GraphicsRecorded::Print(_)))
            );
        }
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn sixel_geometry_loss_falls_back_to_text() {
        let mut presenter =
            graphics_presenter(GraphicsSelection::Sixel, RecordingGraphicsWriter::default());
        presenter
            .present(image_view("first", "fallback"), Some(graphics_surface()))
            .unwrap();
        let surface_without_pixels = Surface {
            size: graphics_surface().size,
            cell_pixels: None,
        };

        let error = presenter
            .present(
                image_view("first", "fallback"),
                Some(surface_without_pixels),
            )
            .expect_err("Sixel cannot continue without cell-pixel geometry");

        assert!(
            error
                .to_string()
                .contains("Sixel presentation lost uniform character-cell pixel geometry")
        );
        assert!(!presenter.enabled);
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn graphics_failure_cleans_up_commits_text_fallback_and_disables_graphics() {
        let mut presenter = graphics_presenter(
            GraphicsSelection::Kitty,
            RecordingGraphicsWriter::failing(1),
        );
        let error = presenter
            .present(image_view("failed", "fallback"), Some(graphics_surface()))
            .expect_err("the graphics failure is reported after fallback");

        assert!(error.to_string().contains("planned graphics failure"));
        assert!(!error.to_string().contains("cleanup also failed"));
        assert!(!presenter.enabled);
        let printed = presenter
            .screen
            .writer()
            .commands
            .iter()
            .filter_map(|command| match command {
                GraphicsRecorded::Print(text) => Some(text.as_str()),
                _ => None,
            })
            .collect::<String>();
        assert!(printed.contains("fallback"));
        let kitty_commands = presenter
            .screen
            .writer()
            .commands
            .iter()
            .filter(|command| matches!(command, GraphicsRecorded::Kitty))
            .count();

        presenter
            .present(image_view("later", "text-only"), Some(graphics_surface()))
            .unwrap();
        assert_eq!(
            presenter
                .screen
                .writer()
                .commands
                .iter()
                .filter(|command| matches!(command, GraphicsRecorded::Kitty))
                .count(),
            kitty_commands
        );
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn cleanup_failure_keeps_the_original_error_and_shutdown_retries_cleanup() {
        let mut presenter = graphics_presenter(
            GraphicsSelection::Kitty,
            RecordingGraphicsWriter::failing(2),
        );
        let error = presenter
            .present(image_view("failed", "fallback"), Some(graphics_surface()))
            .expect_err("graphics and cleanup failures are reported");

        let message = error.to_string();
        assert!(message.contains("Kitty graphics output failed: planned graphics failure"));
        assert!(message.contains("graphics cleanup also failed: planned graphics failure"));
        let before_shutdown = presenter.screen.writer().commands.len();
        presenter.shutdown().expect("shutdown retries cleanup");
        assert!(
            presenter.screen.writer().commands[before_shutdown..]
                .iter()
                .any(|command| matches!(command, GraphicsRecorded::Kitty))
        );
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn the_next_text_frame_retries_failed_graphics_cleanup() {
        let mut presenter = graphics_presenter(
            GraphicsSelection::Kitty,
            RecordingGraphicsWriter::failing(2),
        );
        presenter
            .present(image_view("failed", "fallback"), Some(graphics_surface()))
            .expect_err("initial output and cleanup fail");
        let before_retry = presenter.screen.writer().commands.len();

        presenter
            .present(
                View::text("recovered", TextStyle::new()),
                Some(graphics_surface()),
            )
            .expect("the next text frame retries cleanup and redraws");

        assert!(!presenter.cleanup_pending);
        assert!(
            presenter.screen.writer().commands[before_retry..]
                .iter()
                .any(|command| matches!(command, GraphicsRecorded::Kitty))
        );
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn successful_shutdown_removes_retained_kitty_state() {
        let mut presenter =
            graphics_presenter(GraphicsSelection::Kitty, RecordingGraphicsWriter::default());
        presenter
            .present(image_view("first", "fallback"), Some(graphics_surface()))
            .unwrap();
        let before_shutdown = presenter.screen.writer().commands.len();

        presenter.shutdown().unwrap();

        assert!(
            presenter.screen.writer().commands[before_shutdown..]
                .iter()
                .any(|command| matches!(command, GraphicsRecorded::Kitty))
        );
    }

    #[derive(Debug, Clone)]
    struct CountingText {
        draws: Arc<AtomicUsize>,
    }

    impl PartialEq for CountingText {
        fn eq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl CanvasItem for CountingText {
        fn draw(&self, context: &mut CanvasContext) {
            self.draws.fetch_add(1, Ordering::Relaxed);
            context.text(Position::new(0, 0), "abcd", TextStyle::new());
        }
    }

    fn viewport_canvas(origin: i64, draws: Arc<AtomicUsize>) -> View {
        View::viewport(
            Viewport::horizontal(Projection::new(origin, ProjectionBoundary::Preserve)),
            View::block(
                BlockStyle::new()
                    .width(Length::Cells(4))
                    .height(Length::Cells(1)),
                View::canvas(
                    Canvas::new()
                        .extent(Size::new(4, 1))
                        .item(CountingText { draws }),
                ),
            ),
        )
    }
}
