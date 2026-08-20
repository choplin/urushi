use std::io;
use std::time::{Duration, Instant};

use urushi::Key;

use super::*;
use crate::runtime::delivery::Policy;
use crate::runtime::source::{FocusChange, KeyCode, KeyEvent};
use crate::runtime::testing::{Collector, Ready, block_on, drain};

#[derive(Debug, PartialEq, Eq)]
enum Child {
    Input(Input),
    Tick,
    Surface(Surface),
    Signal(Signal),
    Failed(String),
    Line(usize),
}

#[derive(Debug, PartialEq, Eq)]
enum Parent {
    Child(Child),
}

fn keys<Message>(subscription: &Subscription<Message>) -> Vec<Key> {
    subscription
        .sources
        .iter()
        .map(|source| source.key)
        .collect()
}

fn names<Message>(subscription: &Subscription<Message>) -> Vec<&'static str> {
    subscription
        .sources
        .iter()
        .map(|source| source.kind.name())
        .collect()
}

fn only<Message>(subscription: Subscription<Message>) -> SourceKind<Message> {
    let mut sources = subscription.into_sources();
    assert_eq!(sources.len(), 1, "this test declares one source");
    sources.remove(0).kind
}

fn a_key_press() -> Input {
    Input::Key(KeyEvent::new(KeyCode::Char('k')))
}

#[test]
fn none_declares_nothing() {
    assert!(keys(&Subscription::<Child>::none()).is_empty());
    assert!(keys(&Subscription::<Child>::default()).is_empty());
}

#[test]
fn the_runtimes_own_sources_are_singletons() {
    assert_eq!(
        keys(&Subscription::input(Child::Input)),
        keys(&Subscription::input(|_| Child::Tick)),
    );
    assert_eq!(
        keys(&Subscription::surface(Child::Surface)),
        keys(&Subscription::surface(|_| Child::Tick)),
    );
    assert_ne!(
        keys(&Subscription::input(Child::Input)),
        keys(&Subscription::surface(Child::Surface)),
    );
    assert_ne!(
        keys(&Subscription::<Child>::terminal_errors(|error| {
            Child::Failed(error.to_string())
        })),
        keys(&Subscription::input(Child::Input)),
    );
}

#[test]
fn an_interval_is_identified_by_its_name_and_its_period() {
    let clock = Subscription::interval("clock", Duration::from_secs(1), |_| Child::Tick);
    let same = Subscription::interval("clock", Duration::from_secs(1), |_| Child::Tick);
    let faster = Subscription::interval("clock", Duration::from_millis(500), |_| Child::Tick);
    let poll = Subscription::interval("poll", Duration::from_secs(1), |_| Child::Tick);

    assert_eq!(keys(&clock), keys(&same));
    assert_ne!(
        keys(&clock),
        keys(&faster),
        "a new period restarts the timer"
    );
    assert_ne!(
        keys(&clock),
        keys(&poll),
        "two timers of one period are two timers"
    );
}

#[test]
fn a_signal_source_is_identified_by_its_signal() {
    let term = Subscription::signal(Signal::Terminate, Child::Signal);
    let same = Subscription::signal(Signal::Terminate, Child::Signal);
    let interrupt = Subscription::signal(Signal::Interrupt, Child::Signal);

    assert_eq!(keys(&term), keys(&same));
    assert_ne!(keys(&term), keys(&interrupt));
}

