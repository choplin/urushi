use std::collections::VecDeque;
use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use urushi::{TextStyle, View};
use urushi_terminal::{
    Event, EventSource, FocusChange, KeyCode, KeyEvent, KeyKind, MouseButton, MouseEvent,
    MouseKind, PixelSize, Position, TerminalQuery, TerminalSize, WindowSize,
};

use super::*;
use crate::application::Application;
use crate::core::{RuntimeCore, RuntimeError};
use crate::delivery::{Delivery, DeliveryQueue};
use crate::effect::Effect;
use crate::executor::{
    Clock, RunningSource, SourceSpawner, SubscriptionExecutor, TokioClock, TokioExecutor,
};
use crate::presentation::{DrawResult, Presentation};
use crate::subscription::{Source, Subscription};
use crate::testing::Harness;

#[test]
fn terminal_events_become_individual_async_inputs_in_read_order() {
    let (terminal, probe) = FakeTerminal::new(
        [
            Ok(Event::Key(
                KeyEvent::new(KeyCode::Down).with_kind(KeyKind::Repeat),
            )),
            Ok(Event::Paste("two".into())),
            Ok(Event::Focus(FocusChange::Lost)),
            Ok(Event::Mouse(MouseEvent {
                kind: MouseKind::Down(MouseButton::Left),
                position: Position::new(3, 4),
                modifiers: Default::default(),
            })),
        ],
        [],
    );
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(spawner, harness.deliveries());

    subscriptions
        .reconcile(Subscription::input(|input| input))
        .unwrap();
    probe.wait_for_polls(4);
    assert!(!harness.complete_effect(0));

    assert_eq!(
        harness.drain(),
        vec![
            Delivery::Async(Input::Key(
                KeyEvent::new(KeyCode::Down).with_kind(KeyKind::Repeat)
            )),
            Delivery::Async(Input::Paste("two".into())),
            Delivery::Async(Input::Focus(FocusChange::Lost)),
            Delivery::Async(Input::Mouse(MouseEvent {
                kind: MouseKind::Down(MouseButton::Left),
                position: Position::new(3, 4),
                modifiers: Default::default(),
            })),
        ]
    );
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn input_uses_the_mapper_snapshotted_when_the_event_is_observed() {
    let (terminal, probe) =
        FakeTerminal::new([Ok(Event::Key(KeyEvent::new(KeyCode::Char('x'))))], []);
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(spawner, harness.deliveries());
    let mapper_entered = Arc::new(std::sync::Barrier::new(2));
    let release_mapper = Arc::new(std::sync::Barrier::new(2));
    let (mapped, mapping_finished) = std::sync::mpsc::channel();
    let entered = Arc::clone(&mapper_entered);
    let release = Arc::clone(&release_mapper);

    subscriptions
        .reconcile(Subscription::input(move |_| {
            entered.wait();
            release.wait();
            mapped.send(()).unwrap();
            1
        }))
        .unwrap();
    probe.wait_for_polls(1);
    mapper_entered.wait();
    subscriptions.reconcile(Subscription::input(|_| 2)).unwrap();
    release_mapper.wait();
    mapping_finished.recv().unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let delivery = loop {
        assert!(!harness.complete_effect(0));
        if let Some(delivery) = harness.next() {
            break delivery;
        }
        assert!(Instant::now() < deadline, "mapped input was not admitted");
        std::thread::yield_now();
    };

    assert_eq!(delivery, Delivery::Async(1));
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn bounded_input_stops_the_reader_until_the_runtime_accepts_space() {
    let events = (0..66).map(|_| Ok(Event::Key(KeyEvent::new(KeyCode::Char('x')))));
    let (terminal, probe) = FakeTerminal::new(events, []);
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(spawner, harness.deliveries());

    subscriptions
        .reconcile(Subscription::input(|input| input))
        .unwrap();
    probe.wait_for_polls(65);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(probe.polls(), 65);

    assert!(!harness.complete_effect(0));
    probe.wait_for_polls(66);
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn shutdown_releases_a_reader_blocked_by_input_backpressure_before_joining_it() {
    let (finished, completion) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let executor: Arc<dyn Executor> = Arc::new(TokioExecutor::new(runtime.handle().clone()));
        let events = (0..65).map(|_| Ok(Event::Key(KeyEvent::new(KeyCode::Char('x')))));
        let (terminal, probe) = FakeTerminal::new(events, []);
        let spawner = Arc::new(
            TerminalSourceSpawner::new(terminal, executor, Arc::new(RejectSources)).unwrap(),
        );
        let mut subscriptions = SubscriptionExecutor::new(
            Arc::clone(&spawner) as Arc<dyn SourceSpawner<Input>>,
            DeliveryQueue::new(),
        );
        subscriptions
            .reconcile(Subscription::input(|input| input))
            .unwrap();
        probe.wait_for_polls(65);

        subscriptions.stop();
        drop(subscriptions);
        drop(spawner);
        finished.send(()).unwrap();
    });

    completion
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown must not deadlock against bounded input backpressure");
}

#[test]
fn surface_keeps_only_the_latest_unaccepted_window_observation() {
    let (terminal, probe) = FakeTerminal::new(
        [
            Ok(Event::Resize(TerminalSize::new(100, 30))),
            Ok(Event::Resize(TerminalSize::new(120, 40))),
        ],
        [
            Ok(window(80, 24, 640, 384)),
            Ok(window(100, 30, 1000, 600)),
            Ok(window(120, 40, 960, 640)),
        ],
    );
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Surface>>,
        harness.deliveries(),
    );

    subscriptions
        .reconcile(Subscription::surface(|surface| surface))
        .unwrap();
    assert_eq!(
        spawner.presentation_surface(Surface::default()),
        Surface {
            size: TerminalSize::new(80, 24),
            cell_pixels: Some(PixelSize::new(8, 16)),
        }
    );
    probe.wait_for_queries(3);
    assert!(!harness.complete_effect(0));
    assert_eq!(
        spawner.presentation_surface(Surface::default()),
        Surface {
            size: TerminalSize::new(120, 40),
            cell_pixels: Some(PixelSize::new(8, 16)),
        }
    );

    assert_eq!(
        harness.drain(),
        vec![
            Delivery::sync(
                Surface {
                    size: TerminalSize::new(80, 24),
                    cell_pixels: Some(PixelSize::new(8, 16)),
                },
                [],
            ),
            Delivery::sync(
                Surface {
                    size: TerminalSize::new(120, 40),
                    cell_pixels: Some(PixelSize::new(8, 16)),
                },
                [],
            ),
        ]
    );
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn resize_without_a_surface_subscription_updates_presentation_and_requests_redraw() {
    let (terminal, probe) = FakeTerminal::new(
        [Ok(Event::Resize(TerminalSize::new(100, 30)))],
        [Ok(window(100, 30, 800, 600))],
    );
    let harness: Harness<Surface> = Harness::new(TerminalSize::ZERO);
    let spawner =
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap();
    let deliveries = DeliveryQueue::<Surface>::new();

    spawner.attach_runtime(deliveries.clone());
    probe.wait_for_queries(1);
    let deadline = Instant::now() + Duration::from_secs(2);
    while deliveries.try_next().is_none() {
        assert!(Instant::now() < deadline, "redraw request was not accepted");
        std::thread::yield_now();
    }

    assert_eq!(
        spawner.presentation_surface(Surface::default()),
        Surface {
            size: TerminalSize::new(100, 30),
            cell_pixels: Some(PixelSize::new(8, 20)),
        }
    );
}

#[test]
fn resize_after_surface_unsubscription_still_updates_presentation() {
    let (terminal, probe) = FakeTerminal::new([], [Ok(window(80, 24, 640, 384))]);
    let harness: Harness<Surface> = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let deliveries = harness.deliveries();
    spawner.attach_runtime(deliveries.clone());
    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Surface>>,
        deliveries.clone(),
    );
    subscriptions
        .reconcile(Subscription::surface(|surface| surface))
        .unwrap();
    assert!(matches!(deliveries.try_next(), Some(Delivery::Sync { .. })));
    subscriptions.stop();

    probe.push_window(Ok(window(120, 40, 960, 800)));
    probe.push_event(Ok(Event::Resize(TerminalSize::new(120, 40))));
    probe.wait_for_queries(2);
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(deliveries.try_next(), Some(Delivery::Redraw)) {
            break;
        }
        assert!(Instant::now() < deadline, "redraw request was not accepted");
        std::thread::yield_now();
    }

    assert_eq!(
        spawner.presentation_surface(Surface::default()),
        Surface {
            size: TerminalSize::new(120, 40),
            cell_pixels: Some(PixelSize::new(8, 20)),
        }
    );
}

