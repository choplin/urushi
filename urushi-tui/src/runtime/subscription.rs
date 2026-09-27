//! What an application declares it wants to hear from.

use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use futures_core::Stream;
use urushi::Key;

use super::delivery::{Admission, Sender};
use super::effect::Mapper;
use super::source::{Input, Signal, Surface};

/// A stream a source is built from.
pub(crate) type BoxStream<Message> = Pin<Box<dyn Stream<Item = Message> + Send + 'static>>;

/// The body of an asynchronous application-defined source.
pub(crate) type AsyncStart<Message> = Box<dyn FnOnce(Sender<Message>) -> BoxDone + Send + 'static>;

/// The body of a blocking application-defined source.
pub(crate) type BlockingStart<Message> = Box<dyn FnOnce(Sender<Message>) + Send + 'static>;

type BoxDone = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// The sources an application wants to hear from, for as long as it keeps
/// declaring them.
///
/// Nothing reaches `update` from a source the application did not declare —
/// terminal input included. After each `update` the runtime reconciles the
/// declaration against what it is running: a declared source whose [`Key`] is
/// already running keeps running, one whose key is not running starts, and a
/// running one whose key is no longer declared stops.
///
/// The function each constructor takes turns the source's own value into the
/// application's message; the runtime does not know that type and cannot
/// deliver without one. That function is not part of a source's identity, so a
/// source declared again keeps running and passes its messages through the most
/// recent declaration's function from then on.
///
/// ```
/// use std::time::Duration;
///
/// use urushi_tui::{Input, Subscription};
///
/// enum Message {
///     Input(Input),
///     Tick,
/// }
///
/// let subscription = Subscription::batch([
///     Subscription::input(Message::Input),
///     Subscription::interval("clock", Duration::from_secs(1), |_| Message::Tick),
/// ]);
/// ```
pub struct Subscription<Message> {
    sources: Vec<Source<Message>>,
}

/// One declared source: its identity, and what it is.
pub(crate) struct Source<Message> {
    pub(crate) key: Key,
    pub(crate) kind: SourceKind<Message>,
}

/// What a [`Source`] is, as the runtime reads it.
pub(crate) enum SourceKind<Message> {
    Input(Mapper<Input, Message>),
    Surface(Mapper<Surface, Message>),
    Interval {
        period: Duration,
        map: Mapper<Instant, Message>,
    },
    Signal {
        signal: Signal,
        map: Mapper<Signal, Message>,
    },
    TerminalErrors(Mapper<io::Error, Message>),
    Stream {
        admission: Admission,
        stream: BoxStream<Message>,
    },
    Run {
        admission: Admission,
        start: AsyncStart<Message>,
    },
    RunBlocking {
        admission: Admission,
        start: BlockingStart<Message>,
    },
}

/// The identity of a source the runtime provides. Private, so an application's
/// own keys cannot collide with one.
#[derive(Hash)]
enum RuntimeSource {
    Input,
    Surface,
    TerminalErrors,
}

/// The identity of an interval: the application's name for it, and its period.
///
/// The period is part of the identity so that a declaration that changes it
/// restarts the timer; without that, reconciliation would see the running key
/// and keep waiting the old period.
#[derive(Hash)]
struct IntervalKey(Key, Duration);

/// The identity of a signal source: the signal it handles.
#[derive(Hash)]
struct SignalKey(Signal);

impl<Message> Subscription<Message> {
    /// No source.
    pub fn none() -> Self {
        Self {
            sources: Vec::new(),
        }
    }

    /// Terminal key and text input.
    ///
    /// Input is a subscription rather than a method the runtime always calls,
    /// so a program that must not receive input in some state — while an
    /// external editor owns the terminal, say — declares none in that state.
    pub fn input<F>(f: F) -> Self
    where
        F: Fn(Input) -> Message + Send + Sync + 'static,
    {
        Self::one(
            Key::of(&RuntimeSource::Input),
            SourceKind::Input(Arc::new(f)),
        )
    }

