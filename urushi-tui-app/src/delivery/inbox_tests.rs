use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use super::*;

#[test]
fn bounded_admission_is_fifo_and_applies_async_backpressure() {
    let deliveries = DeliveryQueue::new();
    let (sender, mut inbox) = source_inbox(Admission::bounded(2));
    assert_eq!(poll_new(sender.send(1)), Poll::Ready(Ok(())));
    assert_eq!(poll_new(sender.send(2)), Poll::Ready(Ok(())));

    let mut waiting = Box::pin(sender.send(3));
    let (poll, wakes) = poll_once(waiting.as_mut());
    assert_eq!(poll, Poll::Pending);

    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some(1));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert_eq!(poll_with(waiting.as_mut(), &wakes), Poll::Ready(Ok(())));
    assert!(inbox.try_accept(&deliveries));
    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some(2));
    assert_eq!(take_one(&deliveries), Some(3));
}

#[test]
fn canceling_a_backpressured_send_removes_its_wake_registration() {
    let deliveries = DeliveryQueue::new();
    let (sender, mut inbox) = source_inbox(Admission::bounded(1));
    sender.blocking_send(1).unwrap();

    let mut canceled = Box::pin(sender.send(2));
    let (poll, canceled_wakes) = poll_once(canceled.as_mut());
    assert_eq!(poll, Poll::Pending);
    drop(canceled);

    let mut waiting = Box::pin(sender.send(3));
    let (poll, waiting_wakes) = poll_once(waiting.as_mut());
    assert_eq!(poll, Poll::Pending);
    assert!(inbox.try_accept(&deliveries));

    assert_eq!(canceled_wakes.load(Ordering::Relaxed), 0);
    assert_eq!(waiting_wakes.load(Ordering::Relaxed), 1);
    assert_eq!(
        poll_with(waiting.as_mut(), &waiting_wakes),
        Poll::Ready(Ok(()))
    );
    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some(1));
    assert_eq!(take_one(&deliveries), Some(3));
}

#[test]
fn latest_admission_replaces_only_the_unaccepted_item_and_never_waits() {
    let deliveries = DeliveryQueue::new();
    let (sender, mut inbox) = source_inbox(Admission::latest());

    assert_eq!(poll_new(sender.send("old")), Poll::Ready(Ok(())));
    assert_eq!(poll_new(sender.send("new")), Poll::Ready(Ok(())));
    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some("new"));

    assert_eq!(poll_new(sender.send("later")), Poll::Ready(Ok(())));
    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some("later"));
}

#[test]
fn dropping_the_runtime_side_stops_async_and_blocking_senders() {
    let (sender, inbox) = source_inbox(Admission::bounded(1));
    drop(inbox);

    assert_eq!(poll_new(sender.send(1)), Poll::Ready(Err(SendError)));
    assert_eq!(sender.blocking_send(2), Err(SendError));
}

#[test]
fn readiness_wakes_without_removing_the_unaccepted_value() {
    let deliveries = DeliveryQueue::new();
    let (sender, mut inbox) = source_inbox(Admission::bounded(1));
    let mut ready = Box::pin(inbox.ready());
    let (poll, wakes) = poll_once(ready.as_mut());
    assert_eq!(poll, Poll::Pending);

    assert_eq!(sender.blocking_send(7), Ok(()));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert_eq!(poll_with(ready.as_mut(), &wakes), Poll::Ready(true));
    drop(ready);
    assert!(inbox.try_accept(&deliveries));
    assert_eq!(take_one(&deliveries), Some(7));
}

#[test]
fn readiness_finishes_when_the_source_drops_its_last_sender() {
    let (sender, mut inbox) = source_inbox::<u8>(Admission::bounded(1));
    let mut ready = Box::pin(inbox.ready());
    let (poll, wakes) = poll_once(ready.as_mut());
    assert_eq!(poll, Poll::Pending);

    drop(sender);
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert_eq!(poll_with(ready.as_mut(), &wakes), Poll::Ready(false));
}

fn take_one<Message>(deliveries: &DeliveryQueue<Message>) -> Option<Message> {
    deliveries.try_next().map(|delivery| match delivery {
        Delivery::Async(message) => message,
        Delivery::Sync { .. } => panic!("source was accepted as an ordinary delivery"),
        Delivery::Redraw => panic!("source was accepted as a redraw request"),
    })
}

fn poll_once<F: Future>(future: Pin<&mut F>) -> (Poll<F::Output>, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let poll = poll_with(future, &wakes);
    (poll, wakes)
}

fn poll_new<F: Future>(future: F) -> Poll<F::Output> {
    let mut future = Box::pin(future);
    poll_once(future.as_mut()).0
}

fn poll_with<F: Future>(future: Pin<&mut F>, wakes: &Arc<AtomicUsize>) -> Poll<F::Output> {
    let waker = Waker::from(Arc::new(CountWake(Arc::clone(wakes))));
    future.poll(&mut Context::from_waker(&waker))
}

struct CountWake(Arc<AtomicUsize>);

impl Wake for CountWake {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}