#[test]
fn an_application_source_is_identified_by_the_key_the_application_gives_it() {
    #[derive(Hash)]
    struct Watch(&'static str);

    let root = Subscription::stream(Key::of(&Watch("/src")), Ready::new([Child::Line(1)]));
    let other = Subscription::stream(Key::of(&Watch("/docs")), Ready::new([Child::Line(1)]));
    let named = Subscription::stream("watch", Ready::new([Child::Line(1)]));

    assert_ne!(keys(&root), keys(&other));
    assert_eq!(keys(&named), vec![Key::from("watch")]);
}

#[test]
fn batch_declares_every_source_it_was_given() {
    let subscription = Subscription::batch([
        Subscription::input(Child::Input),
        Subscription::batch([Subscription::interval(
            "clock",
            Duration::from_secs(1),
            |_| Child::Tick,
        )]),
        Subscription::none(),
    ]);

    assert_eq!(names(&subscription), ["input", "interval"]);
}

#[test]
fn an_application_source_is_bounded_unless_it_says_otherwise() {
    let default = only(Subscription::stream("watch", Ready::new([Child::Line(1)])));
    let SourceKind::Stream { admission, .. } = default else {
        panic!("declared a stream")
    };
    assert_eq!(admission, Admission::default());
    assert_eq!(admission.policy(), Policy::Bounded { capacity: 64 });

    let stated = only(Subscription::stream_with(
        "watch",
        Admission::latest(),
        Ready::new([Child::Line(1)]),
    ));
    let SourceKind::Stream { admission, .. } = stated else {
        panic!("declared a stream")
    };
    assert_eq!(admission.policy(), Policy::Latest);
}

#[test]
fn a_bounded_source_always_has_room_for_one() {
    assert_eq!(
        Admission::bounded(0).policy(),
        Policy::Bounded { capacity: 1 }
    );
}

#[test]
fn map_keeps_the_source_and_its_identity() {
    let child = Subscription::input(Child::Input);
    let expected = keys(&child);
    let parent = child.map(Parent::Child);

    assert_eq!(keys(&parent), expected);
    assert_eq!(names(&parent), ["input"]);
}

#[test]
fn map_composes_the_function_of_a_runtime_source() {
    let parent = Subscription::input(Child::Input).map(Parent::Child);
    let SourceKind::Input(map) = only(parent) else {
        panic!("declared input")
    };
    assert_eq!(
        map(a_key_press()),
        Parent::Child(Child::Input(a_key_press()))
    );

    let parent = Subscription::surface(Child::Surface).map(Parent::Child);
    let SourceKind::Surface(map) = only(parent) else {
        panic!("declared surface")
    };
    assert_eq!(
        map(Surface::new(80, 24)),
        Parent::Child(Child::Surface(Surface::new(80, 24)))
    );

    let parent =
        Subscription::interval("clock", Duration::from_secs(1), |_| Child::Tick).map(Parent::Child);
    let SourceKind::Interval { period, map } = only(parent) else {
        panic!("declared interval")
    };
    assert_eq!(period, Duration::from_secs(1));
    assert_eq!(map(Instant::now()), Parent::Child(Child::Tick));

    let parent = Subscription::signal(Signal::Terminate, Child::Signal).map(Parent::Child);
    let SourceKind::Signal { signal, map } = only(parent) else {
        panic!("declared a signal")
    };
    assert_eq!(signal, Signal::Terminate);
    assert_eq!(
        map(Signal::Terminate),
        Parent::Child(Child::Signal(Signal::Terminate))
    );

    let parent =
        Subscription::terminal_errors(|error| Child::Failed(error.to_string())).map(Parent::Child);
    let SourceKind::TerminalErrors(map) = only(parent) else {
        panic!("declared terminal errors")
    };
    assert_eq!(
        map(io::Error::other("broken pipe")),
        Parent::Child(Child::Failed("broken pipe".to_owned()))
    );
}

#[test]
fn map_reaches_the_items_of_a_stream() {
    let parent = Subscription::stream("watch", Ready::new([Child::Line(1), Child::Line(2)]))
        .map(Parent::Child);
    let SourceKind::Stream { mut stream, .. } = only(parent) else {
        panic!("declared a stream")
    };

    assert_eq!(
        drain(stream.as_mut()),
        [Parent::Child(Child::Line(1)), Parent::Child(Child::Line(2))]
    );
}

#[test]
fn map_reaches_what_an_asynchronous_source_sends() {
    let parent = Subscription::run("watch", |sender| async move {
        sender
            .send(Child::Line(1))
            .await
            .expect("the collector takes everything");
        sender
            .send(Child::Line(2))
            .await
            .expect("the collector takes everything");
    })
    .map(Parent::Child);
    let SourceKind::Run { start, .. } = only(parent) else {
        panic!("declared a run")
    };

    let collector = Collector::new();
    block_on(start(collector.sender()));

    assert_eq!(
        collector.take(),
        [Parent::Child(Child::Line(1)), Parent::Child(Child::Line(2))]
    );
}

#[test]
fn map_reaches_what_a_blocking_source_sends() {
    let parent = Subscription::run_blocking("watch", |sender| {
        sender
            .blocking_send(Child::Line(1))
            .expect("the collector takes everything");
    })
    .map(Parent::Child);
    let SourceKind::RunBlocking { start, .. } = only(parent) else {
        panic!("declared a blocking run")
    };

    let collector = Collector::new();
    start(collector.sender());

    assert_eq!(collector.take(), [Parent::Child(Child::Line(1))]);
}

#[test]
fn input_carries_what_the_terminal_session_turned_on() {
    let subscription = Subscription::input(Child::Input);
    let SourceKind::Input(map) = only(subscription) else {
        panic!("declared input")
    };

    assert_eq!(
        map(Input::Paste("pasted".to_owned())),
        Child::Input(Input::Paste("pasted".to_owned()))
    );
    assert_eq!(
        map(Input::Focus(FocusChange::Lost)),
        Child::Input(Input::Focus(FocusChange::Lost))
    );
}

#[test]
fn debug_says_which_sources_are_declared() {
    let subscription = Subscription::batch([
        Subscription::input(Child::Input),
        Subscription::stream("watch", Ready::new([Child::Line(1)])),
    ]);

    let shown = format!("{subscription:?}");
    assert!(shown.contains("kind: \"input\""), "{shown}");
    assert!(shown.contains("Key(watch)"), "{shown}");
}
