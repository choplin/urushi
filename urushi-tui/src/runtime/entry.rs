//! Public construction and terminal ownership for the runtime.

use std::fmt;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use urushi_terminal::{
    Command, CommandWriter, Event, EventSource, KeyboardEnhancementFlags, KeyboardEnhancementQuery,
    Position, RawModeControl, SessionOptions, TerminalBackend, TerminalBackground,
    TerminalCapabilities, TerminalOutput, TerminalQuery, TerminalSession, TerminalSize, WindowSize,
};

use super::application::Application;
use super::core::{RuntimeCore, RuntimeError};
use super::executor::{Clock, Executor, TokioClock, TokioExecutor};
use super::presentation::{BlockingPresentation, BlockingPresentationError};
use super::sources::RuntimeSourceSpawner;
use super::terminal_source::TerminalSourceSpawner;
use crate::Screen;

/// A failure to construct or drive a terminal application.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// A physical terminal operation failed.
    Terminal(io::Error),
    /// Runtime-owned execution or presentation machinery failed.
    Runtime(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(error) => write!(formatter, "terminal runtime failed: {error}"),
            Self::Runtime(error) => write!(formatter, "application runtime failed: {error}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Terminal(error) | Self::Runtime(error) => Some(error),
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Terminal(error)
    }
}

/// The marker used until a caller supplies a terminal backend.
#[doc(hidden)]
pub struct DefaultTerminal;

/// A blocking full-screen application runtime.
///
/// [`Runtime::new`] uses the production executor, clock, and TUI session
/// profile. With the `crossterm` feature it also supplies the production
/// terminal backend; a runtime-only build must provide one with
/// [`backend`](Runtime::backend). Builders replace those boundaries or
/// individual session choices before [`run`](Runtime::run) takes ownership and
/// blocks the calling thread.
pub struct Runtime<A, B = DefaultTerminal> {
    application: A,
    backend: B,
    executor: Option<Arc<dyn Executor>>,
    clock: Option<Arc<dyn Clock>>,
    session: SessionOptions,
}

impl<A> Runtime<A, DefaultTerminal> {
    /// Builds a runtime with production defaults.
    pub fn new(application: A) -> Self {
        Self {
            application,
            backend: DefaultTerminal,
            executor: None,
            clock: None,
            session: tui_session_options(),
        }
    }

    /// Runs with the default Crossterm backend.
    #[cfg(feature = "crossterm")]
    pub fn run(self) -> Result<A::Model, Error>
    where
        A: Application,
    {
        let Self {
            application,
            executor,
            clock,
            session,
            ..
        } = self;
        Runtime {
            application,
            backend: urushi_terminal::backend::crossterm::CrosstermBackend::new(io::stdout()),
            executor,
            clock,
            session,
        }
        .run()
    }
}

impl<A, B> Runtime<A, B> {
    /// Replaces the physical connection used for the session, input, and queries.
    pub fn backend<U>(self, backend: U) -> Runtime<A, U>
    where
        U: TerminalBackend + Send + 'static,
    {
        Runtime {
            application: self.application,
            backend,
            executor: self.executor,
            clock: self.clock,
            session: self.session,
        }
    }

    /// Replaces the executor used by effects and subscriptions.
    pub fn executor(mut self, executor: impl Executor) -> Self {
        self.executor = Some(Arc::new(executor));
        self
    }

    /// Replaces the clock used by effects and frame scheduling.
    pub fn clock(mut self, clock: impl Clock) -> Self {
        self.clock = Some(Arc::new(clock));
        self
    }

    /// Enables or disables raw input mode for the session.
    pub const fn raw_mode(mut self, enabled: bool) -> Self {
        self.session.raw_mode = enabled;
        self
    }

    /// Enables or disables the alternate screen for the session.
    pub const fn alternate_screen(mut self, enabled: bool) -> Self {
        self.session.alternate_screen = enabled;
        self
    }

    /// Enables or disables bracketed-paste reporting.
    pub const fn bracketed_paste(mut self, enabled: bool) -> Self {
        self.session.bracketed_paste = enabled;
        self
    }