#[test]
fn resize_observed_before_unsubscription_becomes_presentation_only_after_it() {
    let (terminal, _probe) = FakeTerminal::new([], [Ok(window(80, 24, 640, 384))]);
    let harness: Harness<Surface> = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let deliveries = harness.deliveries();
    spawner.attach_runtime(deliveries.clone());
    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Surface>>,
        deliveries.clone(),
    );
    subscriptions
        .reconcile(Subscription::surface(|surface| surface))
        .unwrap();
    assert!(matches!(deliveries.try_next(), Some(Delivery::Sync { .. })));

    let (endpoint_id, mapper, publisher) =
        surface_endpoint(&spawner.owner.shared).expect("surface endpoint is registered");
    let observed = Surface {
        size: TerminalSize::new(120, 40),
        cell_pixels: Some(PixelSize::new(8, 20)),
    };
    let observation = Observation::Surface {
        endpoint_id,
        surface: observed,
        mapper,
        publisher,
        sizes: Arc::clone(&spawner.owner.shared.surface_sizes),
        sources: Arc::clone(&spawner.owner.shared.sources),
    };

    subscriptions.stop();
    observation.deliver();

    assert_eq!(spawner.presentation_surface(Surface::default()), observed);
    assert_eq!(deliveries.try_next(), Some(Delivery::Redraw));
}

