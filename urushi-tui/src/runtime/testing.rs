//! Deterministic runtime boundaries shared by behavioral tests.

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use futures_core::Stream;
use urushi::StyledGrapheme;
use urushi_terminal::{Position, TerminalSize};

use crate::terminal::{Frame, Rect, Terminal};

use super::delivery::{
    Admission, Delivery, DeliveryQueue, SendError, Sender, Sink, SourceInbox, source_inbox,
};
use super::executor::{BlockingTask, Clock, Execution, Executor, Task};

/// All replaceable runtime boundaries needed by tests that drive delivery.
pub(crate) struct Harness<Message> {
    deliveries: DeliveryQueue<Message>,
    executor: ManualExecutor,
    clock: ManualClock,
    #[expect(dead_code, reason = "read by the runtime core tests in the next issue")]
    terminal: InMemoryTerminal,
}

impl<Message: Send + 'static> Harness<Message> {
    pub(crate) fn new(size: TerminalSize) -> Self {
        Self {
            deliveries: DeliveryQueue::new(),
            executor: ManualExecutor::new(),
            clock: ManualClock::new(),
            terminal: InMemoryTerminal::new(size),
        }
    }

    pub(crate) fn source(&self, admission: Admission) -> DeterministicSource<Message> {
        DeterministicSource::new(admission)
    }

    pub(crate) fn accept(&self, source: &mut DeterministicSource<Message>) -> bool {
        source.inbox.try_accept(&self.deliveries)
    }

    pub(crate) fn complete(&self, message: Message) {
        self.deliveries.ordinary_completion().complete(message);
    }

    pub(crate) fn next(&self) -> Option<Delivery<Message>> {
        self.deliveries.try_next()
    }

    pub(crate) fn drain(&self) -> Vec<Delivery<Message>> {
        let mut deliveries = Vec::new();
        while let Some(delivery) = self.next() {
            deliveries.push(delivery);
        }
        deliveries
    }

    pub(crate) fn deliveries(&self) -> DeliveryQueue<Message> {
        self.deliveries.clone()
    }

    pub(crate) fn executor(&self) -> Arc<dyn Executor> {
        Arc::new(self.executor.clone())
    }

    pub(crate) fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(self.clock.clone())
    }

    pub(crate) fn begin_effect(&self, id: usize) -> bool {
        self.executor.begin(id)
    }

    pub(crate) fn complete_effect(&self, id: usize) -> bool {
        self.executor.complete(id)
    }

    pub(crate) fn advance(&self, duration: Duration) {
        self.clock.advance(duration);
    }

    pub(crate) fn effect_events(&self) -> Vec<EffectEvent> {
        self.executor.events()
    }

    #[expect(dead_code, reason = "read by the runtime core tests in the next issue")]
    pub(crate) fn terminal(&mut self) -> &mut InMemoryTerminal {
        &mut self.terminal
    }
}

/// One source whose admission and acceptance are advanced explicitly by a test.
pub(crate) struct DeterministicSource<Message> {
    sender: Sender<Message>,
    inbox: SourceInbox<Message>,
}

impl<Message: Send + 'static> DeterministicSource<Message> {
    fn new(admission: Admission) -> Self {
        let (sender, inbox) = source_inbox(admission);
        Self { sender, inbox }
    }

    pub(crate) fn send(&self, message: Message) -> Result<(), SendError> {
        self.sender.blocking_send(message)
    }
}

/// Observable lifecycle of effect work, independent of task representation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EffectEvent {
    Started { id: usize },
    Completed { id: usize },
    Canceled { id: usize },
}

#[derive(Clone)]
struct ManualExecutor {
    shared: Arc<ManualExecutorShared>,
}

struct ManualExecutorShared {
    state: Mutex<ManualExecutorState>,
}

struct ManualExecutorState {
    next_id: usize,
    jobs: VecDeque<Job>,
    running: Vec<Job>,
    events: Vec<EffectEvent>,
}

impl ManualExecutor {
    fn new() -> Self {
        Self {
            shared: Arc::new(ManualExecutorShared {
                state: Mutex::new(ManualExecutorState {
                    next_id: 0,
                    jobs: VecDeque::new(),
                    running: Vec::new(),
                    events: Vec::new(),
                }),
            }),
        }
    }

    fn schedule(&self, task: JobTask) -> Box<dyn Execution> {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("test executor lock is healthy");
        let id = state.next_id;
        state.next_id += 1;
        state.events.push(EffectEvent::Started { id });
        let lifecycle = Arc::new(TaskLifecycle::new(id, Arc::clone(&self.shared)));
        state.jobs.push_back(Job {
            task,
            lifecycle: Arc::clone(&lifecycle),
        });
        Box::new(ManualExecution { lifecycle })
    }

