//! Deliveries after admission has given them one runtime-wide order.
//!
//! Every clone shares one mutex-protected FIFO of accepted [`Delivery`] values.
//! Insertion under that mutex is the global acceptance linearization point, so
//! insertion order is the order the runtime core observes. The queue never
//! examines `Message`; scheduling depends only on the [`Delivery`] variant.
//!
//! The runtime can wait in either execution model. [`Next`] stores one async
//! receiver Waker while the FIFO is empty; [`DeliveryQueue::blocking_next`]
//! waits on the `available` condition variable. Insertion removes the stored
//! Waker and notification signals both mechanisms after the queue mutex has
//! been released.
//!
//! Some producers must keep their own state lock until insertion has fixed the
//! delivery's global position. They call [`DeliveryQueue::enqueue`], release
//! their state lock, then call [`DeliveryQueue::notify`]. Producers without an
//! upstream lock use [`DeliveryQueue::accept`], which performs both steps.
//! Latest-only effects similarly hold their freshness mutex through insertion,
//! record `Accepted`, release it, and only then notify the runtime.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

/// One position in the runtime-wide order.
///
/// Ordinary deliveries carry exactly one message. Only a rendering-environment
/// barrier may carry a batch, and requiring its first message makes an empty
/// barrier unrepresentable.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Delivery<Message> {
    Async(Message),
    Sync { first: Message, rest: Vec<Message> },
}

impl<Message> Delivery<Message> {
    /// Makes a render barrier while requiring its first message at the call site.
    pub(crate) fn sync(first: Message, rest: impl IntoIterator<Item = Message>) -> Self {
        Self::Sync {
            first,
            rest: rest.into_iter().collect(),
        }
    }
}

/// The single accepted order shared by every runtime source.
pub(crate) struct DeliveryQueue<Message> {
    shared: Arc<Shared<Message>>,
}

struct Shared<Message> {
    state: Mutex<State<Message>>,
    available: Condvar,
}

struct State<Message> {
    accepted: VecDeque<Delivery<Message>>,
    receiver_waker: Option<Waker>,
}

impl<Message> DeliveryQueue<Message> {
    pub(crate) fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    accepted: VecDeque::new(),
                    receiver_waker: None,
                }),
                available: Condvar::new(),
            }),
        }
    }

    /// Linearizes one accepted delivery at the tail of the global order.
    pub(super) fn accept(&self, delivery: Delivery<Message>) {
        let receiver_waker = self.enqueue(delivery);
        self.notify(receiver_waker);
    }

    /// Appends while the caller may still hold a producer-local state lock.
    ///
    /// Returning the waker separates the state transition from notification:
    /// the caller can release every lock before scheduler code runs.
    pub(super) fn enqueue(&self, delivery: Delivery<Message>) -> Option<Waker> {
        let mut state = lock(&self.shared.state);
        state.accepted.push_back(delivery);
        state.receiver_waker.take()
    }

    pub(super) fn notify(&self, receiver_waker: Option<Waker>) {
        self.shared.available.notify_one();
        wake(receiver_waker);
    }

    /// Accepts one ordinary effect completion as an Async delivery.
    fn accept_effect(&self, message: Message) {
        self.accept(Delivery::Async(message));
    }

    /// Removes the earliest accepted delivery without waiting.
    pub(crate) fn try_next(&self) -> Option<Delivery<Message>> {
        lock(&self.shared.state).accepted.pop_front()
    }

    /// Waits without choosing an executor for the earliest accepted delivery.
    pub(crate) fn next(&self) -> Next<'_, Message> {
        Next { queue: self }
    }

    /// Blocks the calling thread for the earliest accepted delivery.
    pub(crate) fn blocking_next(&self) -> Delivery<Message> {
        let mut state = lock(&self.shared.state);
        loop {
            if let Some(delivery) = state.accepted.pop_front() {
                return delivery;
            }
            state = wait(&self.shared.available, state);
        }
    }

    /// A completion whose result is always accepted.
    pub(crate) fn ordinary_completion(&self) -> EffectCompletion<Message> {
        EffectCompletion {
            queue: self.clone(),
            freshness: None,
        }
    }

    /// A completion paired with the handle that replacement drops or cancels.
    pub(crate) fn latest_completion(&self) -> (EffectCompletion<Message>, EffectCancellation) {
        let freshness = Arc::new(Mutex::new(Freshness::Pending));
        (
            EffectCompletion {
                queue: self.clone(),
                freshness: Some(Arc::clone(&freshness)),
            },
            EffectCancellation { freshness },
        )
    }
}

