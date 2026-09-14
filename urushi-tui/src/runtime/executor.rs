//! Execution of effects and reconciliation of long-lived sources.
//!
//! The application-facing values name no executor. This module is the runtime
//! side of that boundary: it hands work to an [`Executor`], owns the returned
//! cancellation handles, and sends completed messages into the one
//! [`DeliveryQueue`]. A handle is kept for exactly as long as the runtime may
//! still accept that execution's completion.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use urushi::Key;

use super::delivery::{DeliveryQueue, EffectCancellation};
use super::effect::{Effect, EffectKind};
use super::subscription::{Source, Subscription};

pub(crate) type Task = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;
pub(crate) type BlockingTask = Box<dyn FnOnce() + Send + 'static>;

/// A running task whose drop cancels work that has not completed yet.
pub(crate) trait Execution: Send {}

/// Where runtime work runs, expressed without an executor's task type.
pub(crate) trait Executor: Send + Sync + 'static {
    fn spawn(&self, task: Task) -> Box<dyn Execution>;

    fn spawn_blocking(&self, task: BlockingTask) -> Box<dyn Execution>;
}

/// The source of time shared by delayed effects and interval subscriptions.
pub(crate) trait Clock: Send + Sync + 'static {
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = Instant> + Send + 'static>>;
}

/// Tokio's task runner, kept behind Urushi's executor vocabulary.
#[cfg_attr(test, allow(dead_code, reason = "wired by the runtime core"))]
pub(crate) struct TokioExecutor {
    handle: tokio::runtime::Handle,
}

impl TokioExecutor {
    #[cfg_attr(test, allow(dead_code, reason = "wired by the runtime core"))]
    pub(crate) fn new(handle: tokio::runtime::Handle) -> Self {
        Self { handle }
    }
}

impl Executor for TokioExecutor {
    fn spawn(&self, task: Task) -> Box<dyn Execution> {
        Box::new(TokioExecution(self.handle.spawn(task)))
    }

    fn spawn_blocking(&self, task: BlockingTask) -> Box<dyn Execution> {
        Box::new(TokioExecution(self.handle.spawn_blocking(task)))
    }
}

#[cfg_attr(test, allow(dead_code, reason = "constructed by TokioExecutor"))]
struct TokioExecution(tokio::task::JoinHandle<()>);

impl Execution for TokioExecution {}

impl Drop for TokioExecution {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Whether interpreting an effect asks the runtime core to continue or stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectControl {
    Continue,
    Shutdown,
}

/// Interprets effects and owns every execution whose result is still live.
pub(crate) struct EffectExecutor<Message> {
    executor: Arc<dyn Executor>,
    clock: Arc<dyn Clock>,
    deliveries: DeliveryQueue<Message>,
    ordinary: Vec<Running>,
    latest: HashMap<Key, Running>,
}

impl<Message: Send + 'static> EffectExecutor<Message> {
    pub(crate) fn new(
        executor: Arc<dyn Executor>,
        clock: Arc<dyn Clock>,
        deliveries: DeliveryQueue<Message>,
    ) -> Self {
        Self {
            executor,
            clock,
            deliveries,
            ordinary: Vec::new(),
            latest: HashMap::new(),
        }
    }

    /// Starts every member in source order, unless the tree requests shutdown.
    ///
    /// Shutdown is found before anything starts so a batch containing it cannot
    /// start a sibling merely because that sibling appeared first.
    pub(crate) fn start(&mut self, effect: Effect<Message>) -> EffectControl {
        self.reap_finished();
        if effect.requests_shutdown() {
            self.stop();
            return EffectControl::Shutdown;
        }
        let kind = effect.into_kind();
        self.start_kind(kind);
        self.reap_finished();
        EffectControl::Continue
    }

    /// Suppresses every completion the runtime still knows to be in flight.
    pub(crate) fn stop(&mut self) {
        self.latest.clear();
        self.ordinary.clear();
    }

    fn start_kind(&mut self, kind: EffectKind<Message>) {
        match kind {
            EffectKind::None => {}
            EffectKind::Shutdown => unreachable!("shutdown is handled before effects start"),
            EffectKind::Perform { key, work } => {
                let executor = Arc::clone(&self.executor);
                self.start_one(key, move |completion, finished| {
                    executor.spawn_blocking(Box::new(move || {
                        let _finished = FinishOnDrop(finished);
                        completion.complete(work());
                    }))
                });
            }
            EffectKind::Future { key, future } => {
                let executor = Arc::clone(&self.executor);
                self.start_one(key, move |completion, finished| {
                    executor.spawn(Box::pin(async move {
                        let _finished = FinishOnDrop(finished);
                        completion.complete(future.await);
                    }))
                });
            }
            EffectKind::After { key, delay, fire } => {
                let executor = Arc::clone(&self.executor);
                let clock = Arc::clone(&self.clock);
                self.start_one(key, move |completion, finished| {
                    executor.spawn(Box::pin(async move {
                        let _finished = FinishOnDrop(finished);
                        let at = clock.sleep(delay).await;
                        completion.complete(fire(at));
                    }))
                });
            }
            EffectKind::Batch(effects) => {
                for effect in effects {
                    self.start_kind(effect.into_kind());
                }
            }
        }
    }