    fn begin(&self, id: usize) -> bool {
        let mut state = self
            .shared
            .state
            .lock()
            .expect("test executor lock is healthy");
        let Some(position) = state.jobs.iter().position(|job| job.lifecycle.id == id) else {
            return false;
        };
        let job = state
            .jobs
            .remove(position)
            .expect("the indexed effect exists");
        if !job.lifecycle.begin() {
            return false;
        }
        state.running.push(job);
        true
    }

    fn complete(&self, id: usize) -> bool {
        let mut job = {
            let mut state = self
                .shared
                .state
                .lock()
                .expect("test executor lock is healthy");
            if let Some(position) = state.running.iter().position(|job| job.lifecycle.id == id) {
                state.running.swap_remove(position)
            } else {
                let Some(position) = state.jobs.iter().position(|job| job.lifecycle.id == id)
                else {
                    return false;
                };
                let job = state
                    .jobs
                    .remove(position)
                    .expect("the indexed effect exists");
                if !job.lifecycle.begin() {
                    return false;
                }
                job
            }
        };

        match &mut job.task {
            JobTask::Future(task) => {
                if job.lifecycle.is_canceled() {
                    return false;
                }
                let mut context = Context::from_waker(Waker::noop());
                if task.as_mut().poll(&mut context).is_pending() {
                    job.lifecycle.pause();
                    self.shared
                        .state
                        .lock()
                        .expect("test executor lock is healthy")
                        .jobs
                        .push_back(job);
                    false
                } else {
                    job.lifecycle.complete();
                    true
                }
            }
            JobTask::Blocking(task) => {
                task.take().expect("an effect completes once")();
                job.lifecycle.complete();
                true
            }
        }
    }

    fn events(&self) -> Vec<EffectEvent> {
        self.shared
            .state
            .lock()
            .expect("test executor lock is healthy")
            .events
            .clone()
    }
}

impl Executor for ManualExecutor {
    fn spawn(&self, task: Task) -> Box<dyn Execution> {
        self.schedule(JobTask::Future(task))
    }

    fn spawn_blocking(&self, task: BlockingTask) -> Box<dyn Execution> {
        self.schedule(JobTask::Blocking(Some(task)))
    }
}

struct Job {
    task: JobTask,
    lifecycle: Arc<TaskLifecycle>,
}

enum JobTask {
    Future(Task),
    Blocking(Option<BlockingTask>),
}

const WAITING: u8 = 0;
const RUNNING: u8 = 1;
const COMPLETED: u8 = 2;
const CANCELED: u8 = 3;

struct TaskLifecycle {
    id: usize,
    status: AtomicU8,
    executor: Arc<ManualExecutorShared>,
}

impl TaskLifecycle {
    fn new(id: usize, executor: Arc<ManualExecutorShared>) -> Self {
        Self {
            id,
            status: AtomicU8::new(WAITING),
            executor,
        }
    }