    /// Enables or disables focus-change reporting.
    pub const fn focus_change(mut self, enabled: bool) -> Self {
        self.session.focus_change = enabled;
        self
    }

    /// Selects the enhanced-keyboard information requested when supported.
    pub const fn keyboard_enhancement(mut self, flags: Option<KeyboardEnhancementFlags>) -> Self {
        self.session.keyboard_enhancement = flags;
        self
    }

    /// Enables or disables mouse capture.
    pub const fn mouse(mut self, enabled: bool) -> Self {
        self.session.mouse_capture = enabled;
        self
    }

    /// Hides the cursor until a frame places it or the session restores it.
    pub const fn hide_cursor(mut self, enabled: bool) -> Self {
        self.session.hide_cursor = enabled;
        self
    }
}

impl<A, B> Runtime<A, B>
where
    A: Application,
    B: TerminalBackend + Send + 'static,
{
    /// Drives the application on the calling thread and returns its final model.
    pub fn run(self) -> Result<A::Model, Error> {
        run_runtime(
            self.application,
            self.backend,
            self.executor,
            self.clock,
            self.session,
            |shared, initial_size, sources| {
                let screen = Screen::new(shared, initial_size).map_err(Error::Terminal)?;
                Ok(sized_presentation(screen, initial_size, sources))
            },
        )
    }
}

fn run_runtime<A, B>(
    application: A,
    backend: B,
    executor: Option<Arc<dyn Executor>>,
    clock: Option<Arc<dyn Clock>>,
    session_options: SessionOptions,
    build_presentation: impl FnOnce(
        SharedTerminal<B>,
        TerminalSize,
        Arc<TerminalSourceSpawner<SharedTerminal<B>, A::Message>>,
    ) -> Result<BlockingPresentation, Error>,
) -> Result<A::Model, Error>
where
    A: Application,
    B: TerminalBackend + Send + 'static,
{
    let tokio = BackgroundRuntime::new()?;
    let executor = executor.unwrap_or_else(|| {
        Arc::new(TokioExecutor::new(tokio.handle().clone())) as Arc<dyn Executor>
    });
    let clock = clock.unwrap_or_else(|| Arc::new(TokioClock) as Arc<dyn Clock>);
    let shared = SharedTerminal::new(backend);
    let mut session_control = shared.clone();
    let mut session = TerminalSession::enter(&mut session_control, session_options)
        .map_err(|error| Error::Terminal(io::Error::other(error)))?;

    let initial_size = match session.control_mut().window_size() {
        Ok(size) => size.cells(),
        Err(error) => {
            return finish_with_restore(Err(Error::Terminal(error)), session.restore());
        }
    };
    let runtime_result = tokio.block_on(async move {
        let fallback = Arc::new(RuntimeSourceSpawner::new(
            Arc::clone(&executor),
            Arc::clone(&clock),
        ));
        let sources = Arc::new(
            TerminalSourceSpawner::new(shared.clone(), Arc::clone(&executor), fallback)
                .map_err(Error::Terminal)?,
        );
        let presentation = build_presentation(shared, initial_size, Arc::clone(&sources))?;
        let core = RuntimeCore::new(application, executor, clock, sources, presentation)
            .map_err(runtime_error)?;
        core.run().await.map_err(runtime_error)
    });

    finish_with_restore(runtime_result, session.restore())
}

struct BackgroundRuntime(Option<tokio::runtime::Runtime>);

impl BackgroundRuntime {
    fn new() -> Result<Self, Error> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map(|runtime| Self(Some(runtime)))
            .map_err(Error::Runtime)
    }

    fn handle(&self) -> &tokio::runtime::Handle {
        self.0.as_ref().expect("runtime is live").handle()
    }

    fn block_on<F: std::future::Future>(&self, future: F) -> F::Output {
        self.0.as_ref().expect("runtime is live").block_on(future)
    }
}

impl Drop for BackgroundRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.0.take() {
            runtime.shutdown_background();
        }
    }
}