impl<Message> Clone for DeliveryQueue<Message> {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl<Message> Default for DeliveryQueue<Message> {
    fn default() -> Self {
        Self::new()
    }
}

/// Executor-neutral wait for the next globally accepted delivery.
pub(crate) struct Next<'a, Message> {
    queue: &'a DeliveryQueue<Message>,
}

impl<Message> Future for Next<'_, Message> {
    type Output = Delivery<Message>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = lock(&self.queue.shared.state);
        if let Some(delivery) = state.accepted.pop_front() {
            Poll::Ready(delivery)
        } else {
            replace_waker(&mut state.receiver_waker, context.waker());
            Poll::Pending
        }
    }
}

/// The receiving end of one effect execution.
pub(crate) struct EffectCompletion<Message> {
    queue: DeliveryQueue<Message>,
    freshness: Option<Arc<Mutex<Freshness>>>,
}

/// The replacement handle paired with a latest-only effect completion.
pub(crate) struct EffectCancellation {
    freshness: Arc<Mutex<Freshness>>,
}

/// State under the completion/cancellation linearization lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Freshness {
    Pending,
    Accepted,
    Canceled,
}

impl<Message> EffectCompletion<Message> {
    /// Accepts the result unless replacement linearized first.
    ///
    /// The freshness lock remains held through queue insertion. Consequently
    /// cancellation and acceptance have one total order: a canceled completion
    /// is suppressed, while an already accepted completion is never removed
    /// retroactively.
    pub(crate) fn complete(self, message: Message) -> CompletionOutcome {
        let Some(freshness) = self.freshness else {
            self.queue.accept_effect(message);
            return CompletionOutcome::Accepted;
        };

        let mut freshness = lock(&freshness);
        let receiver_waker = match *freshness {
            Freshness::Pending => {
                let receiver_waker = self.queue.enqueue(Delivery::Async(message));
                *freshness = Freshness::Accepted;
                receiver_waker
            }
            Freshness::Canceled => return CompletionOutcome::Suppressed,
            Freshness::Accepted => unreachable!("a completion is consumed when it is accepted"),
        };

        // A Waker may immediately inspect or cancel this execution, so publish
        // the accepted state and release its lock before notification.
        drop(freshness);
        self.queue.notify(receiver_waker);
        CompletionOutcome::Accepted
    }
}

impl EffectCancellation {
    pub(crate) fn cancel(&self) {
        let mut freshness = lock(&self.freshness);
        if *freshness == Freshness::Pending {
            *freshness = Freshness::Canceled;
        }
    }
}

/// Dropping the execution handle is the replacement signal the design gives
/// the runtime, so drop and explicit cancellation have identical semantics.
impl Drop for EffectCancellation {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompletionOutcome {
    Accepted,
    Suppressed,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn wait<'a, T>(condition: &Condvar, guard: MutexGuard<'a, T>) -> MutexGuard<'a, T> {
    condition
        .wait(guard)
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn wake(waker: Option<Waker>) {
    if let Some(waker) = waker {
        waker.wake();
    }
}

fn replace_waker(slot: &mut Option<Waker>, waker: &Waker) {
    if slot
        .as_ref()
        .is_none_or(|current| !current.will_wake(waker))
    {
        *slot = Some(waker.clone());
    }
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
