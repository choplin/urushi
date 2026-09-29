use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::{Context, Poll, Wake, Waker};

use super::*;

#[test]
fn latest_only_cancellation_suppresses_only_the_completion_the_runtime_knows() {
    let deliveries = DeliveryQueue::new();
    let ordinary = deliveries.ordinary_completion();
    let (replaced, cancellation) = deliveries.latest_completion();

    cancellation.cancel();
    assert_eq!(replaced.complete("replaced"), CompletionOutcome::Suppressed);
    assert_eq!(
        ordinary.complete("ordinary stale"),
        CompletionOutcome::Accepted
    );

    assert_eq!(
        deliveries.try_next(),
        Some(Delivery::Async("ordinary stale"))
    );
    assert!(deliveries.try_next().is_none());
}

#[test]
fn a_completion_accepted_before_replacement_stays_in_the_global_order() {
    let deliveries = DeliveryQueue::new();
    let (completion, cancellation) = deliveries.latest_completion();

    assert_eq!(completion.complete(1), CompletionOutcome::Accepted);
    cancellation.cancel();

    assert_eq!(deliveries.try_next(), Some(Delivery::Async(1)));
}

#[test]
fn latest_completion_releases_its_freshness_lock_before_waking_the_runtime() {
    let deliveries = DeliveryQueue::new();
    let (completion, cancellation) = deliveries.latest_completion();
    let lock_was_available = Arc::new(AtomicBool::new(false));
    let waker = Waker::from(Arc::new(FreshnessProbe {
        freshness: Arc::clone(&cancellation.freshness),
        lock_was_available: Arc::clone(&lock_was_available),
    }));
    let mut next = Box::pin(deliveries.next());
    assert_eq!(
        next.as_mut().poll(&mut Context::from_waker(&waker)),
        Poll::Pending
    );

    assert_eq!(completion.complete(1), CompletionOutcome::Accepted);

    assert!(lock_was_available.load(Ordering::Relaxed));
}

#[test]
fn a_sync_delivery_preserves_its_non_empty_message_order() {
    let deliveries = DeliveryQueue::new();
    deliveries.accept(Delivery::sync(1, [2, 3]));

    assert_eq!(
        deliveries.try_next(),
        Some(Delivery::Sync {
            first: 1,
            rest: vec![2, 3]
        })
    );
}

#[test]
fn an_accepted_sync_fences_draws_until_the_runtime_completes_it() {
    let deliveries = DeliveryQueue::new();
    assert!(deliveries.sync_fence_allows_draw());

    deliveries.accept(Delivery::sync(1, []));
    assert!(!deliveries.sync_fence_allows_draw());

    assert!(matches!(deliveries.try_next(), Some(Delivery::Sync { .. })));
    assert!(!deliveries.sync_fence_allows_draw());

    deliveries.complete_sync();
    assert!(deliveries.sync_fence_allows_draw());
}

#[test]
fn the_queue_wakes_an_async_or_blocking_runtime_driver() {
    let deliveries = DeliveryQueue::new();
    let mut next = Box::pin(deliveries.next());
    let (poll, wakes) = poll_once(next.as_mut());
    assert!(matches!(poll, Poll::Pending));

    deliveries.accept(Delivery::Async(9));
    assert_eq!(wakes.load(Ordering::Relaxed), 1);
    let delivery = match poll_with(next.as_mut(), &wakes) {
        Poll::Ready(delivery) => delivery,
        Poll::Pending => panic!("the accepted delivery must be ready"),
    };
    assert_eq!(delivery, Delivery::Async(9));
    drop(next);

    let blocking_queue = DeliveryQueue::new();
    let producer = blocking_queue.clone();
    let joined = std::thread::spawn(move || blocking_queue.blocking_next());
    producer.accept(Delivery::Async(10));

    assert_eq!(joined.join().unwrap(), Delivery::Async(10));
}

fn poll_once<F: Future>(future: Pin<&mut F>) -> (Poll<F::Output>, Arc<AtomicUsize>) {
    let wakes = Arc::new(AtomicUsize::new(0));
    let poll = poll_with(future, &wakes);
    (poll, wakes)
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

struct FreshnessProbe {
    freshness: Arc<std::sync::Mutex<Freshness>>,
    lock_was_available: Arc<AtomicBool>,
}

impl Wake for FreshnessProbe {
    fn wake(self: Arc<Self>) {
        self.lock_was_available
            .store(self.freshness.try_lock().is_ok(), Ordering::Relaxed);
    }
}
