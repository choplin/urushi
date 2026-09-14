//! Cross-stage tests: source admission followed by runtime-wide acceptance.

use super::*;
use crate::{Input, KeyCode, KeyEvent, KeyKind};

#[test]
fn accepted_deliveries_keep_one_runtime_wide_order_across_sources() {
    #[derive(Debug, PartialEq, Eq)]
    enum Message {
        Input(u8),
        Subscription(u8),
        Effect(u8),
        Surface(u8),
    }

    let deliveries = DeliveryQueue::new();
    let (input_sender, mut input) = source_inbox(Admission::bounded(4));
    let (subscription_sender, mut subscription) = source_inbox(Admission::bounded(4));
    let (surface_publisher, mut surface) = surface_slot();

    input_sender.blocking_send(Message::Input(1)).unwrap();
    subscription_sender
        .blocking_send(Message::Subscription(1))
        .unwrap();
    input_sender.blocking_send(Message::Input(2)).unwrap();
    assert!(surface_publisher.publish(Message::Surface(1)));

    assert!(input.try_accept(&deliveries));
    deliveries
        .ordinary_completion()
        .complete(Message::Effect(1));
    assert!(subscription.try_accept(&deliveries));
    deliveries
        .ordinary_completion()
        .complete(Message::Effect(2));
    assert!(surface.try_accept(&deliveries));
    assert!(input.try_accept(&deliveries));

    assert_eq!(
        drain(&deliveries),
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
    let deliveries = DeliveryQueue::new();
    let (sender, mut input) = source_inbox(Admission::bounded(4));
    let repeat = Input::Key(KeyEvent {
        code: KeyCode::Down,
        modifiers: crate::Modifiers::NONE,
        kind: KeyKind::Repeat,
    });

    sender.blocking_send(repeat.clone()).unwrap();
    sender.blocking_send(repeat.clone()).unwrap();
    assert!(input.try_accept(&deliveries));
    assert!(input.try_accept(&deliveries));

    assert_eq!(
        drain(&deliveries),
        vec![Delivery::Async(repeat.clone()), Delivery::Async(repeat)]
    );
}

fn drain<Message>(deliveries: &DeliveryQueue<Message>) -> Vec<Delivery<Message>> {
    let mut drained = Vec::new();
    while let Some(delivery) = deliveries.try_next() {
        drained.push(delivery);
    }
    drained
}
