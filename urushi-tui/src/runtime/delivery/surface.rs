//! Runtime-owned latest slot for logical rendering-environment observations.
//!
//! Surface publication is not an application-defined source: it needs neither
//! the public [`Admission`](super::Admission) vocabulary nor a [`Sender`](super::Sender).
//! The surface source maps each observation with the mapper current when that
//! observation occurs, then publishes the resulting application message here.
//! The publisher and [`SurfaceSlot`] share an `Arc<Shared<_>>`; its mutex
//! protects one pending message, both endpoint-open flags, and the runtime
//! task's Waker.
//!
//! [`SurfaceMessagePublisher::publish`] never waits. It replaces `pending`,
//! takes any registered Waker, releases the mutex, and wakes the runtime.
//! [`Ready`] registers that Waker while no message is pending and the publisher
//! remains open. Dropping either endpoint records closure under the same mutex.
//!
//! [`SurfaceSlot::try_accept`] keeps the slot mutex locked while it takes the
//! pending message and appends [`Delivery::Sync`] to the [`DeliveryQueue`]. A
//! concurrent publication therefore either replaces the message first or
//! becomes the next pending message after the accepted one has a global queue
//! position. The slot and queue locks are released before the queue Waker is
//! invoked.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};

use super::queue::{Delivery, DeliveryQueue};

/// Publishes mapped surface messages into the runtime-owned latest slot.
pub(crate) struct SurfaceMessagePublisher<Message> {
    shared: Arc<Shared<Message>>,
}

/// The event loop's end of the runtime-owned latest slot.
pub(crate) struct SurfaceSlot<Message> {
    shared: Arc<Shared<Message>>,
}

struct Shared<Message> {
    state: Mutex<State<Message>>,
}

struct State<Message> {
    pending: Option<Message>,
    receiver_open: bool,
    publisher_open: bool,
    receiver_waker: Option<Waker>,
}

/// Builds the publishing and accepting ends of the surface path.
pub(crate) fn surface_slot<Message>() -> (SurfaceMessagePublisher<Message>, SurfaceSlot<Message>) {
    let shared = Arc::new(Shared {
        state: Mutex::new(State {
            pending: None,
            receiver_open: true,
            publisher_open: true,
            receiver_waker: None,
        }),
    });
    (
        SurfaceMessagePublisher {
            shared: Arc::clone(&shared),
        },
        SurfaceSlot { shared },
    )
}

impl<Message> SurfaceMessagePublisher<Message> {
    /// Replaces the unaccepted surface message and wakes the event loop.
    ///
    /// Returns `false` after the runtime has dropped its receiving end.
    pub(crate) fn publish(&self, message: Message) -> bool {
        let receiver_waker = {
            let mut state = lock(&self.shared.state);
            if !state.receiver_open {
                return false;
            }
            state.pending = Some(message);
            state.receiver_waker.take()
        };
        wake(receiver_waker);
        true
    }
}

impl<Message> Drop for SurfaceMessagePublisher<Message> {
    fn drop(&mut self) {
        let receiver_waker = {
            let mut state = lock(&self.shared.state);
            state.publisher_open = false;
            state.receiver_waker.take()
        };
        wake(receiver_waker);
    }
}

impl<Message> SurfaceSlot<Message> {
    /// Waits for a surface message, or returns `false` after the publisher ends
    /// and the final message has been accepted.
    pub(crate) fn ready(&mut self) -> Ready<'_, Message> {
        Ready { slot: self }
    }

    /// Atomically gives the latest surface message a Sync queue position.
    pub(crate) fn try_accept(&mut self, deliveries: &DeliveryQueue<Message>) -> bool {
        let delivery_waker = {
            let mut state = lock(&self.shared.state);
            let Some(message) = state.pending.take() else {
                return false;
            };
            deliveries.enqueue(Delivery::sync(message, []))
        };

        // A Waker may run arbitrary scheduler code, so both locks are released
        // before notification.
        deliveries.notify(delivery_waker);
        true
    }
}

impl<Message> Drop for SurfaceSlot<Message> {
    fn drop(&mut self) {
        let mut state = lock(&self.shared.state);
        state.receiver_open = false;
        state.pending = None;
    }
}

/// Executor-neutral notification that the surface slot is ready.
pub(crate) struct Ready<'a, Message> {
    slot: &'a mut SurfaceSlot<Message>,
}

impl<Message> Future for Ready<'_, Message> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = lock(&self.slot.shared.state);
        if state.pending.is_some() {
            Poll::Ready(true)
        } else if state.publisher_open {
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
#[path = "surface_tests.rs"]
mod tests;
