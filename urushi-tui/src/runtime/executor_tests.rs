use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use super::{
    BlockingTask, Clock, EffectControl, EffectExecutor, Execution, Executor, RunningSource,
    SourceSpawner, SubscriptionExecutor, Task,
};
use crate::runtime::delivery::{Delivery, DeliveryQueue};
use crate::runtime::subscription::Source;
use crate::runtime::testing::Ready;
use crate::{Effect, Subscription};

#[test]
fn one_shot_effects_are_scheduled_in_source_order_and_delivered_as_they_complete() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    assert_eq!(
        effects.start(Effect::batch([
            Effect::perform(|| 1),
            Effect::perform(|| 2),
            Effect::future(async { 3 }),
        ])),
        EffectControl::Continue
    );
    assert_eq!(
        executor.scheduled(),
        vec![TaskKind::Blocking, TaskKind::Blocking, TaskKind::Future]
    );

    let first = executor.take_next_blocking();
    let second = executor.take_next_blocking();
    second();
    first();
    executor.run_next();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async(2)));
    assert_eq!(deliveries.try_next(), Some(Delivery::Async(1)));
    assert_eq!(deliveries.try_next(), Some(Delivery::Async(3)));
    assert_eq!(deliveries.try_next(), None);
}

#[test]
fn replacement_drops_an_unstarted_future() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    effects.start(Effect::future_latest("preview", async { "old" }));
    effects.start(Effect::future_latest("preview", async { "new" }));
    executor.run_next();
    executor.run_next();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async("new")));
    assert_eq!(deliveries.try_next(), None);
}

#[test]
fn replacement_does_not_remove_an_already_accepted_latest_completion() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    effects.start(Effect::future_latest("preview", async { "accepted" }));
    executor.run_next();
    effects.start(Effect::future_latest("preview", async { "replacement" }));
    executor.run_next();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async("accepted")));
    assert_eq!(deliveries.try_next(), Some(Delivery::Async("replacement")));
}

#[test]
fn replacement_suppresses_a_running_blocking_completion() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    effects.start(Effect::perform_latest("preview", || "old"));
    let old = executor.take_next_blocking();
    effects.start(Effect::perform_latest("preview", || "new"));

    // A blocking closure already handed to a worker may finish, but its
    // runtime-known replacement suppresses the completion.
    old();
    executor.run_next();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async("new")));
    assert_eq!(deliveries.try_next(), None);
}

#[test]
fn ordinary_stale_completion_is_still_delivered() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    effects.start(Effect::perform(|| "ordinary"));
    let ordinary = executor.take_next_blocking();
    effects.start(Effect::perform_latest("preview", || "latest"));
    ordinary();
    executor.run_next();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async("ordinary")));
    assert_eq!(deliveries.try_next(), Some(Delivery::Async("latest")));
}

#[test]
fn shutdown_starts_no_sibling_and_suppresses_in_flight_work() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries.clone(),
    );

    effects.start(Effect::perform(|| "in flight"));
    let in_flight = executor.take_next_blocking();
    assert_eq!(
        effects.start(Effect::batch([
            Effect::perform(|| "must not start"),
            Effect::shutdown(),
        ])),
        EffectControl::Shutdown
    );

    in_flight();
    assert!(executor.scheduled().is_empty());
    assert_eq!(deliveries.try_next(), None);
}

#[test]
fn a_panicked_task_is_reaped_on_the_next_effect() {
    let executor = ManualExecutor::new();
    let deliveries = DeliveryQueue::<()>::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(ImmediateClock::new()),
        deliveries,
    );

    effects.start(Effect::perform(|| panic!("effect failed")));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| executor.run_next()));
    assert!(panic.is_err());
    assert_eq!(effects.ordinary.len(), 1);

    effects.start(Effect::none());
    assert!(effects.ordinary.is_empty());
}