#[test]
fn presentation_only_resize_observed_before_subscription_cannot_replace_its_initial_surface() {
    let newer = Surface {
        size: TerminalSize::new(120, 40),
        cell_pixels: Some(PixelSize::new(8, 20)),
    };
    let (terminal, _probe) = FakeTerminal::new([], [Ok(window(120, 40, 960, 800))]);
    let harness: Harness<Surface> = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let deliveries = harness.deliveries();
    spawner.attach_runtime(deliveries.clone());
    let old_observation = Observation::PresentationSurface {
        surface: Surface {
            size: TerminalSize::new(100, 30),
            cell_pixels: Some(PixelSize::new(8, 20)),
        },
        sizes: Arc::clone(&spawner.owner.shared.surface_sizes),
        sources: Arc::clone(&spawner.owner.shared.sources),
    };

    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Surface>>,
        deliveries.clone(),
    );
    subscriptions
        .reconcile(Subscription::surface(|surface| surface))
        .unwrap();
    assert!(matches!(deliveries.try_next(), Some(Delivery::Sync { .. })));

    old_observation.deliver();

    assert_eq!(spawner.presentation_surface(Surface::default()), newer);
    assert!(deliveries.try_next().is_none());
    subscriptions.stop();
}

#[test]
fn surface_observations_use_the_mapper_current_when_the_resize_arrives() {
    let (terminal, probe) = FakeTerminal::new([], [Ok(window(80, 24, 640, 384))]);
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(spawner, harness.deliveries());

    subscriptions
        .reconcile(Subscription::surface(|surface| (1, surface)))
        .unwrap();
    assert_eq!(
        harness.next(),
        Some(Delivery::sync(
            (
                1,
                Surface {
                    size: TerminalSize::new(80, 24),
                    cell_pixels: Some(PixelSize::new(8, 16)),
                }
            ),
            [],
        ))
    );
    subscriptions
        .reconcile(Subscription::surface(|surface| (2, surface)))
        .unwrap();
    probe.push_window(Ok(window(100, 25, 800, 400)));
    probe.push_event(Ok(Event::Resize(TerminalSize::new(100, 25))));
    probe.wait_for_queries(2);

    let deadline = Instant::now() + Duration::from_secs(2);
    let delivery = loop {
        assert!(!harness.complete_effect(0));
        if let Some(delivery) = harness.next() {
            break delivery;
        }
        assert!(
            Instant::now() < deadline,
            "surface observation was not delivered"
        );
        std::thread::yield_now();
    };

    assert_eq!(
        delivery,
        Delivery::sync(
            (
                2,
                Surface {
                    size: TerminalSize::new(100, 25),
                    cell_pixels: Some(PixelSize::new(8, 16)),
                }
            ),
            [],
        )
    );
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn initial_surface_query_failure_prevents_runtime_construction() {
    let (terminal, _probe) = FakeTerminal::new([], [Err(io::Error::other("size failed"))]);
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(spawner, harness.deliveries());

    let error = subscriptions
        .reconcile(Subscription::<Surface>::surface(|surface| surface))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Other);
    assert_eq!(error.to_string(), "size failed");
}