    /// Observations of the surface the application draws on.
    pub fn surface<F>(f: F) -> Self
    where
        F: Fn(Surface) -> Message + Send + Sync + 'static,
    {
        Self::one(
            Key::of(&RuntimeSource::Surface),
            SourceKind::Surface(Arc::new(f)),
        )
    }

    /// A timer that fires every `period`, for as long as it is declared.
    ///
    /// An interval is the one runtime source an application can want more than
    /// one of, so it names its own: two timers of one period are two timers
    /// when their keys differ. Declaring the same key with a different period
    /// restarts that timer at the new period.
    ///
    /// A timer that should fire once is [`Effect::after`](super::Effect::after)
    /// rather than an interval the application stops declaring.
    pub fn interval<F>(key: impl Into<Key>, period: Duration, f: F) -> Self
    where
        F: Fn(Instant) -> Message + Send + Sync + 'static,
    {
        Self::one(
            Key::of(&IntervalKey(key.into(), period)),
            SourceKind::Interval {
                period,
                map: Arc::new(f),
            },
        )
    }

    /// One process signal.
    ///
    /// A handler is installed only for a signal that is declared.
    pub fn signal<F>(signal: Signal, f: F) -> Self
    where
        F: Fn(Signal) -> Message + Send + Sync + 'static,
    {
        Self::one(
            Key::of(&SignalKey(signal)),
            SourceKind::Signal {
                signal,
                map: Arc::new(f),
            },
        )
    }

    /// Failures the terminal reports while drawing.
    ///
    /// A draw that fails ends the run unless this is declared. With it, the
    /// failure arrives as a message, the frame that failed is not committed,
    /// and the application decides what to do — ignore it, record it, save and
    /// shut down.
    pub fn terminal_errors<F>(f: F) -> Self
    where
        F: Fn(io::Error) -> Message + Send + Sync + 'static,
    {
        Self::one(
            Key::of(&RuntimeSource::TerminalErrors),
            SourceKind::TerminalErrors(Arc::new(f)),
        )
    }

    /// An application-defined source that is a stream, under the default
    /// admission.
    pub fn stream<S>(key: impl Into<Key>, stream: S) -> Self
    where
        S: Stream<Item = Message> + Send + 'static,
    {
        Self::stream_with(key, Admission::default(), stream)
    }

    /// An application-defined source that is a stream, under `admission`.
    pub fn stream_with<S>(key: impl Into<Key>, admission: Admission, stream: S) -> Self
    where
        S: Stream<Item = Message> + Send + 'static,
    {
        Self::one(
            key.into(),
            SourceKind::Stream {
                admission,
                stream: Box::pin(stream),
            },
        )
    }

    /// An application-defined asynchronous source, under the default admission.
    pub fn run<F, Fut>(key: impl Into<Key>, f: F) -> Self
    where
        F: FnOnce(Sender<Message>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Self::run_with(key, Admission::default(), f)
    }

    /// An application-defined asynchronous source, under `admission`.
    pub fn run_with<F, Fut>(key: impl Into<Key>, admission: Admission, f: F) -> Self
    where
        F: FnOnce(Sender<Message>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let start: AsyncStart<Message> = Box::new(move |sender| Box::pin(f(sender)));
        Self::one(key.into(), SourceKind::Run { admission, start })
    }

    /// An application-defined blocking source, run on its own thread, under the
    /// default admission.
    pub fn run_blocking<F>(key: impl Into<Key>, f: F) -> Self
    where
        F: FnOnce(Sender<Message>) + Send + 'static,
    {
        Self::run_blocking_with(key, Admission::default(), f)
    }

    /// An application-defined blocking source, run on its own thread, under
    /// `admission`.
    pub fn run_blocking_with<F>(key: impl Into<Key>, admission: Admission, f: F) -> Self
    where
        F: FnOnce(Sender<Message>) + Send + 'static,
    {
        Self::one(
            key.into(),
            SourceKind::RunBlocking {
                admission,
                start: Box::new(f),
            },
        )
    }

    /// Several sources.
    pub fn batch(subscriptions: impl IntoIterator<Item = Self>) -> Self {
        Self {
            sources: subscriptions
                .into_iter()
                .flat_map(|subscription| subscription.sources)
                .collect(),
        }
    }