#[test]
fn delayed_effect_uses_the_runtime_clock() {
    let executor = ManualExecutor::new();
    let clock = ImmediateClock::new();
    let deliveries = DeliveryQueue::new();
    let mut effects = EffectExecutor::new(
        Arc::new(executor.clone()),
        Arc::new(clock.clone()),
        deliveries.clone(),
    );

    effects.start(Effect::after(Duration::from_millis(25), |at| at));
    executor.run_next();

    assert_eq!(clock.sleeps(), vec![Duration::from_millis(25)]);
    assert_eq!(
        deliveries.try_next(),
        Some(Delivery::Async(clock.origin + Duration::from_millis(25)))
    );
}

#[test]
fn reconciliation_starts_refreshes_and_stops_sources() {
    let deliveries = DeliveryQueue::new();
    let spawner = Arc::new(ObservableSources);
    let mut subscriptions = SubscriptionExecutor::new(spawner, deliveries.clone());

    subscriptions.reconcile(Subscription::batch([
        Subscription::run_blocking("alpha", |_| {}),
        Subscription::run_blocking("beta", |_| {}),
    ]));
    assert_events(
        &deliveries,
        [
            SourceEvent::Started("Key(alpha)".into()),
            SourceEvent::Started("Key(beta)".into()),
        ],
    );

    subscriptions.reconcile(Subscription::batch([
        Subscription::run_blocking("alpha", |_| {}),
        Subscription::run_blocking("gamma", |_| {}),
    ]));
    let mut events = take_events(&deliveries);
    events.sort();
    assert_eq!(
        events,
        vec![
            SourceEvent::Started("Key(gamma)".into()),
            SourceEvent::Refreshed("Key(alpha)".into()),
            SourceEvent::Stopped("Key(beta)".into()),
        ]
    );

    subscriptions.stop();
    let mut events = take_events(&deliveries);
    events.sort();
    assert_eq!(
        events,
        vec![
            SourceEvent::Stopped("Key(alpha)".into()),
            SourceEvent::Stopped("Key(gamma)".into()),
        ]
    );
}

#[test]
fn duplicate_subscription_key_uses_the_latest_declaration_once() {
    let deliveries = DeliveryQueue::new();
    let mut subscriptions =
        SubscriptionExecutor::new(Arc::new(ObservableSources), deliveries.clone());

    subscriptions.reconcile(Subscription::batch([
        Subscription::stream(
            "source",
            Ready::new([SourceEvent::Started("unused".into())]),
        ),
        Subscription::run_blocking("source", |_| {}),
    ]));

    assert_eq!(
        take_events(&deliveries),
        vec![SourceEvent::Started("Key(source)".into())]
    );
}

#[derive(Clone)]
struct ManualExecutor {
    jobs: Arc<Mutex<VecDeque<Job>>>,
}

impl ManualExecutor {
    fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    fn scheduled(&self) -> Vec<TaskKind> {
        self.jobs
            .lock()
            .expect("no test panics under this lock")
            .iter()
            .map(Job::kind)
            .collect()
    }

    fn run_next(&self) {
        let job = self
            .jobs
            .lock()
            .expect("no test panics under this lock")
            .pop_front()
            .expect("a scheduled task remains");
        job.run();
    }

    fn take_next_blocking(&self) -> BlockingTask {
        match self
            .jobs
            .lock()
            .expect("no test panics under this lock")
            .pop_front()
            .expect("a scheduled task remains")
        {
            Job::Blocking { task, .. } => task,
            Job::Future { .. } => panic!("the next task is not blocking"),
        }
    }
}

impl Executor for ManualExecutor {
    fn spawn(&self, task: Task) -> Box<dyn Execution> {
        self.schedule(Job::Future {
            task,
            canceled: Arc::new(AtomicFlag::new()),
        })
    }

    fn spawn_blocking(&self, task: BlockingTask) -> Box<dyn Execution> {
        self.schedule(Job::Blocking {
            task,
            canceled: Arc::new(AtomicFlag::new()),
        })
    }
}

