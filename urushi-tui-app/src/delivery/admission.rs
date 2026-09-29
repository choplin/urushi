//! Public admission policy and sender values.

use std::error::Error;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::super::effect::Mapper;

/// The default number of unaccepted items a bounded source may hold.
const DEFAULT_CAPACITY: usize = 64;

/// What a source does with an item the runtime has not accepted yet.
///
/// A source declares its policy through the `_with` form of its subscription
/// constructor and otherwise gets [`Admission::bounded`] at the default
/// capacity. The runtime's own sources carry the policies the runtime gives
/// them and take no admission from the application.
///
/// ```
/// use urushi_tui_app::Admission;
///
/// assert_eq!(Admission::default(), Admission::bounded(64));
/// assert_ne!(Admission::latest(), Admission::bounded(1));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Admission {
    policy: Policy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Policy {
    /// At most `capacity` unaccepted items, oldest accepted first; the source
    /// waits while it is full.
    Bounded { capacity: usize },
    /// One unaccepted slot, which a newer item replaces; the source never
    /// waits.
    Latest,
}

impl Admission {
    /// A FIFO of at most `capacity` unaccepted items.
    ///
    /// The source waits while the queue is full, which is what makes a fast
    /// source slow down rather than the runtime grow without bound. A capacity
    /// of zero would admit nothing, so it is read as one.
    pub const fn bounded(capacity: usize) -> Self {
        let capacity = if capacity == 0 { 1 } else { capacity };
        Self {
            policy: Policy::Bounded { capacity },
        }
    }

    /// One unaccepted slot, which a newer item replaces.
    ///
    /// The source never waits, and an item the runtime did not accept before
    /// the next arrived is dropped. This suits an observation whose older value
    /// carries nothing once a newer one exists.
    pub const fn latest() -> Self {
        Self {
            policy: Policy::Latest,
        }
    }

    pub(crate) const fn policy(self) -> Policy {
        self.policy
    }
}

impl Default for Admission {
    fn default() -> Self {
        Self::bounded(DEFAULT_CAPACITY)
    }
}

/// Where an application-defined source puts its messages.
///
/// The source is handed one by [`Subscription::run`](crate::Subscription::run)
/// or [`run_blocking`](crate::Subscription::run_blocking). Whether a send waits
/// is the source's [`Admission`]: under [`Admission::bounded`] a send waits
/// while the queue is full, and under [`Admission::latest`] it never waits.
pub struct Sender<Message> {
    sink: Arc<dyn Sink<Message>>,
}

/// The capability hidden behind a public [`Sender`].
///
/// The source-local inbox implements it; tests for mapping may substitute a
/// collector without depending on admission internals.
pub(crate) trait Sink<Message>: Send + Sync + 'static {
    fn send<'a>(&'a self, message: Message) -> BoxSend<'a>;

    fn blocking_send(&self, message: Message) -> Result<(), SendError>;
}

pub(crate) type BoxSend<'a> = Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + 'a>>;

impl<Message: Send + 'static> Sender<Message> {
    pub(crate) fn new(sink: Arc<dyn Sink<Message>>) -> Self {
        Self { sink }
    }

    /// Sends a message, waiting where the source's admission says to.
    pub async fn send(&self, message: Message) -> Result<(), SendError> {
        self.sink.send(message).await
    }

    /// Sends a message from a thread that may block, for a source that is not
    /// asynchronous.
    pub fn blocking_send(&self, message: Message) -> Result<(), SendError> {
        self.sink.blocking_send(message)
    }

    /// A sender that takes the source's own message and passes it through `f`
    /// on the way in. This is how `map` reaches a source that is given a sender
    /// rather than one that returns values.
    pub(crate) fn contramap<From>(self, f: Mapper<From, Message>) -> Sender<From>
    where
        From: Send + 'static,
    {
        Sender::new(Arc::new(Mapped {
            inner: self,
            map: f,
        }))
    }
}

impl<Message> Clone for Sender<Message> {
    fn clone(&self) -> Self {
        Self {
            sink: Arc::clone(&self.sink),
        }
    }
}

impl<Message> fmt::Debug for Sender<Message> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Sender")
    }
}

struct Mapped<From, To> {
    inner: Sender<To>,
    map: Mapper<From, To>,
}

impl<From, To> Sink<From> for Mapped<From, To>
where
    From: Send + 'static,
    To: Send + 'static,
{
    fn send<'a>(&'a self, message: From) -> BoxSend<'a> {
        let mapped = (self.map)(message);
        Box::pin(self.inner.send(mapped))
    }

    fn blocking_send(&self, message: From) -> Result<(), SendError> {
        self.inner.blocking_send((self.map)(message))
    }
}

/// The runtime is no longer taking this source's messages.
///
/// A source that meets this has been stopped — the application stopped
/// declaring it, or the runtime is shutting down — and should return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SendError;

impl fmt::Display for SendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the runtime stopped taking this source's messages")
    }
}

impl Error for SendError {}
