//! Source-local state before runtime-wide acceptance.
//!
//! One [`SourceInbox`] belongs to one running application-defined source. A
//! [`Sender`] and the inbox share an `Arc<Shared<_>>`. The mutex inside it
//! protects the unaccepted FIFO, both endpoint-open flags, and every registered
//! async Waker. Every accepted value becomes one [`Delivery::Async`].
//!
//! Sending and waiting work as follows:
//!
//! - bounded admission appends below capacity; an asynchronous sender registers
//!   its Waker when full, while a blocking sender waits on `space_available`;
//! - latest admission replaces the sole unaccepted value and never waits for
//!   capacity;
//! - [`Ready`] stores the runtime task's Waker while the FIFO is empty and a
//!   sender is still open;
//! - dropping the last sender marks the producer side closed and wakes
//!   [`Ready`]; dropping the inbox marks the receiver side closed, clears
//!   unaccepted values, and releases every waiting sender.
//!
//! [`SourceInbox::try_accept`] holds the inbox mutex while it removes the oldest
//! value and appends that value to the [`DeliveryQueue`]. This is the acceptance
//! linearization point: a concurrent latest send either replaces the value
//! first or observes that it already has a global queue position. Every path
//! that needs both locks takes the inbox lock before the delivery-queue lock;
//! the queue never locks an inbox. Both locks are released before queue,
//! asynchronous-sender, or blocking-sender notifications run.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

use super::admission::{Admission, BoxSend, Policy, SendError, Sender, Sink};
use super::queue::{Delivery, DeliveryQueue};

/// The runtime-owned end of one source's admission boundary.
pub(crate) struct SourceInbox<Message> {
    shared: Arc<Shared<Message>>,
}

/// Synchronously closes the runtime end even when its acceptance task has not
/// yet observed cancellation.
pub(crate) struct SourceInboxCloser<Message> {
    shared: Arc<Shared<Message>>,
}

struct InboxSink<Message> {
    shared: Arc<Shared<Message>>,
}

struct Shared<Message> {
    policy: Policy,
    state: Mutex<State<Message>>,
    space_available: Condvar,
}

/// State protected by the source-local admission lock.
struct State<Message> {
    unaccepted: VecDeque<Message>,
    receiver_open: bool,
    sender_open: bool,
    receiver_waker: Option<Waker>,
    sender_wakers: Vec<(usize, Waker)>,
    next_sender_waiter: usize,
}

/// Builds both ends of an application-defined asynchronous source.
pub(crate) fn source_inbox<Message: Send + 'static>(
    admission: Admission,
) -> (Sender<Message>, SourceInbox<Message>) {
    let shared = Arc::new(Shared {
        policy: admission.policy(),
        state: Mutex::new(State {
            unaccepted: VecDeque::new(),
            receiver_open: true,
            sender_open: true,
            receiver_waker: None,
            sender_wakers: Vec::new(),
            next_sender_waiter: 0,
        }),
        space_available: Condvar::new(),
    });
    let sink = Arc::new(InboxSink {
        shared: Arc::clone(&shared),
    });
    (Sender::new(sink), SourceInbox { shared })
}

impl<Message: Send + 'static> Sink<Message> for InboxSink<Message> {
    fn send<'a>(&'a self, message: Message) -> BoxSend<'a> {
        Box::pin(SendFuture {
            shared: &self.shared,
            message: Some(message),
            waiter_id: None,
        })
    }

    fn blocking_send(&self, message: Message) -> Result<(), SendError> {
        let mut message = Some(message);
        let mut state = lock(&self.shared.state);
        loop {
            match admit(self.shared.policy, &mut state, &mut message) {
                AdmissionResult::Admitted(receiver_waker) => {
                    drop(state);
                    wake(receiver_waker);
                    return Ok(());
                }
                AdmissionResult::Closed => return Err(SendError),
                AdmissionResult::Full => {
                    state = wait(&self.shared.space_available, state);
                }
            }
        }
    }
}

/// Dropping the last source-side handle closes the producer side while leaving
/// already queued values available for acceptance.
impl<Message> Drop for InboxSink<Message> {
    fn drop(&mut self) {
        let receiver_waker = {
            let mut state = lock(&self.shared.state);
            state.sender_open = false;
            state.receiver_waker.take()
        };
        wake(receiver_waker);
    }
}

struct SendFuture<'a, Message> {
    shared: &'a Shared<Message>,
    message: Option<Message>,
    waiter_id: Option<usize>,
}

// The future never projects a pinned reference to `message`, so moving even a
// `!Unpin` message together with the future is safe.
impl<Message> Unpin for SendFuture<'_, Message> {}

impl<Message: Send> Future for SendFuture<'_, Message> {
    type Output = Result<(), SendError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut state = lock(&this.shared.state);
        match admit(this.shared.policy, &mut state, &mut this.message) {
            AdmissionResult::Admitted(receiver_waker) => {
                remove_waiter(&mut state.sender_wakers, this.waiter_id.take());
                drop(state);
                wake(receiver_waker);
                Poll::Ready(Ok(()))
            }
            AdmissionResult::Closed => {
                remove_waiter(&mut state.sender_wakers, this.waiter_id.take());
                Poll::Ready(Err(SendError))
            }
            AdmissionResult::Full => {
                let State {
                    sender_wakers,
                    next_sender_waiter,
                    ..
                } = &mut *state;
                register_waiter(
                    sender_wakers,
                    next_sender_waiter,
                    &mut this.waiter_id,
                    context.waker(),
                );
                Poll::Pending
            }
        }
    }
}

