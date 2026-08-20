//! What the runtime's own tests need, and nothing a terminal or an executor
//! would provide.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use futures_core::Stream;

use super::delivery::{SendError, Sender, Sink};

/// Drives a future to completion on this thread.
///
/// Every future these tests build is ready without waiting, so nothing here
/// wakes one, and a future that would wait is a test that means something other
/// than it says.
pub(crate) fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("this future waits, and no test here can wake it"),
    }
}

/// Polls a stream to exhaustion on this thread.
pub(crate) fn drain<S: Stream + ?Sized>(stream: Pin<&mut S>) -> Vec<S::Item> {
    let mut stream = stream;
    let mut context = Context::from_waker(Waker::noop());
    let mut items = Vec::new();
    loop {
        match stream.as_mut().poll_next(&mut context) {
            Poll::Ready(Some(item)) => items.push(item),
            Poll::Ready(None) => return items,
            Poll::Pending => panic!("this stream waits, and no test here can wake it"),
        }
    }
}

/// A [`Sender`] destination that keeps what it was sent, standing in for the
/// runtime's queue where the queue is not what is under test.
pub(crate) struct Collector<Message> {
    messages: Mutex<Vec<Message>>,
}

impl<Message: Send + 'static> Collector<Message> {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            messages: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn sender(self: &Arc<Self>) -> Sender<Message> {
        Sender::new(Arc::clone(self) as Arc<dyn Sink<Message>>)
    }

    pub(crate) fn take(&self) -> Vec<Message> {
        std::mem::take(
            &mut self
                .messages
                .lock()
                .expect("no test panics under this lock"),
        )
    }
}

impl<Message: Send + 'static> Sink<Message> for Collector<Message> {
    fn send<'a>(
        &'a self,
        message: Message,
    ) -> Pin<Box<dyn Future<Output = Result<(), SendError>> + Send + 'a>> {
        let result = self.blocking_send(message);
        Box::pin(async move { result })
    }

    fn blocking_send(&self, message: Message) -> Result<(), SendError> {
        self.messages
            .lock()
            .expect("no test panics under this lock")
            .push(message);
        Ok(())
    }
}

/// A stream of what an iterator yields, ready at every poll.
pub(crate) struct Ready<I> {
    items: I,
}

impl<I: Iterator + Unpin> Ready<I> {
    pub(crate) fn new(items: impl IntoIterator<IntoIter = I>) -> Self {
        Self {
            items: items.into_iter(),
        }
    }
}

impl<I: Iterator + Unpin> Stream for Ready<I> {
    type Item = I::Item;

    fn poll_next(self: Pin<&mut Self>, _context: &mut Context<'_>) -> Poll<Option<I::Item>> {
        Poll::Ready(self.get_mut().items.next())
    }
}
