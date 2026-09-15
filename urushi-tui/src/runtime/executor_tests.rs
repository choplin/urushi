use std::sync::Arc;
use std::time::Duration;

use super::{EffectControl, EffectExecutor, RunningSource, SourceSpawner, SubscriptionExecutor};
use crate::runtime::delivery::{Delivery, DeliveryQueue};
use crate::runtime::subscription::Source;
use crate::runtime::testing::{EffectEvent, Harness, Ready};
use crate::{Effect, Subscription};
use urushi_terminal::TerminalSize;

#[test]
fn one_shot_effects_are_scheduled_in_source_order_and_delivered_as_they_complete() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    assert_eq!(
        effects.start(Effect::batch([
            Effect::perform(|| 1),
            Effect::perform(|| 2),
            Effect::future(async { 3 }),
        ])),
        EffectControl::Continue
    );
    assert_eq!(
        harness.effect_events(),
        vec![
            EffectEvent::Started { id: 0 },
            EffectEvent::Started { id: 1 },
            EffectEvent::Started { id: 2 },
        ]
    );

    assert!(harness.complete_effect(1));
    assert!(harness.complete_effect(0));
    assert!(harness.complete_effect(2));

    assert_eq!(harness.next(), Some(Delivery::Async(2)));
    assert_eq!(harness.next(), Some(Delivery::Async(1)));
    assert_eq!(harness.next(), Some(Delivery::Async(3)));
    assert_eq!(harness.next(), None);
    assert_eq!(
        &harness.effect_events()[3..],
        &[
            EffectEvent::Completed { id: 1 },
            EffectEvent::Completed { id: 0 },
            EffectEvent::Completed { id: 2 },
        ]
    );
}

#[test]
fn replacement_drops_an_unstarted_future() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::future_latest("preview", async { "old" }));
    effects.start(Effect::future_latest("preview", async { "new" }));
    assert!(harness.complete_effect(1));

    assert_eq!(harness.next(), Some(Delivery::Async("new")));
    assert_eq!(harness.next(), None);
    assert_eq!(
        harness.effect_events(),
        vec![
            EffectEvent::Started { id: 0 },
            EffectEvent::Canceled { id: 0 },
            EffectEvent::Started { id: 1 },
            EffectEvent::Completed { id: 1 },
        ]
    );
}

#[test]
fn replacement_does_not_remove_an_already_accepted_latest_completion() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::future_latest("preview", async { "accepted" }));
    assert!(harness.complete_effect(0));
    effects.start(Effect::future_latest("preview", async { "replacement" }));
    assert!(harness.complete_effect(1));

    assert_eq!(harness.next(), Some(Delivery::Async("accepted")));
    assert_eq!(harness.next(), Some(Delivery::Async("replacement")));
}

#[test]
fn replacement_suppresses_a_running_blocking_completion() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::perform_latest("preview", || "old"));
    assert!(harness.begin_effect(0));
    effects.start(Effect::perform_latest("preview", || "new"));

    // A blocking closure already handed to a worker may finish, but its
    // runtime-known replacement suppresses the completion.
    assert!(harness.complete_effect(0));
    assert!(harness.complete_effect(1));

    assert_eq!(harness.next(), Some(Delivery::Async("new")));
    assert_eq!(harness.next(), None);
    assert_eq!(
        harness.effect_events(),
        vec![
            EffectEvent::Started { id: 0 },
            EffectEvent::Canceled { id: 0 },
            EffectEvent::Started { id: 1 },
            EffectEvent::Completed { id: 0 },
            EffectEvent::Completed { id: 1 },
        ]
    );
}

#[test]
fn ordinary_stale_completion_is_still_delivered() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::perform(|| "ordinary"));
    assert!(harness.begin_effect(0));
    effects.start(Effect::perform_latest("preview", || "latest"));
    assert!(harness.complete_effect(0));
    assert!(harness.complete_effect(1));

    assert_eq!(harness.next(), Some(Delivery::Async("ordinary")));
    assert_eq!(harness.next(), Some(Delivery::Async("latest")));
}

#[test]
fn shutdown_starts_no_sibling_and_suppresses_in_flight_work() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::perform(|| "in flight"));
    assert!(harness.begin_effect(0));
    assert_eq!(
        effects.start(Effect::batch([
            Effect::perform(|| "must not start"),
            Effect::shutdown(),
        ])),
        EffectControl::Shutdown
    );

    assert!(harness.complete_effect(0));
    assert_eq!(harness.next(), None);
    assert_eq!(
        harness.effect_events(),
        vec![
            EffectEvent::Started { id: 0 },
            EffectEvent::Canceled { id: 0 },
            EffectEvent::Completed { id: 0 },
        ]
    );
}

#[test]
fn a_panicked_task_is_reaped_on_the_next_effect() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::perform(|| panic!("effect failed")));
    let panic =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| harness.complete_effect(0)));
    assert!(panic.is_err());

    effects.start(Effect::none());
    assert_eq!(
        harness.effect_events(),
        vec![
            EffectEvent::Started { id: 0 },
            EffectEvent::Canceled { id: 0 },
        ]
    );
}

#[test]
fn delayed_effect_uses_the_runtime_clock() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut effects =
        EffectExecutor::new(harness.executor(), harness.clock(), harness.deliveries());

    effects.start(Effect::after(Duration::from_millis(25), |_| "timer"));
    assert!(!harness.complete_effect(0));
    assert_eq!(harness.next(), None);

    harness.advance(Duration::from_millis(24));
    assert!(!harness.complete_effect(0));
    assert_eq!(harness.next(), None);

    harness.advance(Duration::from_millis(1));
    assert!(harness.complete_effect(0));

    assert_eq!(harness.next(), Some(Delivery::Async("timer")));
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