fn sized_presentation<W, B, Message>(
    screen: Screen<W>,
    initial_size: TerminalSize,
    sources: Arc<TerminalSourceSpawner<SharedTerminal<B>, Message>>,
) -> BlockingPresentation
where
    W: CommandWriter + Send + 'static,
    B: TerminalBackend + Send + 'static,
    Message: Send + 'static,
{
    BlockingPresentation::spawn_sized_terminal(screen, move || {
        sources.presentation_size(initial_size)
    })
}

/// Runs an application with the production defaults.
#[cfg(feature = "crossterm")]
pub fn run<A>(application: A) -> Result<A::Model, Error>
where
    A: Application,
{
    Runtime::new(application).run()
}

fn runtime_error(error: RuntimeError<BlockingPresentationError>) -> Error {
    match error {
        RuntimeError::Terminal(error) => Error::Terminal(error),
        RuntimeError::Presentation(error) => Error::Runtime(io::Error::other(error)),
    }
}

fn finish_with_restore<Model>(
    runtime: Result<Model, Error>,
    restore: io::Result<()>,
) -> Result<Model, Error> {
    match (runtime, restore) {
        (Ok(model), Ok(())) => Ok(model),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(Error::Terminal(error)),
        (Err(runtime), Err(restore)) => Err(Error::Terminal(io::Error::other(format!(
            "{runtime}; terminal restoration also failed: {restore}"
        )))),
    }
}

const fn tui_session_options() -> SessionOptions {
    SessionOptions {
        raw_mode: true,
        alternate_screen: true,
        bracketed_paste: true,
        focus_change: true,
        keyboard_enhancement: Some(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                .union(KeyboardEnhancementFlags::REPORT_EVENT_TYPES),
        ),
        mouse_capture: false,
        hide_cursor: true,
    }
}

struct SharedTerminal<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> Clone for SharedTerminal<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> SharedTerminal<T> {
    fn new(terminal: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(terminal)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, T> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl<T: TerminalOutput> TerminalOutput for SharedTerminal<T> {
    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
}

impl<T: CommandWriter> CommandWriter for SharedTerminal<T> {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.lock().write_command(command)
    }
}

impl<T: EventSource> EventSource for SharedTerminal<T> {
    fn read_event(&mut self) -> io::Result<Event> {
        self.lock().read_event()
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.lock().poll_event()
    }

    fn poll_event_timeout(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        let event = self.lock().poll_event()?;
        if event.is_none() {
            // Presentation shares this physical connection. Wait outside the
            // mutex so an idle input source cannot starve frame output.
            std::thread::sleep(timeout);
        }
        Ok(event)
    }
}

impl<T: RawModeControl> RawModeControl for SharedTerminal<T> {
    fn is_interactive(&self) -> bool {
        self.lock().is_interactive()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        self.lock().enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        self.lock().disable_raw_mode()
    }
}

impl<T: TerminalQuery> TerminalQuery for SharedTerminal<T> {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        self.lock().terminal_size()
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        self.lock().cursor_position()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.lock().window_size()
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        self.lock().raw_mode_enabled()
    }

    fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
        self.lock().terminal_capabilities()
    }

    fn terminal_background(&mut self) -> io::Result<Option<TerminalBackground>> {
        self.lock().terminal_background()
    }
}

impl<T: KeyboardEnhancementQuery> KeyboardEnhancementQuery for SharedTerminal<T> {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        self.lock().supports_keyboard_enhancement()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Condvar;
    use std::time::Duration;

    use urushi::{TextStyle, View};
    use urushi_terminal::{KeyCode, KeyEvent};

    use super::*;
    use crate::{Effect, Input, Subscription, Surface};

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[derive(Default)]
    struct ObservedTerminal {
        events: VecDeque<Event>,
        event_delay_polls: usize,
        raw: bool,
        alternate_screen: bool,
        bracketed_paste: bool,
        focus_change: bool,
        keyboard_enhancement: bool,
        cursor_visible: bool,
        flushes: usize,
        fail_window_query: bool,
    }