#[test]
fn event_read_failure_reaches_the_runtime_failure_path() {
    let (terminal, _probe) = FakeTerminal::new(
        [Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "input failed",
        ))],
        [],
    );
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Input>>,
        harness.deliveries(),
    );
    subscriptions
        .reconcile(Subscription::input(|input| input))
        .unwrap();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let error = runtime.block_on(spawner.failure());
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert_eq!(error.to_string(), "input failed");
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn resize_query_failure_reaches_the_runtime_failure_path() {
    let (terminal, probe) = FakeTerminal::new([], [Ok(window(80, 24, 640, 384))]);
    let harness = Harness::new(TerminalSize::ZERO);
    let spawner = Arc::new(
        TerminalSourceSpawner::new(terminal, harness.executor(), Arc::new(RejectSources)).unwrap(),
    );
    let mut subscriptions = SubscriptionExecutor::new(
        Arc::clone(&spawner) as Arc<dyn SourceSpawner<Surface>>,
        harness.deliveries(),
    );
    subscriptions
        .reconcile(Subscription::surface(|surface| surface))
        .unwrap();
    probe.push_window(Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "resize query failed",
    )));
    probe.push_event(Ok(Event::Resize(TerminalSize::new(100, 25))));

    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let error = runtime.block_on(spawner.failure());
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert_eq!(error.to_string(), "resize query failed");
    subscriptions.stop();
    assert!(!harness.complete_effect(0));
}

#[test]
fn event_read_failure_stops_the_core_and_shuts_down_presentation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let (terminal, _probe) = FakeTerminal::new(
        [Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "input failed",
        ))],
        [],
    );
    let executor: Arc<dyn Executor> = Arc::new(TokioExecutor::new(runtime.handle().clone()));
    let spawner: Arc<dyn SourceSpawner<Input>> = Arc::new(
        TerminalSourceSpawner::new(terminal, Arc::clone(&executor), Arc::new(RejectSources))
            .unwrap(),
    );
    let shutdown = Arc::new(Mutex::new(false));
    let core = RuntimeCore::new(
        InputApplication,
        executor,
        Arc::new(TokioClock) as Arc<dyn Clock>,
        spawner,
        PendingPresentation {
            shutdown: Arc::clone(&shutdown),
        },
    )
    .unwrap();

    let error = runtime.block_on(core.run()).unwrap_err();
    let RuntimeError::Terminal(error) = error;
    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert!(*shutdown.lock().unwrap());
}

#[test]
fn startup_surface_updates_the_model_before_the_first_view() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let (terminal, _probe) = FakeTerminal::new([], [Ok(window(80, 24, 640, 384))]);
    let viewed = Arc::new(Mutex::new(Vec::new()));
    let executor: Arc<dyn Executor> = Arc::new(TokioExecutor::new(runtime.handle().clone()));
    let spawner: Arc<dyn SourceSpawner<Surface>> = Arc::new(
        TerminalSourceSpawner::new(terminal, Arc::clone(&executor), Arc::new(RejectSources))
            .unwrap(),
    );
    let core = RuntimeCore::new(
        SurfaceApplication {
            viewed: Arc::clone(&viewed),
        },
        executor,
        Arc::new(TokioClock) as Arc<dyn Clock>,
        spawner,
        StopOnSubmit,
    )
    .unwrap();

    let error = runtime.block_on(core.run()).unwrap_err();
    assert!(matches!(error, RuntimeError::Presentation(StopDrawing)));
    assert_eq!(&*viewed.lock().unwrap(), &[TerminalSize::new(80, 24)]);
}

struct RejectSources;

impl<Message: Send + 'static> SourceSpawner<Message> for RejectSources {
    fn start(
        &self,
        _source: Source<Message>,
        _deliveries: DeliveryQueue<Message>,
    ) -> io::Result<Box<dyn RunningSource<Message>>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unexpected non-terminal source",
        ))
    }
}

struct SurfaceApplication {
    viewed: Arc<Mutex<Vec<TerminalSize>>>,
}

struct InputApplication;

impl Application for InputApplication {
    type Model = ();
    type Message = Input;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        ((), Effect::none())
    }

    fn update(&self, _model: &mut Self::Model, _message: Self::Message) -> Effect<Self::Message> {
        Effect::none()
    }

    fn view(&self, _model: &Self::Model) -> View {
        View::text("waiting", TextStyle::new())
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::input(|input| input)
    }
}

impl Application for SurfaceApplication {
    type Model = Surface;
    type Message = Surface;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (Surface::new(0, 0), Effect::none())
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        *model = message;
        Effect::none()
    }

    fn view(&self, model: &Self::Model) -> View {
        self.viewed.lock().unwrap().push(model.size);
        View::text(model.size.columns().to_string(), TextStyle::new())
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::surface(|surface| surface)
    }
}

