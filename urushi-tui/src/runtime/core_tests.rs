use std::cell::RefCell;
use std::convert::Infallible;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use urushi::{TextStyle, View};
use urushi_terminal::TerminalSize;

use super::*;
use crate::runtime::effect::Effect;
use crate::runtime::executor::{RunningSource, SourceSpawner};
use crate::runtime::executor::{TokioClock, TokioExecutor};
use crate::runtime::presentation::DrawResult;
use crate::runtime::presentation::Presentation;
use crate::runtime::scheduler::DEFAULT_MINIMUM_INTERVAL;
use crate::runtime::subscription::{Source, Subscription};
use crate::runtime::testing::Harness;

impl<A, P> RuntimeCore<A, P>
where
    A: Application,
    P: Presentation,
{
    /// Polls and submits one ready draw without running the asynchronous loop.
    fn drive_draw(&mut self) -> Result<(), RuntimeError<P::Error>> {
        if self.stopping {
            return Ok(());
        }
        let ready = {
            let mut next = Box::pin(
                self.scheduler
                    .next_draw(|| self.deliveries.sync_fence_allows_draw()),
            );
            matches!(
                next.as_mut().poll(&mut Context::from_waker(Waker::noop())),
                Poll::Ready(())
            )
        };
        if !ready {
            return Ok(());
        }

        let view = self.application.view(&self.model);
        self.presentation
            .submit(view)
            .map_err(RuntimeError::Presentation)
    }

    fn draw_completed(&mut self, completed_at: Instant) {
        self.scheduler.draw_completed(completed_at);
    }
}

#[derive(Clone, Copy)]
enum Message {
    Add(i32),
    Stop,
}

struct Counter {
    views: Rc<RefCell<Vec<i32>>>,
}

impl Application for Counter {
    type Model = i32;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (0, Effect::none())
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Add(value) => {
                *model += value;
                Effect::none()
            }
            Message::Stop => Effect::shutdown(),
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        self.views.borrow_mut().push(*model);
        View::text(model.to_string(), TextStyle::new())
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::none()
    }
}

#[derive(Clone)]
struct RecordingPresentation {
    submitted: Rc<RefCell<usize>>,
    shutdown: Rc<RefCell<bool>>,
}

impl RecordingPresentation {
    fn new() -> Self {
        Self {
            submitted: Rc::new(RefCell::new(0)),
            shutdown: Rc::new(RefCell::new(false)),
        }
    }
}

impl Presentation for RecordingPresentation {
    type Error = Infallible;

    fn submit(&mut self, _view: View) -> Result<(), Self::Error> {
        *self.submitted.borrow_mut() += 1;
        Ok(())
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(std::future::pending())
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        *self.shutdown.borrow_mut() = true;
        Box::pin(async { Ok(()) })
    }
}

struct NoSources;

impl<T: Send + 'static> SourceSpawner<T> for NoSources {
    fn start(
        &self,
        _source: Source<T>,
        _deliveries: DeliveryQueue<T>,
    ) -> io::Result<Box<dyn RunningSource<T>>> {
        panic!("these applications declare no sources")
    }
}

struct PassiveSources;
struct PassiveSource;

impl<T: Send + 'static> SourceSpawner<T> for PassiveSources {
    fn start(
        &self,
        _source: Source<T>,
        _deliveries: DeliveryQueue<T>,
    ) -> io::Result<Box<dyn RunningSource<T>>> {
        Ok(Box::new(PassiveSource))
    }
}

impl<T> RunningSource<T> for PassiveSource {
    fn refresh(&mut self, _source: Source<T>) {}
}

fn runtime(
    views: Rc<RefCell<Vec<i32>>>,
    presentation: RecordingPresentation,
) -> (
    RuntimeCore<Counter, RecordingPresentation>,
    Harness<Message>,
) {
    let harness: Harness<Message> = Harness::new(TerminalSize::new(1, 1));
    let core = RuntimeCore::new(
        Counter { views },
        harness.executor(),
        harness.clock(),
        Arc::new(NoSources),
        presentation,
    )
    .unwrap();
    (core, harness)
}

