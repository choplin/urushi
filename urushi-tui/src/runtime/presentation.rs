//! The asynchronous boundary around synchronous physical presentation.

use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::time::Instant;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use urushi::{StyledGrapheme, View};

use super::evaluator::Evaluator;
use super::renderer;
use crate::terminal::Terminal;

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
    Completed {
        at: Instant,
    },
    Failed {
        error: io::Error,
        completed_at: Instant,
    },
}

enum Command {
    Draw(Box<View>),
    Shutdown,
}

/// Runs one synchronous presenter on Tokio's blocking pool.
pub(crate) struct BlockingPresentation {
    commands: mpsc::Sender<Command>,
    results: mpsc::Receiver<DrawResult>,
    worker: Option<JoinHandle<()>>,
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
    pub(crate) fn spawn_terminal<T>(terminal: T) -> Self
    where
        T: Terminal<Cell = StyledGrapheme> + Send + 'static,
    {
        Self::spawn_terminal_with(terminal, Evaluator::default())
    }

    /// Spawns a terminal presenter that retains core evaluation across frames.
    pub(crate) fn spawn_retained_terminal<T>(terminal: T) -> Self
    where
        T: Terminal<Cell = StyledGrapheme> + Send + 'static,
    {
        Self::spawn_terminal_with(terminal, Evaluator::retained())
    }

    fn spawn_terminal_with<T>(mut terminal: T, mut evaluator: Evaluator) -> Self
    where
        T: Terminal<Cell = StyledGrapheme> + Send + 'static,
    {
        Self::spawn(move |view| {
            terminal.draw(|frame| renderer::render(&view, frame, &mut evaluator))
        })
    }

    pub(crate) fn spawn(mut present: impl FnMut(View) -> io::Result<()> + Send + 'static) -> Self {
        let (commands, mut command_rx) = mpsc::channel(1);
        let (result_tx, results) = mpsc::channel(1);
        let worker = tokio::task::spawn_blocking(move || {
            while let Some(command) = command_rx.blocking_recv() {
                match command {
                    Command::Draw(view) => {
                        let result = match present(*view) {
                            Ok(()) => DrawResult::Completed { at: Instant::now() },
                            Err(error) => DrawResult::Failed {
                                error,
                                completed_at: Instant::now(),
                            },
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
        }
    }
}

impl Presentation for BlockingPresentation {
    type Error = BlockingPresentationError;

    fn submit(&mut self, view: View) -> Result<(), Self::Error> {
        self.commands
            .try_send(Command::Draw(Box::new(view)))
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
    use urushi_terminal::TerminalSize;

    use super::*;
    use crate::runtime::testing::InMemoryTerminal;

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
                DrawResult::Completed { .. }
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
            let terminal = InMemoryTerminal::new(TerminalSize::new(2, 1));
            let mut presentation = BlockingPresentation::spawn_terminal(terminal);
            for _ in 0..2 {
                presentation.submit(view.clone()).unwrap();
                assert!(matches!(
                    presentation.completed().await.unwrap(),
                    DrawResult::Completed { .. }
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
            let terminal = InMemoryTerminal::new(TerminalSize::new(2, 1));
            let mut presentation = BlockingPresentation::spawn_retained_terminal(terminal);
            for origin in [0, 1, 0] {
                presentation
                    .submit(viewport_canvas(origin, Arc::clone(&observed)))
                    .unwrap();
                assert!(matches!(
                    presentation.completed().await.unwrap(),
                    DrawResult::Completed { .. }
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
