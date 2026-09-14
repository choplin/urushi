use std::time::{Duration, Instant};

use urushi::{Align, Available, TextStyle, View, resolve};

use super::*;
use crate::runtime::effect::EffectKind;
use crate::runtime::source::{Input, KeyCode, KeyEvent};
use crate::runtime::testing::block_on;

/// A counter, as small as an application gets: it is a value, its model is a
/// number, and none of the four methods needs a terminal.
struct Counter {
    step: i32,
}

#[derive(Debug, PartialEq, Eq)]
struct Model {
    count: i32,
    counting: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Message {
    Restored(i32),
    Key(Input),
    Tick,
    Stop,
}

impl Application for Counter {
    type Model = Model;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (
            Model {
                count: 0,
                counting: true,
            },
            Effect::perform(|| Message::Restored(7)),
        )
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Restored(count) => {
                model.count = count;
                Effect::none()
            }
            Message::Tick => {
                model.count += self.step;
                Effect::none()
            }
            Message::Key(_) => {
                model.counting = !model.counting;
                Effect::none()
            }
            Message::Stop => Effect::perform_latest("save", || Message::Restored(0)),
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        View::column(
            Align::Left,
            [
                View::text(model.count.to_string(), TextStyle::new()),
                View::text(
                    if model.counting { "counting" } else { "paused" },
                    TextStyle::new(),
                ),
            ],
        )
    }

    fn subscriptions(&self, model: &Self::Model) -> Subscription<Self::Message> {
        let ticking = if model.counting {
            Subscription::interval("clock", Duration::from_secs(1), |_| Message::Tick)
        } else {
            Subscription::none()
        };
        Subscription::batch([Subscription::input(Message::Key), ticking])
    }
}

fn drive(effect: Effect<Message>) -> Vec<Message> {
    match effect.into_kind() {
        EffectKind::None | EffectKind::Shutdown => Vec::new(),
        EffectKind::Perform { work, .. } => vec![work()],
        EffectKind::Future { future, .. } => vec![block_on(future)],
        EffectKind::After { fire, .. } => vec![fire(Instant::now())],
        EffectKind::Batch(effects) => effects.into_iter().flat_map(drive).collect(),
    }
}

fn lines(view: &View) -> Vec<String> {
    resolve(view, Available::NONE)
        .unwrap()
        .rows()
        .iter()
        .map(|row| row.iter().map(|grapheme| grapheme.symbol()).collect())
        .collect()
}

#[test]
fn init_gives_the_first_model_and_the_work_to_start_with_it() {
    let application = Counter { step: 1 };

    let (model, effect) = application.init();

    assert_eq!(
        model,
        Model {
            count: 0,
            counting: true
        }
    );
    assert_eq!(drive(effect), [Message::Restored(7)]);
}

#[test]
fn update_is_the_only_method_that_changes_the_model() {
    let application = Counter { step: 2 };
    let (mut model, _) = application.init();

    application.update(&mut model, Message::Restored(7));
    application.update(&mut model, Message::Tick);
    application.update(&mut model, Message::Tick);

    assert_eq!(
        model,
        Model {
            count: 11,
            counting: true
        }
    );
}

#[test]
fn view_reads_the_model_and_needs_nothing_else() {
    let application = Counter { step: 1 };
    let (mut model, _) = application.init();
    application.update(&mut model, Message::Restored(7));

    assert_eq!(lines(&application.view(&model)), ["7       ", "counting"]);

    application.update(
        &mut model,
        Message::Key(Input::Key(KeyEvent::new(KeyCode::Char(' ')))),
    );

    assert_eq!(lines(&application.view(&model)), ["7     ", "paused"]);
}

#[test]
fn subscriptions_follow_the_model() {
    let application = Counter { step: 1 };
    let (mut model, _) = application.init();

    let counting = application.subscriptions(&model);
    assert_eq!(counting.into_sources().len(), 2);

    application.update(
        &mut model,
        Message::Key(Input::Key(KeyEvent::new(KeyCode::Char(' ')))),
    );

    let paused = application.subscriptions(&model);
    assert_eq!(paused.into_sources().len(), 1);
}

#[test]
fn work_an_update_returns_reaches_update_again_as_a_message() {
    let application = Counter { step: 1 };
    let (mut model, _) = application.init();
    application.update(&mut model, Message::Restored(7));

    let effect = application.update(&mut model, Message::Stop);
    for message in drive(effect) {
        application.update(&mut model, message);
    }

    assert_eq!(model.count, 0);
}