#[test]
fn idle_draws_immediately_and_busy_updates_coalesce_to_the_latest_model() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let presentation = RecordingPresentation::new();
    let submitted = Rc::clone(&presentation.submitted);
    let (mut core, harness) = runtime(Rc::clone(&views), presentation);

    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0]);

    let deliveries = core.deliveries();
    deliveries.ordinary_completion().complete(Message::Add(1));
    deliveries.ordinary_completion().complete(Message::Add(2));
    core.process_delivery(deliveries.try_next().unwrap())
        .unwrap();
    core.process_delivery(deliveries.try_next().unwrap())
        .unwrap();
    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0]);

    core.draw_completed(harness.clock().now());
    harness.advance(DEFAULT_MINIMUM_INTERVAL - Duration::from_nanos(1));
    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0]);

    harness.advance(Duration::from_nanos(1));
    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0, 3]);
    assert_eq!(*submitted.borrow(), 2);
}

#[test]
fn sync_acceptance_fences_the_initial_draw_and_applies_its_batch_without_intermediate_views() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let presentation = RecordingPresentation::new();
    let (mut core, _harness) = runtime(Rc::clone(&views), presentation);
    let deliveries = core.deliveries();
    let (publisher, mut slot) = crate::runtime::delivery::surface_slot();

    deliveries.ordinary_completion().complete(Message::Add(10));
    assert!(publisher.publish(Message::Add(1)));
    assert!(slot.try_accept(&deliveries));
    core.drive_draw().unwrap();
    assert!(views.borrow().is_empty());

    core.process_delivery(deliveries.try_next().unwrap())
        .unwrap();
    core.drive_draw().unwrap();
    assert!(views.borrow().is_empty());

    let Delivery::Sync { first, .. } = deliveries.try_next().unwrap() else {
        panic!("surface observations are Sync")
    };
    core.process_delivery(Delivery::sync(first, [Message::Add(2)]))
        .unwrap();
    core.drive_draw().unwrap();

    assert_eq!(&*views.borrow(), &[13]);
}

#[test]
fn sync_accepted_during_a_draw_fences_only_the_next_draw() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let presentation = RecordingPresentation::new();
    let (mut core, harness) = runtime(Rc::clone(&views), presentation);
    let deliveries = core.deliveries();
    let (publisher, mut slot) = crate::runtime::delivery::surface_slot();

    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0]);

    assert!(publisher.publish(Message::Add(4)));
    assert!(slot.try_accept(&deliveries));
    let completed_at = harness.clock().now();
    core.draw_completed(completed_at);
    harness.advance(DEFAULT_MINIMUM_INTERVAL);
    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0]);

    core.process_delivery(deliveries.try_next().unwrap())
        .unwrap();
    core.drive_draw().unwrap();
    assert_eq!(&*views.borrow(), &[0, 4]);
}

#[test]
fn shutdown_effect_stops_before_another_view() {
    let views = Rc::new(RefCell::new(Vec::new()));
    let presentation = RecordingPresentation::new();
    let (mut core, _harness) = runtime(Rc::clone(&views), presentation);
    let deliveries = core.deliveries();
    deliveries.ordinary_completion().complete(Message::Stop);

    core.process_delivery(deliveries.try_next().unwrap())
        .unwrap();
    core.drive_draw().unwrap();

    assert!(core.is_stopping());
    assert!(views.borrow().is_empty());
}

struct ImmediateStop;

impl Application for ImmediateStop {
    type Model = i32;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (7, Effect::shutdown())
    }

    fn update(&self, _model: &mut Self::Model, _message: Self::Message) -> Effect<Self::Message> {
        unreachable!()
    }

    fn view(&self, _model: &Self::Model) -> View {
        unreachable!()
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::none()
    }
}