impl ManualExecutor {
    fn schedule(&self, job: Job) -> Box<dyn Execution> {
        let canceled = job.canceled();
        self.jobs
            .lock()
            .expect("no test panics under this lock")
            .push_back(job);
        Box::new(ManualExecution { canceled })
    }
}

enum Job {
    Future {
        task: Task,
        canceled: Arc<AtomicFlag>,
    },
    Blocking {
        task: BlockingTask,
        canceled: Arc<AtomicFlag>,
    },
}

impl Job {
    fn kind(&self) -> TaskKind {
        match self {
            Self::Future { .. } => TaskKind::Future,
            Self::Blocking { .. } => TaskKind::Blocking,
        }
    }

    fn canceled(&self) -> Arc<AtomicFlag> {
        match self {
            Self::Future { canceled, .. } | Self::Blocking { canceled, .. } => Arc::clone(canceled),
        }
    }

    fn run(self) {
        match self {
            Self::Future { mut task, canceled } => {
                if canceled.get() {
                    return;
                }
                let mut context = Context::from_waker(Waker::noop());
                assert_eq!(task.as_mut().poll(&mut context), Poll::Ready(()));
            }
            Self::Blocking { task, canceled } => {
                if !canceled.get() {
                    task();
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TaskKind {
    Future,
    Blocking,
}

struct ManualExecution {
    canceled: Arc<AtomicFlag>,
}

impl Execution for ManualExecution {}

impl Drop for ManualExecution {
    fn drop(&mut self) {
        self.canceled.set();
    }
}

struct AtomicFlag(std::sync::atomic::AtomicBool);

impl AtomicFlag {
    fn new() -> Self {
        Self(std::sync::atomic::AtomicBool::new(false))
    }

    fn get(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Acquire)
    }

    fn set(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
}

#[derive(Clone)]
struct ImmediateClock {
    origin: Instant,
    sleeps: Arc<Mutex<Vec<Duration>>>,
}

impl ImmediateClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
            sleeps: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn sleeps(&self) -> Vec<Duration> {
        self.sleeps
            .lock()
            .expect("no test panics under this lock")
            .clone()
    }
}

impl Clock for ImmediateClock {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = Instant> + Send + 'static>> {
        self.sleeps
            .lock()
            .expect("no test panics under this lock")
            .push(duration);
        let at = self.origin + duration;
        Box::pin(async move { at })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SourceEvent {
    Started(String),
    Refreshed(String),
    Stopped(String),
}

struct ObservableSources;

impl SourceSpawner<SourceEvent> for ObservableSources {
    fn start(
        &self,
        source: Source<SourceEvent>,
        deliveries: DeliveryQueue<SourceEvent>,
    ) -> Box<dyn RunningSource<SourceEvent>> {
        let key = format!("{:?}", source.key);
        deliveries
            .ordinary_completion()
            .complete(SourceEvent::Started(key.clone()));
        Box::new(ObservableSource { key, deliveries })
    }
}

struct ObservableSource {
    key: String,
    deliveries: DeliveryQueue<SourceEvent>,
}

impl RunningSource<SourceEvent> for ObservableSource {
    fn refresh(&mut self, _source: Source<SourceEvent>) {
        self.deliveries
            .ordinary_completion()
            .complete(SourceEvent::Refreshed(self.key.clone()));
    }
}

impl Drop for ObservableSource {
    fn drop(&mut self) {
        self.deliveries
            .ordinary_completion()
            .complete(SourceEvent::Stopped(self.key.clone()));
    }
}

fn take_events(deliveries: &DeliveryQueue<SourceEvent>) -> Vec<SourceEvent> {
    let mut events = Vec::new();
    while let Some(Delivery::Async(event)) = deliveries.try_next() {
        events.push(event);
    }
    events
}

fn assert_events(
    deliveries: &DeliveryQueue<SourceEvent>,
    expected: impl IntoIterator<Item = SourceEvent>,
) {
    assert_eq!(
        take_events(deliveries),
        expected.into_iter().collect::<Vec<_>>()
    );
}