    fn start_one(
        &mut self,
        key: Option<Key>,
        spawn: impl FnOnce(
            super::delivery::EffectCompletion<Message>,
            Arc<AtomicBool>,
        ) -> Box<dyn Execution>,
    ) {
        if let Some(key) = key {
            // Cancellation must linearize before the replacement can run.
            drop(self.latest.remove(&key));
            let running = Running::start(&self.deliveries, spawn);
            self.latest.insert(key, running);
        } else {
            self.ordinary.push(Running::start(&self.deliveries, spawn));
        }
    }

    fn reap_finished(&mut self) {
        self.ordinary.retain(|running| !running.is_finished());
        self.latest.retain(|_, running| !running.is_finished());
    }
}

struct Running {
    // Drop the permit before the task handle: a blocking task may finish while
    // cancellation is in progress, and its completion must see Canceled first.
    _cancellation: EffectCancellation,
    _execution: Box<dyn Execution>,
    finished: Arc<AtomicBool>,
}

/// Marks a task finished on normal return and while unwinding from its work.
struct FinishOnDrop(Arc<AtomicBool>);

impl Drop for FinishOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

impl Running {
    fn start<Message>(
        deliveries: &DeliveryQueue<Message>,
        spawn: impl FnOnce(
            super::delivery::EffectCompletion<Message>,
            Arc<AtomicBool>,
        ) -> Box<dyn Execution>,
    ) -> Self {
        let (completion, cancellation) = deliveries.latest_completion();
        let finished = Arc::new(AtomicBool::new(false));
        let execution = spawn(completion, Arc::clone(&finished));
        Self {
            _cancellation: cancellation,
            _execution: execution,
            finished,
        }
    }

    fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

/// Starts a source and owns whatever must be dropped to stop it.
pub(crate) trait SourceSpawner<Message>: Send + Sync + 'static {
    fn start(
        &self,
        source: Source<Message>,
        deliveries: DeliveryQueue<Message>,
    ) -> Box<dyn RunningSource<Message>>;
}

/// One running source. Refreshing changes its declaration without restarting it.
pub(crate) trait RunningSource<Message>: Send {
    fn refresh(&mut self, source: Source<Message>);
}

/// Reconciles declarations against the exact keys currently running.
pub(crate) struct SubscriptionExecutor<Message> {
    spawner: Arc<dyn SourceSpawner<Message>>,
    deliveries: DeliveryQueue<Message>,
    running: HashMap<Key, Box<dyn RunningSource<Message>>>,
}

impl<Message: Send + 'static> SubscriptionExecutor<Message> {
    pub(crate) fn new(
        spawner: Arc<dyn SourceSpawner<Message>>,
        deliveries: DeliveryQueue<Message>,
    ) -> Self {
        Self {
            spawner,
            deliveries,
            running: HashMap::new(),
        }
    }

    pub(crate) fn reconcile(&mut self, subscription: Subscription<Message>) {
        let desired = latest_declarations(subscription.into_sources());
        let desired_keys: HashSet<Key> = desired.iter().map(|source| source.key).collect();
        self.running.retain(|key, _| desired_keys.contains(key));

        for source in desired {
            if let Some(running) = self.running.get_mut(&source.key) {
                running.refresh(source);
            } else {
                let key = source.key;
                let running = self.spawner.start(source, self.deliveries.clone());
                self.running.insert(key, running);
            }
        }
    }

    pub(crate) fn stop(&mut self) {
        self.running.clear();
    }
}

/// Keeps the last declaration for a duplicate key, ordered by those surviving
/// declarations' positions. This is the same "latest declaration wins" rule
/// reconciliation applies to an already running source.
fn latest_declarations<Message>(sources: Vec<Source<Message>>) -> Vec<Source<Message>> {
    let mut seen = HashSet::new();
    let mut declarations: Vec<_> = sources
        .into_iter()
        .rev()
        .filter(|source| seen.insert(source.key))
        .collect();
    declarations.reverse();
    declarations
}

#[cfg(test)]
#[path = "executor_tests.rs"]
mod tests;