    fn begin(&self) -> bool {
        self.status
            .compare_exchange(WAITING, RUNNING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn pause(&self) {
        let _ = self
            .status
            .compare_exchange(RUNNING, WAITING, Ordering::AcqRel, Ordering::Acquire);
    }

    fn complete(&self) {
        let previous = self.status.swap(COMPLETED, Ordering::AcqRel);
        if previous != COMPLETED {
            self.record(EffectEvent::Completed { id: self.id });
        }
    }

    fn is_canceled(&self) -> bool {
        self.status.load(Ordering::Acquire) == CANCELED
    }

    fn cancel(&self) {
        let previous = self.status.swap(CANCELED, Ordering::AcqRel);
        if previous != COMPLETED && previous != CANCELED {
            self.record(EffectEvent::Canceled { id: self.id });
        }
    }

    fn record(&self, event: EffectEvent) {
        self.executor
            .state
            .lock()
            .expect("test executor lock is healthy")
            .events
            .push(event);
    }
}

struct ManualExecution {
    lifecycle: Arc<TaskLifecycle>,
}

impl Execution for ManualExecution {}

impl Drop for ManualExecution {
    fn drop(&mut self) {
        self.lifecycle.cancel();
    }
}

#[derive(Clone)]
struct ManualClock {
    shared: Arc<Mutex<ManualClockState>>,
}

struct ManualClockState {
    now: Instant,
    sleepers: Vec<Waker>,
}

impl ManualClock {
    fn new() -> Self {
        Self {
            shared: Arc::new(Mutex::new(ManualClockState {
                now: Instant::now(),
                sleepers: Vec::new(),
            })),
        }
    }

    fn advance(&self, duration: Duration) {
        let sleepers = {
            let mut state = self.shared.lock().expect("test clock lock is healthy");
            state.now += duration;
            std::mem::take(&mut state.sleepers)
        };
        sleepers.into_iter().for_each(Waker::wake);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Instant {
        self.shared.lock().expect("test clock lock is healthy").now
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = Instant> + Send + 'static>> {
        let deadline = self.shared.lock().expect("test clock lock is healthy").now + duration;
        Box::pin(Sleep {
            clock: Arc::clone(&self.shared),
            deadline,
        })
    }
}

struct Sleep {
    clock: Arc<Mutex<ManualClockState>>,
    deadline: Instant,
}

impl Future for Sleep {
    type Output = Instant;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.clock.lock().expect("test clock lock is healthy");
        if state.now >= self.deadline {
            Poll::Ready(self.deadline)
        } else {
            if !state
                .sleepers
                .iter()
                .any(|waker| waker.will_wake(context.waker()))
            {
                state.sleepers.push(context.waker().clone());
            }
            Poll::Pending
        }
    }
}

/// One cell in a committed in-memory frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InMemoryCell {
    Empty,
    Grapheme(StyledGrapheme),
    Continuation,
}

/// One successfully committed frame and its cursor request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CommittedFrame {
    size: TerminalSize,
    cells: Vec<InMemoryCell>,
    cursor: Option<Position>,
}

impl CommittedFrame {
    #[expect(dead_code, reason = "retained for runtime behavior tests")]
    pub(crate) fn size(&self) -> TerminalSize {
        self.size
    }

    #[expect(dead_code, reason = "retained for runtime behavior tests")]
    pub(crate) fn cells(&self) -> &[InMemoryCell] {
        &self.cells
    }

    pub(crate) fn cell(&self, x: usize, y: usize) -> Option<&InMemoryCell> {
        if x >= self.size.columns() || y >= self.size.rows() {
            return None;
        }
        self.cells.get(y * self.size.columns() + x)
    }

    pub(crate) fn cursor(&self) -> Option<Position> {
        self.cursor
    }
}

/// A backend-independent terminal that records every committed presentation.
pub(crate) struct InMemoryTerminal {
    size: TerminalSize,
    committed: Vec<CommittedFrame>,
}

impl InMemoryTerminal {
    pub(crate) fn new(size: TerminalSize) -> Self {
        Self {
            size,
            committed: Vec::new(),
        }
    }

    pub(crate) fn frames(&self) -> &[CommittedFrame] {
        &self.committed
    }
}

impl Terminal for InMemoryTerminal {
    type Cell = StyledGrapheme;

    type Frame<'a>
        = InMemoryFrame
    where
        Self: 'a;

    fn size(&self) -> TerminalSize {
        self.size
    }

    fn resize(&mut self, size: TerminalSize) -> io::Result<()> {
        self.size = size;
        Ok(())
    }

    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()> {
        let mut frame = InMemoryFrame {
            size: self.size,
            cells: vec![InMemoryCell::Empty; self.size.columns().saturating_mul(self.size.rows())],
            cursor: None,
        };
        draw(&mut frame);
        self.committed.push(CommittedFrame {
            size: frame.size,
            cells: frame.cells,
            cursor: frame.cursor,
        });
        Ok(())
    }
}

pub(crate) struct InMemoryFrame {
    size: TerminalSize,
    cells: Vec<InMemoryCell>,
    cursor: Option<Position>,
}

impl Frame for InMemoryFrame {
    type Cell = StyledGrapheme;

    fn area(&self) -> Rect {
        Rect::from_size(self.size)
    }

    fn put(&mut self, x: usize, y: usize, grapheme: &StyledGrapheme) {
        if x >= self.size.columns() || y >= self.size.rows() {
            return;
        }
        let row_end = (y + 1) * self.size.columns();
        let start = y * self.size.columns() + x;
        self.cells[start] = InMemoryCell::Grapheme(grapheme.clone());
        let end = start.saturating_add(grapheme.width()).min(row_end);
        if end > start + 1 {
            self.cells[start + 1..end].fill(InMemoryCell::Continuation);
        }
    }

    fn set_cursor(&mut self, at: Option<Position>) {
        self.cursor = at;
    }
}

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