    /// The same sources with their messages passed through `f`.
    ///
    /// A parent program declares a child's subscriptions mapped into its own
    /// message, so the child stays a program of its own.
    pub fn map<To>(self, f: impl Fn(Message) -> To + Send + Sync + 'static) -> Subscription<To>
    where
        Message: Send + 'static,
        To: Send + 'static,
    {
        let f: Mapper<Message, To> = Arc::new(f);
        Subscription {
            sources: self
                .sources
                .into_iter()
                .map(|source| source.map_with(Arc::clone(&f)))
                .collect(),
        }
    }

    fn one(key: Key, kind: SourceKind<Message>) -> Self {
        Self {
            sources: vec![Source { key, kind }],
        }
    }

    /// The declared sources, for the runtime that reconciles them.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "read by subscription reconciliation")
    )]
    pub(crate) fn into_sources(self) -> Vec<Source<Message>> {
        self.sources
    }

    pub(crate) fn terminal_error_mapper(&self) -> Option<Mapper<io::Error, Message>> {
        self.sources.iter().rev().find_map(|source| {
            if let SourceKind::TerminalErrors(mapper) = &source.kind {
                Some(Arc::clone(mapper))
            } else {
                None
            }
        })
    }
}

impl<Message> Source<Message> {
    fn map_with<To>(self, f: Mapper<Message, To>) -> Source<To>
    where
        Message: Send + 'static,
        To: Send + 'static,
    {
        let kind = match self.kind {
            SourceKind::Input(map) => SourceKind::Input(compose(map, f)),
            SourceKind::Surface(map) => SourceKind::Surface(compose(map, f)),
            SourceKind::Interval { period, map } => SourceKind::Interval {
                period,
                map: compose(map, f),
            },
            SourceKind::Signal { signal, map } => SourceKind::Signal {
                signal,
                map: compose(map, f),
            },
            SourceKind::TerminalErrors(map) => SourceKind::TerminalErrors(compose(map, f)),
            SourceKind::Stream { admission, stream } => SourceKind::Stream {
                admission,
                stream: Box::pin(MapStream { stream, map: f }),
            },
            SourceKind::Run { admission, start } => SourceKind::Run {
                admission,
                start: Box::new(move |sender| start(sender.contramap(f))),
            },
            SourceKind::RunBlocking { admission, start } => SourceKind::RunBlocking {
                admission,
                start: Box::new(move |sender| start(sender.contramap(f))),
            },
        };
        Source {
            key: self.key,
            kind,
        }
    }
}

fn compose<From, Mid, To>(first: Mapper<From, Mid>, second: Mapper<Mid, To>) -> Mapper<From, To>
where
    From: 'static,
    Mid: 'static,
    To: 'static,
{
    Arc::new(move |value| second(first(value)))
}

/// A stream whose items pass through a function on the way out.
struct MapStream<From, To> {
    stream: BoxStream<From>,
    map: Mapper<From, To>,
}

impl<From, To> Stream for MapStream<From, To> {
    type Item = To;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<To>> {
        let this = self.get_mut();
        this.stream
            .as_mut()
            .poll_next(context)
            .map(|item| item.map(|item| (this.map)(item)))
    }
}

impl<Message> Default for Subscription<Message> {
    fn default() -> Self {
        Self::none()
    }
}

impl<Message> fmt::Debug for Subscription<Message> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_list().entries(self.sources.iter()).finish()
    }
}

impl<Message> fmt::Debug for Source<Message> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Source")
            .field("key", &self.key)
            .field("kind", &self.kind.name())
            .finish()
    }
}

impl<Message> SourceKind<Message> {
    pub(crate) fn is_surface(&self) -> bool {
        matches!(self, Self::Surface(_))
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::Input(_) => "input",
            Self::Surface(_) => "surface",
            Self::Interval { .. } => "interval",
            Self::Signal { .. } => "signal",
            Self::TerminalErrors(_) => "terminal_errors",
            Self::Stream { .. } => "stream",
            Self::Run { .. } => "run",
            Self::RunBlocking { .. } => "run_blocking",
        }
    }
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
mod tests;
