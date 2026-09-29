//! Coalescing policy for logical view production.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::executor::Clock;

pub(crate) const DEFAULT_MINIMUM_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 30);

enum Phase {
    Idle,
    Drawing,
    CoolingDown {
        timer: Pin<Box<dyn Future<Output = Instant> + Send + 'static>>,
    },
}

/// Decides when the runtime should build the next `View`.
///
/// Invalidation is state, not a queued event: any number of invalidations while
/// a draw is running or cooling down become one next draw from the latest
/// model.
pub(crate) struct DrawScheduler {
    minimum_interval: Duration,
    clock: Arc<dyn Clock>,
    phase: Phase,
    dirty: bool,
}

impl DrawScheduler {
    pub(crate) fn new(minimum_interval: Duration, clock: Arc<dyn Clock>) -> Self {
        Self {
            minimum_interval,
            clock,
            phase: Phase::Idle,
            dirty: false,
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Waits until a draw is ready, admits it, and enters `Drawing`.
    pub(crate) async fn next_draw(&mut self, admit: impl FnOnce() -> bool) {
        match &mut self.phase {
            Phase::CoolingDown { timer } => {
                timer.as_mut().await;
                self.phase = Phase::Idle;
            }
            Phase::Drawing => return std::future::pending().await,
            Phase::Idle => {}
        }

        if !self.dirty || !admit() {
            return std::future::pending().await;
        }
        self.phase = Phase::Drawing;
        self.dirty = false;
    }

    pub(crate) fn draw_completed(&mut self, completed_at: Instant) {
        assert!(matches!(self.phase, Phase::Drawing));
        let until = completed_at + self.minimum_interval;
        let delay = until.saturating_duration_since(self.clock.now());
        self.phase = Phase::CoolingDown {
            timer: self.clock.sleep(delay),
        };
    }

    pub(crate) fn draw_completed_now(&mut self) {
        self.draw_completed(self.clock.now());
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Poll, Waker};

    use urushi_terminal::TerminalSize;

    use super::*;
    use crate::runtime::testing::Harness;

    fn poll_next_draw(scheduler: &mut DrawScheduler, admit: impl FnOnce() -> bool) -> Poll<()> {
        let mut next = Box::pin(scheduler.next_draw(admit));
        next.as_mut().poll(&mut Context::from_waker(Waker::noop()))
    }

    #[test]
    fn idle_invalidation_is_ready_immediately() {
        let harness = Harness::<()>::new(TerminalSize::ZERO);
        let now = harness.clock().now();
        let mut scheduler = DrawScheduler::new(Duration::from_millis(33), harness.clock());

        scheduler.invalidate();

        assert!(poll_next_draw(&mut scheduler, || true).is_ready());
        scheduler.draw_completed(now);
    }

    #[test]
    fn invalidations_during_draw_and_cooldown_coalesce_into_one_draw() {
        let interval = Duration::from_millis(33);
        let harness = Harness::<()>::new(TerminalSize::ZERO);
        let completed_at = harness.clock().now();
        let mut scheduler = DrawScheduler::new(interval, harness.clock());
        scheduler.invalidate();
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());

        scheduler.invalidate();
        scheduler.invalidate();
        scheduler.draw_completed(completed_at);
        scheduler.invalidate();

        harness.advance(interval / 2);
        assert!(poll_next_draw(&mut scheduler, || true).is_pending());
        harness.advance(interval / 2);
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());

        assert!(poll_next_draw(&mut scheduler, || true).is_pending());
    }

    #[test]
    fn a_clean_scheduler_becomes_idle_when_cooldown_expires() {
        let interval = Duration::from_millis(33);
        let harness = Harness::<()>::new(TerminalSize::ZERO);
        let completed_at = harness.clock().now();
        let mut scheduler = DrawScheduler::new(interval, harness.clock());
        scheduler.invalidate();
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());
        scheduler.draw_completed(completed_at);

        harness.advance(interval);
        assert!(poll_next_draw(&mut scheduler, || true).is_pending());

        scheduler.invalidate();
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());
    }

    #[test]
    fn completion_now_uses_the_scheduler_clock() {
        let interval = Duration::from_millis(33);
        let harness = Harness::<()>::new(TerminalSize::ZERO);
        let mut scheduler = DrawScheduler::new(interval, harness.clock());
        scheduler.invalidate();
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());

        harness.advance(Duration::from_secs(3_600));
        scheduler.draw_completed_now();
        scheduler.invalidate();
        assert!(poll_next_draw(&mut scheduler, || true).is_pending());

        harness.advance(interval);
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());
    }

    #[test]
    fn denied_admission_does_not_partially_start_a_draw() {
        let harness = Harness::<()>::new(TerminalSize::ZERO);
        let mut scheduler = DrawScheduler::new(Duration::from_millis(33), harness.clock());
        scheduler.invalidate();

        assert!(poll_next_draw(&mut scheduler, || false).is_pending());
        assert!(poll_next_draw(&mut scheduler, || true).is_ready());
    }
}
