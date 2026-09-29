//! What an `update` asks the runtime to do.

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use urushi::Key;

/// Blocking work an effect carries.
pub(crate) type Work<Message> = Box<dyn FnOnce() -> Message + Send + 'static>;

/// What a delayed effect turns its firing time into.
pub(crate) type Fire<Message> = Box<dyn FnOnce(Instant) -> Message + Send + 'static>;

/// A future an effect carries.
pub(crate) type BoxFuture<Message> = Pin<Box<dyn Future<Output = Message> + Send + 'static>>;

/// A function the runtime may call for many values.
pub(crate) type Mapper<From, To> = Arc<dyn Fn(From) -> To + Send + Sync + 'static>;

/// Work the runtime performs on behalf of an `update`, whose result returns as
/// a message.
///
/// An effect is a value. It runs nothing when it is built, it cannot touch the
/// model, and it names no executor: which thread runs a closure and what polls
/// a future is the runtime's, behind a boundary an application never sees.
/// Building one in a test therefore costs nothing and needs no terminal.
///
/// ```
/// use urushi_tui_app::Effect;
///
/// enum Message {
///     Loaded(usize),
/// }
///
/// let effect = Effect::perform(|| Message::Loaded(1));
/// ```
///
/// # Which constructor
///
/// [`perform`](Effect::perform) carries blocking work and
/// [`future`](Effect::future) carries a future, so an application whose I/O is
/// already asynchronous does not wrap it in a thread and one whose work is
/// blocking does not reach for an executor by name. The `_latest` forms of each
/// carry a [`Key`]: starting one replaces an unfinished effect running under
/// the same key, which is the one staleness the runtime knows about on its own.
/// Every other reason a completion no longer applies is the application's to
/// check in `update`.
///
/// [`after`](Effect::after) is the one-shot timer: it waits and then sends its
/// message, reading the runtime's clock rather than an executor the application
/// brought. [`after_latest`](Effect::after_latest) is the same under a key, so a
/// second one restarts the wait rather than adding a second timer — which is
/// what debouncing is. A timer that repeats is
/// [`Subscription::interval`](super::Subscription::interval) instead, because it
/// lives for as long as the application declares it.
///
/// [`batch`](Effect::batch) starts several effects concurrently and promises
/// nothing about the order their completions arrive in. There is no sequencing
/// combinator: work that must follow other work is returned from the `update`
/// that receives the first completion.
pub struct Effect<Message> {
    kind: EffectKind<Message>,
}

/// The shape of an [`Effect`], as the runtime reads it.
pub(crate) enum EffectKind<Message> {
    /// Nothing to do.
    None,
    /// Stop the runtime.
    Shutdown,
    /// Blocking work, replaceable when it carries a key.
    Perform {
        key: Option<Key>,
        work: Work<Message>,
    },
    /// A future, replaceable when it carries a key.
    Future {
        key: Option<Key>,
        future: BoxFuture<Message>,
    },
    /// A message to send once `delay` has passed, replaceable when it carries a
    /// key.
    After {
        key: Option<Key>,
        delay: Duration,
        fire: Fire<Message>,
    },
    /// Several effects, started concurrently.
    Batch(Vec<Effect<Message>>),
}

impl<Message> Effect<Message> {
    /// An effect that does nothing.
    pub fn none() -> Self {
        Self {
            kind: EffectKind::None,
        }
    }

    /// The request to stop the runtime.
    ///
    /// The runtime reads the request from `update`'s return value rather than
    /// delivering it, so it never enters admission and is not a message. On it,
    /// no sibling effect in the same return value starts, the completions of
    /// effects still in flight are discarded, every subscription stops, the
    /// terminal session is restored, and the entry point returns the final
    /// model. Work that must finish before the application exits is an ordinary
    /// effect whose completion is the `update` that returns this.
    pub fn shutdown() -> Self {
        Self {
            kind: EffectKind::Shutdown,
        }
    }

    /// Blocking work, run off the thread that runs `update`.
    pub fn perform<F>(work: F) -> Self
    where
        F: FnOnce() -> Message + Send + 'static,
    {
        Self {
            kind: EffectKind::Perform {
                key: None,
                work: Box::new(work),
            },
        }
    }

    /// Blocking work that replaces any unfinished work running under `key`.
    pub fn perform_latest<F>(key: impl Into<Key>, work: F) -> Self
    where
        F: FnOnce() -> Message + Send + 'static,
    {
        Self {
            kind: EffectKind::Perform {
                key: Some(key.into()),
                work: Box::new(work),
            },
        }
    }

    /// Asynchronous work.
    pub fn future<F>(future: F) -> Self
    where
        F: Future<Output = Message> + Send + 'static,
    {
        Self {
            kind: EffectKind::Future {
                key: None,
                future: Box::pin(future),
            },
        }
    }

