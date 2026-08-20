use std::time::{Duration, Instant};

use urushi::Key;

use super::*;
use crate::runtime::testing::block_on;

#[derive(Debug, PartialEq, Eq)]
enum Child {
    Loaded(usize),
}

#[derive(Debug, PartialEq, Eq)]
enum Parent {
    Child(Child),
}

/// Runs whatever an effect carries and returns the messages it produced, in
/// the order the effect's own structure gives them.
fn drive<Message>(effect: Effect<Message>) -> Vec<Message> {
    match effect.into_kind() {
        EffectKind::None | EffectKind::Shutdown => Vec::new(),
        EffectKind::Perform { work, .. } => vec![work()],
        EffectKind::Future { future, .. } => vec![block_on(future)],
        EffectKind::After { fire, .. } => vec![fire(Instant::now())],
        EffectKind::Batch(effects) => effects.into_iter().flat_map(drive).collect(),
    }
}

fn key_of<Message>(effect: &Effect<Message>) -> Option<Key> {
    match &effect.kind {
        EffectKind::Perform { key, .. }
        | EffectKind::Future { key, .. }
        | EffectKind::After { key, .. } => *key,
        _ => None,
    }
}

#[test]
fn none_carries_no_work() {
    assert!(matches!(
        Effect::<Child>::none().into_kind(),
        EffectKind::None
    ));
    assert!(matches!(
        Effect::<Child>::default().into_kind(),
        EffectKind::None
    ));
}

#[test]
fn shutdown_is_a_kind_of_its_own_rather_than_a_message() {
    assert!(matches!(
        Effect::<Child>::shutdown().into_kind(),
        EffectKind::Shutdown
    ));
}

#[test]
fn one_shot_work_runs_when_the_runtime_runs_it_and_not_before() {
    let effect = Effect::perform(|| Child::Loaded(1));
    assert!(key_of(&effect).is_none());
    assert_eq!(drive(effect), [Child::Loaded(1)]);

    let effect = Effect::future(async { Child::Loaded(2) });
    assert!(key_of(&effect).is_none());
    assert_eq!(drive(effect), [Child::Loaded(2)]);
}

#[test]
fn latest_only_work_carries_the_key_that_replaces_it() {
    #[derive(Hash)]
    enum Pane {
        Preview,
        Chart,
    }

    let preview = Effect::perform_latest(Key::of(&Pane::Preview), || Child::Loaded(1));
    let again = Effect::future_latest(Key::of(&Pane::Preview), async { Child::Loaded(2) });
    let other = Effect::perform_latest(Key::of(&Pane::Chart), || Child::Loaded(3));

    assert_eq!(key_of(&preview), key_of(&again));
    assert_ne!(key_of(&preview), key_of(&other));
    assert_eq!(key_of(&preview), Some(Key::of(&Pane::Preview)));
}

#[test]
fn a_name_is_enough_of_a_key() {
    let effect = Effect::perform_latest("preview", || Child::Loaded(1));

    assert_eq!(key_of(&effect), Some(Key::from("preview")));
}

#[test]
fn batch_keeps_every_member() {
    let effect = Effect::batch([
        Effect::perform(|| Child::Loaded(1)),
        Effect::future(async { Child::Loaded(2) }),
        Effect::none(),
    ]);

    assert_eq!(drive(effect), [Child::Loaded(1), Child::Loaded(2)]);
}

#[test]
fn map_passes_the_message_through_without_touching_the_work() {
    let effect = Effect::perform(|| Child::Loaded(1)).map(Parent::Child);
    assert_eq!(drive(effect), [Parent::Child(Child::Loaded(1))]);

    let effect = Effect::future(async { Child::Loaded(2) }).map(Parent::Child);
    assert_eq!(drive(effect), [Parent::Child(Child::Loaded(2))]);
}

#[test]
fn map_reaches_every_member_of_a_batch() {
    let effect = Effect::batch([
        Effect::perform(|| Child::Loaded(1)),
        Effect::batch([Effect::future(async { Child::Loaded(2) })]),
    ])
    .map(Parent::Child);

    assert_eq!(
        drive(effect),
        [
            Parent::Child(Child::Loaded(1)),
            Parent::Child(Child::Loaded(2))
        ]
    );
}

#[test]
fn map_keeps_a_shutdown_a_shutdown_and_a_key_a_key() {
    let effect = Effect::<Child>::shutdown().map(Parent::Child);
    assert!(matches!(effect.into_kind(), EffectKind::Shutdown));

    let effect = Effect::perform_latest("preview", || Child::Loaded(1)).map(Parent::Child);
    assert_eq!(key_of(&effect), Some(Key::from("preview")));
}

#[test]
fn debug_says_the_shape_without_the_work() {
    assert_eq!(format!("{:?}", Effect::<Child>::none()), "Effect::none");
    assert_eq!(
        format!("{:?}", Effect::<Child>::shutdown()),
        "Effect::shutdown"
    );
    assert_eq!(
        format!(
            "{:?}",
            Effect::perform_latest("preview", || Child::Loaded(1))
        ),
        "Effect::perform { key: Some(Key(preview)) }"
    );
    assert_eq!(
        format!(
            "{:?}",
            Effect::batch([Effect::perform(|| Child::Loaded(1))])
        ),
        "[Effect::perform { key: None }]"
    );
}

#[test]
fn a_delayed_message_carries_its_wait_and_fires_once() {
    let effect = Effect::after(Duration::from_millis(300), |_| Child::Loaded(1));

    let EffectKind::After { key, delay, fire } = effect.into_kind() else {
        panic!("built a delayed effect")
    };
    assert_eq!(key, None);
    assert_eq!(delay, Duration::from_millis(300));
    assert_eq!(fire(Instant::now()), Child::Loaded(1));
}

#[test]
fn a_keyed_delay_is_the_same_timer_again_which_is_what_debouncing_needs() {
    let first = Effect::after_latest("search", Duration::from_millis(300), |_| Child::Loaded(1));
    let second = Effect::after_latest("search", Duration::from_millis(300), |_| Child::Loaded(2));
    let other = Effect::after_latest("preview", Duration::from_millis(300), |_| Child::Loaded(3));

    assert_eq!(key_of(&first), key_of(&second));
    assert_ne!(key_of(&first), key_of(&other));
    assert_eq!(key_of(&first), Some(Key::from("search")));
}

#[test]
fn map_passes_a_delayed_message_through_and_keeps_the_wait() {
    let effect = Effect::after_latest("search", Duration::from_millis(300), |_| Child::Loaded(1))
        .map(Parent::Child);

    assert_eq!(key_of(&effect), Some(Key::from("search")));
    let EffectKind::After { delay, fire, .. } = effect.into_kind() else {
        panic!("built a delayed effect")
    };
    assert_eq!(delay, Duration::from_millis(300));
    assert_eq!(fire(Instant::now()), Parent::Child(Child::Loaded(1)));
}