#[test]
fn run_stops_executors_and_presentation_before_returning_the_model() {
    let harness: Harness<Message> = Harness::new(TerminalSize::new(1, 1));
    let presentation = RecordingPresentation::new();
    let shutdown = Rc::clone(&presentation.shutdown);
    let core = RuntimeCore::new(
        ImmediateStop,
        harness.executor(),
        harness.clock(),
        Arc::new(NoSources),
        presentation,
    )
    .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    let model = runtime.block_on(core.run()).unwrap();

    assert_eq!(model, 7);
    assert!(*shutdown.borrow());
}

struct LoopApplication {
    views: Rc<RefCell<Vec<i32>>>,
}

impl Application for LoopApplication {
    type Model = i32;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (0, Effect::future(async { Message::Add(1) }))
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Add(value) => {
                *model += value;
                Effect::after(Duration::from_millis(50), |_| Message::Stop)
            }
            Message::Stop => Effect::shutdown(),
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        self.views.borrow_mut().push(*model);
        View::text(model.to_string(), TextStyle::new())
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::none()
    }
}

struct ImmediatePresentation {
    results_tx: tokio::sync::mpsc::UnboundedSender<DrawResult>,
    results_rx: tokio::sync::mpsc::UnboundedReceiver<DrawResult>,
}

impl ImmediatePresentation {
    fn new() -> Self {
        let (results_tx, results_rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            results_tx,
            results_rx,
        }
    }
}

impl Presentation for ImmediatePresentation {
    type Error = Infallible;

    fn submit(&mut self, _view: View) -> Result<(), Self::Error> {
        self.results_tx.send(DrawResult::Completed).unwrap();
        Ok(())
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        Box::pin(async { Ok(self.results_rx.recv().await.unwrap()) })
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn run_selects_draw_delivery_completion_and_scheduler_timer_before_shutdown() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let views = Rc::new(RefCell::new(Vec::new()));
    let application = LoopApplication {
        views: Rc::clone(&views),
    };
    let executor = Arc::new(TokioExecutor::new(runtime.handle().clone()));
    let clock = Arc::new(TokioClock);
    let core = RuntimeCore::new(
        application,
        executor,
        clock,
        Arc::new(NoSources),
        ImmediatePresentation::new(),
    )
    .unwrap();

    let model = runtime.block_on(core.run()).unwrap();

    assert_eq!(model, 1);
    assert_eq!(&*views.borrow(), &[0, 1]);
}

struct RecoverDrawFailure;

impl Application for RecoverDrawFailure {
    type Model = bool;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (false, Effect::none())
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Stop => {
                *model = true;
                Effect::shutdown()
            }
            Message::Add(_) => unreachable!(),
        }
    }

    fn view(&self, _model: &Self::Model) -> View {
        View::text("frame", TextStyle::new())
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::terminal_errors(|_| Message::Stop)
    }
}

struct FailingPresentation {
    result: Option<DrawResult>,
}

impl Presentation for FailingPresentation {
    type Error = Infallible;

    fn submit(&mut self, _view: View) -> Result<(), Self::Error> {
        self.result = Some(DrawResult::Failed {
            error: io::Error::other("draw failed"),
        });
        Ok(())
    }

    fn completed(&mut self) -> Pin<Box<dyn Future<Output = Result<DrawResult, Self::Error>> + '_>> {
        let result = self.result.take();
        Box::pin(async move {
            match result {
                Some(result) => Ok(result),
                None => std::future::pending().await,
            }
        })
    }

    fn shutdown(&mut self) -> Pin<Box<dyn Future<Output = Result<(), Self::Error>> + '_>> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn a_subscribed_draw_failure_returns_to_update_as_an_async_message() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let core = RuntimeCore::new(
        RecoverDrawFailure,
        Arc::new(TokioExecutor::new(runtime.handle().clone())),
        Arc::new(TokioClock),
        Arc::new(PassiveSources),
        FailingPresentation { result: None },
    )
    .unwrap();

    let handled = runtime.block_on(core.run()).unwrap();

    assert!(handled);
}