    /// Asynchronous work that replaces any unfinished work running under `key`.
    pub fn future_latest<F>(key: impl Into<Key>, future: F) -> Self
    where
        F: Future<Output = Message> + Send + 'static,
    {
        Self {
            kind: EffectKind::Future {
                key: Some(key.into()),
                future: Box::pin(future),
            },
        }
    }

    /// A message sent once `delay` has passed, timed by the runtime's clock.
    ///
    /// The clock is the runtime's, which is what keeps a waiting application
    /// free of an executor of its own and what lets a test drive the wait by
    /// hand instead of sleeping.
    pub fn after<F>(delay: Duration, f: F) -> Self
    where
        F: FnOnce(Instant) -> Message + Send + 'static,
    {
        Self {
            kind: EffectKind::After {
                key: None,
                delay,
                fire: Box::new(f),
            },
        }
    }

    /// A delayed message that replaces any unfired timer under `key`.
    ///
    /// Replacing a timer restarts its wait, so an `update` that returns this on
    /// every keystroke fires once the keystrokes stop: the debounce a search
    /// field or a live preview needs, with no state in the model.
    pub fn after_latest<F>(key: impl Into<Key>, delay: Duration, f: F) -> Self
    where
        F: FnOnce(Instant) -> Message + Send + 'static,
    {
        Self {
            kind: EffectKind::After {
                key: Some(key.into()),
                delay,
                fire: Box::new(f),
            },
        }
    }

    /// Several effects, started concurrently.
    ///
    /// Their completions may arrive in any order.
    pub fn batch(effects: impl IntoIterator<Item = Self>) -> Self {
        Self {
            kind: EffectKind::Batch(effects.into_iter().collect()),
        }
    }

    /// The same effect with its message passed through `f`.
    ///
    /// This is what lets a program hold another program: a parent whose message
    /// wraps a child's calls the child's `update`, receives the child's effect,
    /// and returns it mapped, without the child knowing the parent's message
    /// type.
    pub fn map<To>(self, f: impl Fn(Message) -> To + Send + Sync + 'static) -> Effect<To>
    where
        Message: Send + 'static,
        To: Send + 'static,
    {
        self.map_with(Arc::new(f))
    }

    fn map_with<To>(self, f: Mapper<Message, To>) -> Effect<To>
    where
        Message: Send + 'static,
        To: Send + 'static,
    {
        let kind = match self.kind {
            EffectKind::None => EffectKind::None,
            EffectKind::Shutdown => EffectKind::Shutdown,
            EffectKind::Perform { key, work } => EffectKind::Perform {
                key,
                work: Box::new(move || f(work())),
            },
            EffectKind::Future { key, future } => EffectKind::Future {
                key,
                future: Box::pin(async move { f(future.await) }),
            },
            EffectKind::After { key, delay, fire } => EffectKind::After {
                key,
                delay,
                fire: Box::new(move |at| f(fire(at))),
            },
            EffectKind::Batch(effects) => EffectKind::Batch(
                effects
                    .into_iter()
                    .map(|effect| effect.map_with(Arc::clone(&f)))
                    .collect(),
            ),
        };
        Effect { kind }
    }

    /// The shape of this effect, for the runtime that interprets it.
    pub(crate) fn into_kind(self) -> EffectKind<Message> {
        self.kind
    }

    /// Whether this effect tree asks the runtime to stop before starting work.
    pub(crate) fn requests_shutdown(&self) -> bool {
        match &self.kind {
            EffectKind::Shutdown => true,
            EffectKind::Batch(effects) => effects.iter().any(Self::requests_shutdown),
            EffectKind::None
            | EffectKind::Perform { .. }
            | EffectKind::Future { .. }
            | EffectKind::After { .. } => false,
        }
    }
}

impl<Message> Default for Effect<Message> {
    fn default() -> Self {
        Self::none()
    }
}

impl<Message> fmt::Debug for Effect<Message> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            EffectKind::None => formatter.write_str("Effect::none"),
            EffectKind::Shutdown => formatter.write_str("Effect::shutdown"),
            EffectKind::Perform { key, .. } => formatter
                .debug_struct("Effect::perform")
                .field("key", key)
                .finish(),
            EffectKind::Future { key, .. } => formatter
                .debug_struct("Effect::future")
                .field("key", key)
                .finish(),
            EffectKind::After { key, delay, .. } => formatter
                .debug_struct("Effect::after")
                .field("key", key)
                .field("delay", delay)
                .finish(),
            EffectKind::Batch(effects) => formatter.debug_list().entries(effects.iter()).finish(),
        }
    }
}

#[cfg(test)]
#[path = "effect_tests.rs"]
mod tests;
