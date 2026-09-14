use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use super::*;

#[test]
fn publication_replaces_only_the_unaccepted_surface_observation() {
    let deliveries = DeliveryQueue::new();
    let (publisher, mut surface) = surface_slot();

    assert!(publisher.publish("old"));
    assert!(publisher.publish("new"));
    assert!(surface.try_accept(&deliveries));
    assert_eq!(
        deliveries.try_next(),
        Some(Delivery::Sync {
            first: "new",
            rest: Vec::new()
        })
    );

    assert!(publisher.publish("later"));
    assert!(surface.try_accept(&deliveries));
    assert_eq!(
        deliveries.try_next(),
        Some(Delivery::Sync {
            first: "later",
            rest: Vec::new()
        })
    );
}

#[test]
fn publication_wakes_readiness_and_closure_finishes_it() {
    let (publisher, mut surface) = surface_slot();
    let mut ready = Box::pin(surface.ready());
    let (poll, wakes) = poll_once(ready.as_mut());
    assert_eq!(poll, Poll::Pending);

    assert!(publisher.publish(7));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    assert_eq!(poll_with(ready.as_mut(), &wakes), Poll::Ready(true));
    drop(ready);

    let deliveries = DeliveryQueue::new();
    assert!(surface.try_accept(&deliveries));
    drop(publisher);
    assert_eq!(poll_new(surface.ready()), Poll::Ready(false));
}

#[test]
fn dropping_the_runtime_side_stops_publication() {
    let (publisher, surface) = surface_slot();
    drop(surface);

    assert!(!publisher.publish(1));
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