    struct FakeTerminal {
        observed: Arc<Mutex<ObservedTerminal>>,
        size: TerminalSize,
    }

    impl TerminalOutput for FakeTerminal {
        fn flush(&mut self) -> io::Result<()> {
            lock(&self.observed).flushes += 1;
            Ok(())
        }
    }

    impl CommandWriter for FakeTerminal {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            let mut observed = lock(&self.observed);
            match command {
                Command::SetAlternateScreen(enabled) => observed.alternate_screen = enabled,
                Command::SetBracketedPaste(enabled) => observed.bracketed_paste = enabled,
                Command::SetFocusReporting(enabled) => observed.focus_change = enabled,
                Command::PushKeyboardEnhancement(_) => observed.keyboard_enhancement = true,
                Command::PopKeyboardEnhancement => observed.keyboard_enhancement = false,
                Command::SetCursorVisible(visible) => observed.cursor_visible = visible,
                _ => {}
            }
            Ok(())
        }
    }

    impl EventSource for FakeTerminal {
        fn read_event(&mut self) -> io::Result<Event> {
            loop {
                if let Some(event) = self.poll_event()? {
                    return Ok(event);
                }
                std::thread::yield_now();
            }
        }

        fn poll_event(&mut self) -> io::Result<Option<Event>> {
            let mut observed = lock(&self.observed);
            if observed.event_delay_polls != 0 {
                observed.event_delay_polls -= 1;
                Ok(None)
            } else {
                Ok(observed.events.pop_front())
            }
        }

        fn poll_event_timeout(&mut self, _timeout: Duration) -> io::Result<Option<Event>> {
            let event = self.poll_event()?;
            if event.is_none() {
                std::thread::sleep(Duration::from_millis(1));
            }
            Ok(event)
        }
    }

    impl RawModeControl for FakeTerminal {
        fn is_interactive(&self) -> bool {
            true
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            lock(&self.observed).raw = true;
            Ok(())
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            lock(&self.observed).raw = false;
            Ok(())
        }
    }

    impl TerminalQuery for FakeTerminal {
        fn terminal_size(&mut self) -> io::Result<TerminalSize> {
            Ok(self.size)
        }

        fn cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            if lock(&self.observed).fail_window_query {
                return Err(io::Error::other("window query failed"));
            }
            Ok(WindowSize::new(self.size, None))
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(lock(&self.observed).raw)
        }
    }

    impl KeyboardEnhancementQuery for FakeTerminal {
        fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
            Ok(true)
        }
    }

    struct ExampleApplication;

    #[derive(Debug, Default, PartialEq, Eq)]
    struct Model {
        surface: Option<TerminalSize>,
        effect_completed: bool,
    }

    enum Message {
        Input(Input),
        Surface(Surface),
        EffectCompleted,
    }

    impl Application for ExampleApplication {
        type Model = Model;
        type Message = Message;

        fn init(&self) -> (Self::Model, Effect<Self::Message>) {
            (Model::default(), Effect::none())
        }

        fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
            match message {
                Message::Surface(surface) => {
                    model.surface = Some(surface.size);
                    Effect::none()
                }
                Message::Input(Input::Key(_)) => Effect::perform(|| Message::EffectCompleted),
                Message::Input(_) => Effect::none(),
                Message::EffectCompleted => {
                    model.effect_completed = true;
                    Effect::shutdown()
                }
            }
        }

        fn view(&self, model: &Self::Model) -> View {
            View::text(format!("done={}", model.effect_completed), TextStyle::new())
        }

        fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
            Subscription::batch([
                Subscription::surface(Message::Surface),
                Subscription::input(Message::Input),
            ])
        }
    }

    struct BlockingApplication {
        started: std::sync::mpsc::Sender<()>,
        exited: std::sync::mpsc::Sender<()>,
        release: Arc<(Mutex<bool>, Condvar)>,
    }

    enum BlockingMessage {
        Input,
        Finished,
    }

    impl Application for BlockingApplication {
        type Model = ();
        type Message = BlockingMessage;

        fn init(&self) -> (Self::Model, Effect<Self::Message>) {
            let started = self.started.clone();
            let exited = self.exited.clone();
            let release = Arc::clone(&self.release);
            let effect = Effect::perform(move || {
                started.send(()).expect("test waits for effect start");
                let (released, wake) = &*release;
                let mut released = lock(released);
                while !*released {
                    released = wake
                        .wait(released)
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                }
                exited.send(()).expect("test waits for effect exit");
                BlockingMessage::Finished
            });
            ((), effect)
        }

        fn update(
            &self,
            _model: &mut Self::Model,
            message: Self::Message,
        ) -> Effect<Self::Message> {
            match message {
                BlockingMessage::Input => Effect::shutdown(),
                BlockingMessage::Finished => Effect::none(),
            }
        }

        fn view(&self, _model: &Self::Model) -> View {
            View::text("blocking", TextStyle::new())
        }

        fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
            Subscription::input(|_| BlockingMessage::Input)
        }
    }

    #[test]
    fn public_runtime_wires_surface_input_effect_and_session_restoration() {
        let observed = Arc::new(Mutex::new(ObservedTerminal {
            events: VecDeque::from([Event::Key(KeyEvent::new(KeyCode::Enter))]),
            event_delay_polls: 0,
            cursor_visible: true,
            ..ObservedTerminal::default()
        }));
        let size = TerminalSize::new(8, 2);
        let terminal = FakeTerminal {
            observed: Arc::clone(&observed),
            size,
        };

        let model = Runtime::new(ExampleApplication)
            .backend(terminal)
            .run()
            .expect("runtime completes");

        assert_eq!(
            model,
            Model {
                surface: Some(size),
                effect_completed: true,
            }
        );
        let observed = lock(&observed);
        assert!(!observed.raw);
        assert!(!observed.alternate_screen);
        assert!(!observed.bracketed_paste);
        assert!(!observed.focus_change);
        assert!(!observed.keyboard_enhancement);
        assert!(observed.cursor_visible);
        assert!(observed.flushes >= 2);
    }

    #[test]
    fn shutdown_returns_while_started_blocking_work_finishes_in_the_background() {
        let observed = Arc::new(Mutex::new(ObservedTerminal {
            events: VecDeque::from([Event::Key(KeyEvent::new(KeyCode::Enter))]),
            event_delay_polls: 2,
            cursor_visible: true,
            ..ObservedTerminal::default()
        }));
        let backend = FakeTerminal {
            observed: Arc::clone(&observed),
            size: TerminalSize::new(8, 2),
        };
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (exited_tx, exited_rx) = std::sync::mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let application = BlockingApplication {
            started: started_tx,
            exited: exited_tx,
            release: Arc::clone(&release),
        };
        let (result_tx, result_rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = Runtime::new(application).backend(backend).run();
            result_tx.send(result).expect("test waits for run result");
        });

        started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("blocking effect starts");
        result_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("runtime returns without waiting for blocking effect")
            .expect("shutdown succeeds");
        assert!(!lock(&observed).raw);

        let (released, wake) = &*release;
        *lock(released) = true;
        wake.notify_all();
        exited_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("background effect can finish after runtime return");
    }

    #[test]
    fn setup_query_failure_restores_the_entered_session() {
        let observed = Arc::new(Mutex::new(ObservedTerminal {
            cursor_visible: true,
            fail_window_query: true,
            ..ObservedTerminal::default()
        }));
        let terminal = FakeTerminal {
            observed: Arc::clone(&observed),
            size: TerminalSize::new(8, 2),
        };

        let error = Runtime::new(ExampleApplication)
            .backend(terminal)
            .run()
            .expect_err("window query failure ends setup");

        assert!(error.to_string().contains("window query failed"));
        let observed = lock(&observed);
        assert!(!observed.raw);
        assert!(!observed.alternate_screen);
        assert!(!observed.bracketed_paste);
        assert!(!observed.focus_change);
        assert!(!observed.keyboard_enhancement);
        assert!(observed.cursor_visible);
    }
}
