//! The asynchronous boundary around synchronous physical presentation.

use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use urushi::View;
use urushi_terminal::{CommandWriter, TerminalSize};

use super::evaluator::Evaluator;
use super::renderer;
use crate::Screen;

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
        size: Option<TerminalSize>,
    },
    Shutdown,
}

/// Runs one synchronous presenter on Tokio's blocking pool.
pub(crate) struct BlockingPresentation {
    commands: mpsc::Sender<Command>,
    results: mpsc::Receiver<DrawResult>,
    worker: Option<JoinHandle<()>>,
    frame_size: Option<Box<dyn Fn() -> TerminalSize + Send + Sync>>,
}

#[derive(Debug)]
pub(crate) enum BlockingPresentationError {
    WorkerClosed,
    WorkerPanicked(tokio::task::JoinError),
}

impl fmt::Display for BlockingPresentationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerClosed => formatter.write_str("presentation worker closed"),
            Self::WorkerPanicked(error) => write!(formatter, "presentation worker failed: {error}"),
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
    pub(crate) fn spawn_sized_terminal<W>(
        screen: Screen<W>,
        frame_size: impl Fn() -> TerminalSize + Send + Sync + 'static,
    ) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_sized_terminal_with(screen, Evaluator::default(), frame_size)
    }

    fn spawn_terminal_with<W>(mut screen: Screen<W>, mut evaluator: Evaluator) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn(move |view| screen.draw(|frame| renderer::render(&view, frame, &mut evaluator)))
    }

    fn spawn_sized_terminal_with<W>(
        mut screen: Screen<W>,
        mut evaluator: Evaluator,
        frame_size: impl Fn() -> TerminalSize + Send + Sync + 'static,
    ) -> Self
    where
        W: CommandWriter + Send + 'static,
    {
        Self::spawn_with(
            move |view, size| {
                let size = size.expect("a sized presenter snapshots every draw");
                if screen.size() != size {
                    screen.resize(size)?;
                }
                screen.draw(|frame| renderer::render(&view, frame, &mut evaluator))
            },
            Some(Box::new(frame_size)),
        )
    }

    pub(crate) fn spawn(mut present: impl FnMut(View) -> io::Result<()> + Send + 'static) -> Self {
        Self::spawn_with(move |view, _size| present(view), None)
    }

    fn spawn_with(
        mut present: impl FnMut(View, Option<TerminalSize>) -> io::Result<()> + Send + 'static,
        frame_size: Option<Box<dyn Fn() -> TerminalSize + Send + Sync>>,
    ) -> Self {
        let (commands, mut command_rx) = mpsc::channel(1);
        let (result_tx, results) = mpsc::channel(1);
        let worker = tokio::task::spawn_blocking(move || {
            while let Some(command) = command_rx.blocking_recv() {
                match command {
                    Command::Draw { view, size } => {
                        let result = match present(*view, size) {
                            Ok(()) => DrawResult::Completed,
                            Err(error) => DrawResult::Failed { error },
                        };
                        if result_tx.blocking_send(result).is_err() {
                            break;
                        }
                    }
                    Command::Shutdown => break,
                }
            }
        });
        Self {
            commands,
            results,
            worker: Some(worker),
            frame_size,
        }
    }
}

impl Presentation for BlockingPresentation {
    type Error = BlockingPresentationError;

    fn submit(&mut self, view: View) -> Result<(), Self::Error> {
        let size = self.frame_size.as_ref().map(|frame_size| frame_size());
        self.commands
            .try_send(Command::Draw {
                view: Box::new(view),
                size,
            })
            .map_err(|_| BlockingPresentationError::WorkerClosed)
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(async {
            match self.results.recv().await {
                Some(result) => Ok(result),
                None => Err(BlockingPresentationError::WorkerClosed),
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
                .map_err(BlockingPresentationError::WorkerPanicked)
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
