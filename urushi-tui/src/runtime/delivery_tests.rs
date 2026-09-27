//! Cross-stage tests: source admission followed by runtime-wide acceptance.

use super::*;
use crate::runtime::testing::Harness;
use crate::{Input, KeyCode, KeyEvent, KeyKind};
use urushi_terminal::TerminalSize;

#[test]
fn accepted_deliveries_keep_one_runtime_wide_order_across_sources() {
    #[derive(Debug, PartialEq, Eq)]
    enum Message {
        Input(u8),
        Subscription(u8),
        Effect(u8),
        Surface(u8),
    }

    let harness = Harness::new(TerminalSize::ZERO);
    let mut input = harness.source(Admission::bounded(4));
    let mut subscription = harness.source(Admission::bounded(4));
    let (surface_publisher, mut surface) = surface_slot();

    input.send(Message::Input(1)).unwrap();
    subscription.send(Message::Subscription(1)).unwrap();
    input.send(Message::Input(2)).unwrap();
    assert!(surface_publisher.publish(Message::Surface(1)));

    assert!(harness.accept(&mut input));
    harness.complete(Message::Effect(1));
    assert!(harness.accept(&mut subscription));
    harness.complete(Message::Effect(2));
    assert!(surface.try_accept(&harness.deliveries()));
    assert!(harness.accept(&mut input));

    assert_eq!(
        harness.drain(),
        vec![
            Delivery::Async(Message::Input(1)),
            Delivery::Async(Message::Effect(1)),
            Delivery::Async(Message::Subscription(1)),
            Delivery::Async(Message::Effect(2)),
            Delivery::Sync {
                first: Message::Surface(1),
                rest: Vec::new()
            },
            Delivery::Async(Message::Input(2)),
        ]
    );
}

#[test]
fn bounded_input_admission_keeps_every_key_repeat_separate() {
    let harness = Harness::new(TerminalSize::ZERO);
    let mut input = harness.source(Admission::bounded(4));
    let repeat = Input::Key(KeyEvent::new(KeyCode::Down).with_kind(KeyKind::Repeat));

    input.send(repeat.clone()).unwrap();
    input.send(repeat.clone()).unwrap();
    assert!(harness.accept(&mut input));
    assert!(harness.accept(&mut input));

    assert_eq!(
        harness.drain(),
        vec![Delivery::Async(repeat.clone()), Delivery::Async(repeat)]
    );
}