/// A canceled `send().await` must not leave a stale waker behind. Besides
/// retaining resources, such a waker could consume the only notification meant
/// for a later sender.
impl<Message> Drop for SendFuture<'_, Message> {
    fn drop(&mut self) {
        if self.waiter_id.is_none() {
            return;
        }
        let mut state = lock(&self.shared.state);
        remove_waiter(&mut state.sender_wakers, self.waiter_id.take());
    }
}

enum AdmissionResult {
    Admitted(Option<Waker>),
    Full,
    Closed,
}

/// Applies one source's admission policy while holding its state lock.
fn admit<Message>(
    policy: Policy,
    state: &mut State<Message>,
    message: &mut Option<Message>,
) -> AdmissionResult {
    if !state.receiver_open {
        return AdmissionResult::Closed;
    }

    match policy {
        Policy::Bounded { capacity } if state.unaccepted.len() >= capacity => AdmissionResult::Full,
        Policy::Bounded { .. } => {
            state
                .unaccepted
                .push_back(message.take().expect("a pending send retains its message"));
            AdmissionResult::Admitted(state.receiver_waker.take())
        }
        Policy::Latest => {
            let message = message.take().expect("a pending send retains its message");
            if let Some(unaccepted) = state.unaccepted.back_mut() {
                *unaccepted = message;
            } else {
                state.unaccepted.push_back(message);
            }
            AdmissionResult::Admitted(state.receiver_waker.take())
        }
    }
}

impl<Message> SourceInbox<Message> {
    pub(crate) fn closer(&self) -> SourceInboxCloser<Message> {
        SourceInboxCloser {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Waits until a value can be accepted, or returns `false` when the source
    /// has ended and no value remains.
    ///
    /// Readiness does not remove the value. The runtime follows it with
    /// [`try_accept`](Self::try_accept), which owns the acceptance transition.
    pub(crate) fn ready(&mut self) -> Ready<'_, Message> {
        Ready { inbox: self }
    }

    /// Atomically moves the oldest admitted value into the runtime-wide order.
    ///
    /// The inbox stays locked through insertion into `deliveries`. Therefore a
    /// latest sender either replaces the waiting value before this operation or
    /// sees that the value has already received its global position after it.
    pub(crate) fn try_accept(&mut self, deliveries: &DeliveryQueue<Message>) -> bool {
        let (delivery_waker, sender_wakers) = {
            let mut state = lock(&self.shared.state);
            let Some(message) = state.unaccepted.pop_front() else {
                return false;
            };
            let delivery_waker = deliveries.enqueue(Delivery::Async(message));
            (delivery_waker, std::mem::take(&mut state.sender_wakers))
        };

        // Wakers may execute arbitrary scheduler code, so no inbox or queue
        // lock remains held when either side is notified.
        deliveries.notify(delivery_waker);
        self.shared.space_available.notify_all();
        sender_wakers
            .into_iter()
            .for_each(|(_, waker)| waker.wake());
        true
    }
}

/// Dropping the runtime side stops both async and blocking senders and discards
/// values the runtime never accepted.
impl<Message> Drop for SourceInbox<Message> {
    fn drop(&mut self) {
        close_receiver(&self.shared);
    }
}

impl<Message> SourceInboxCloser<Message> {
    pub(crate) fn close(&self) {
        close_receiver(&self.shared);
    }
}

fn close_receiver<Message>(shared: &Shared<Message>) {
    let sender_wakers = {
        let mut state = lock(&shared.state);
        state.receiver_open = false;
        state.unaccepted.clear();
        std::mem::take(&mut state.sender_wakers)
    };
    shared.space_available.notify_all();
    sender_wakers
        .into_iter()
        .for_each(|(_, waker)| waker.wake());
}

/// Executor-neutral notification that an inbox is ready for acceptance.
pub(crate) struct Ready<'a, Message> {
    inbox: &'a mut SourceInbox<Message>,
}

impl<Message> Future for Ready<'_, Message> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = lock(&self.inbox.shared.state);
        if !state.unaccepted.is_empty() {
            Poll::Ready(true)
        } else if state.sender_open {
            replace_waker(&mut state.receiver_waker, context.waker());
            Poll::Pending
        } else {
            Poll::Ready(false)
        }
    }
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

fn register_waiter(
    wakers: &mut Vec<(usize, Waker)>,
    next_id: &mut usize,
    waiter_id: &mut Option<usize>,
    waker: &Waker,
) {
    if let Some(id) = *waiter_id
        && let Some((_, current)) = wakers.iter_mut().find(|(current_id, _)| *current_id == id)
    {
        if !current.will_wake(waker) {
            *current = waker.clone();
        }
        return;
    }

    let id = *next_id;
    *next_id = next_id.wrapping_add(1);
    *waiter_id = Some(id);
    wakers.push((id, waker.clone()));
}

fn remove_waiter(wakers: &mut Vec<(usize, Waker)>, waiter_id: Option<usize>) {
    if let Some(id) = waiter_id {
        wakers.retain(|(current_id, _)| *current_id != id);
    }
}

#[cfg(test)]
#[path = "inbox_tests.rs"]
mod tests;