#[derive(Debug)]
struct StopDrawing;

impl std::fmt::Display for StopDrawing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("stop after observing the first view")
    }
}

impl std::error::Error for StopDrawing {}

struct StopOnSubmit;

impl Presentation for StopOnSubmit {
    type Error = StopDrawing;

    fn submit(&mut self, _view: View) -> Result<(), Self::Error> {
        Err(StopDrawing)
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(std::future::pending())
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        Box::pin(async { Ok(()) })
    }
}

struct PendingPresentation {
    shutdown: Arc<Mutex<bool>>,
}

impl Presentation for PendingPresentation {
    type Error = Infallible;

    fn submit(&mut self, _view: View) -> Result<(), Self::Error> {
        Ok(())
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(std::future::pending())
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        *self.shutdown.lock().unwrap() = true;
        Box::pin(async { Ok(()) })
    }
}

fn window(columns: usize, rows: usize, width: usize, height: usize) -> WindowSize {
    WindowSize::new(
        TerminalSize::new(columns, rows),
        Some(PixelSize::new(width, height)),
    )
}

struct FakeTerminal {
    shared: Arc<FakeShared>,
}

struct FakeProbe {
    shared: Arc<FakeShared>,
}

struct FakeShared {
    state: Mutex<FakeState>,
    changed: Condvar,
}

struct FakeState {
    events: VecDeque<io::Result<Event>>,
    windows: VecDeque<io::Result<WindowSize>>,
    polls: usize,
    queries: usize,
}

impl FakeTerminal {
    fn new(
        events: impl IntoIterator<Item = io::Result<Event>>,
        windows: impl IntoIterator<Item = io::Result<WindowSize>>,
    ) -> (Self, FakeProbe) {
        let shared = Arc::new(FakeShared {
            state: Mutex::new(FakeState {
                events: events.into_iter().collect(),
                windows: windows.into_iter().collect(),
                polls: 0,
                queries: 0,
            }),
            changed: Condvar::new(),
        });
        (
            Self {
                shared: Arc::clone(&shared),
            },
            FakeProbe { shared },
        )
    }
}

impl EventSource for FakeTerminal {
    fn read_event(&mut self) -> io::Result<Event> {
        loop {
            if let Some(event) = self.poll_event_timeout(Duration::from_millis(25))? {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.poll_event_timeout(Duration::ZERO)
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        let mut state = self.shared.state.lock().unwrap();
        state.polls += 1;
        self.shared.changed.notify_all();
        if state.events.is_empty() && !timeout.is_zero() {
            let (next, _) = self.shared.changed.wait_timeout(state, timeout).unwrap();
            state = next;
        }
        state.events.pop_front().transpose()
    }
}

impl TerminalQuery for FakeTerminal {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        self.window_size().map(WindowSize::cells)
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        Ok(Position::new(0, 0))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        let result = {
            let mut state = self.shared.state.lock().unwrap();
            state.queries += 1;
            state
                .windows
                .pop_front()
                .unwrap_or_else(|| Ok(WindowSize::new(TerminalSize::ZERO, None)))
        };
        self.shared.changed.notify_all();
        result
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        Ok(false)
    }
}

impl FakeProbe {
    fn polls(&self) -> usize {
        self.shared.state.lock().unwrap().polls
    }

    fn wait_for_polls(&self, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut state = self.shared.state.lock().unwrap();
        while state.polls < expected {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "terminal reader did not reach {expected} polls"
            );
            let (next, timeout) = self.shared.changed.wait_timeout(state, remaining).unwrap();
            state = next;
            assert!(!timeout.timed_out() || state.polls >= expected);
        }
    }

    fn wait_for_queries(&self, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut state = self.shared.state.lock().unwrap();
        while state.queries < expected {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "terminal reader did not reach {expected} queries"
            );
            let (next, timeout) = self.shared.changed.wait_timeout(state, remaining).unwrap();
            state = next;
            assert!(!timeout.timed_out() || state.queries >= expected);
        }
    }

    fn push_event(&self, event: io::Result<Event>) {
        self.shared.state.lock().unwrap().events.push_back(event);
        self.shared.changed.notify_all();
    }

    fn push_window(&self, window: io::Result<WindowSize>) {
        self.shared.state.lock().unwrap().windows.push_back(window);
    }
}
