//! Terminal input and surface observations owned by the runtime.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use urushi_terminal::{Event, EventSource, TerminalQuery, TerminalSize};

use super::delivery::{
    Admission, DeliveryQueue, Sender, SourceInboxCloser, SurfaceMessagePublisher, source_inbox,
    surface_slot,
};
use super::effect::Mapper;
use super::executor::{Execution, Executor, RunningSource, SourceSpawner};
use super::source::{Input, Surface};
use super::subscription::{Source, SourceKind};

const READ_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Adds the runtime-owned terminal sources to another source spawner.
pub(crate) struct TerminalSourceSpawner<T, Message> {
    owner: Arc<Owner<T, Message>>,
    executor: Arc<dyn Executor>,
    fallback: Arc<dyn SourceSpawner<Message>>,
}

struct Owner<T, Message> {
    shared: Arc<Shared<T, Message>>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

struct Shared<T, Message> {
    terminal: Mutex<T>,
    sources: Mutex<Sources<Message>>,
    source_changed: Condvar,
    failure: Failure,
    surface_sizes: Arc<Mutex<SurfaceSizes>>,
}

#[derive(Default)]
struct SurfaceSizes {
    pending: Option<TerminalSize>,
    accepted: Option<TerminalSize>,
}

struct Sources<Message> {
    input: Option<InputEndpoint<Message>>,
    surface: Option<SurfaceEndpoint<Message>>,
    next_id: usize,
    stopping: bool,
}

struct InputEndpoint<Message> {
    id: usize,
    mapper: Mapper<Input, Message>,
    sender: Sender<Message>,
}

struct SurfaceEndpoint<Message> {
    id: usize,
    mapper: Mapper<Surface, Message>,
    publisher: Arc<SurfaceMessagePublisher<Message>>,
}

struct Failure {
    error: Mutex<Option<io::Error>>,
    notify: tokio::sync::Notify,
}

impl<T, Message> TerminalSourceSpawner<T, Message>
where
    T: EventSource + TerminalQuery + Send + 'static,
    Message: Send + 'static,
{
    pub(crate) fn new(
        terminal: T,
        executor: Arc<dyn Executor>,
        fallback: Arc<dyn SourceSpawner<Message>>,
    ) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            terminal: Mutex::new(terminal),
            sources: Mutex::new(Sources {
                input: None,
                surface: None,
                next_id: 0,
                stopping: false,
            }),
            source_changed: Condvar::new(),
            failure: Failure {
                error: Mutex::new(None),
                notify: tokio::sync::Notify::new(),
            },
            surface_sizes: Arc::new(Mutex::new(SurfaceSizes::default())),
        });
        let worker_shared = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("urushi-terminal-input".into())
            .spawn(move || read_events(worker_shared))?;
        Ok(Self {
            owner: Arc::new(Owner {
                shared,
                worker: Mutex::new(Some(worker)),
            }),
            executor,
            fallback,
        })
    }

    pub(crate) fn presentation_size(&self, fallback: TerminalSize) -> TerminalSize {
        lock(&self.owner.shared.surface_sizes)
            .accepted
            .unwrap_or(fallback)
    }

    fn start_input(
        &self,
        mapper: Mapper<Input, Message>,
        deliveries: DeliveryQueue<Message>,
    ) -> Box<dyn RunningSource<Message>> {
        let (sender, mut inbox) = source_inbox(Admission::default());
        let inbox_closer = inbox.closer();
        let id = self.register_input(mapper, sender);
        let acceptance_deliveries = deliveries.clone();
        let acceptance = self.executor.spawn(Box::pin(async move {
            while inbox.ready().await {
                inbox.try_accept(&acceptance_deliveries);
            }
        }));
        Box::new(RunningInput {
            shared: Arc::clone(&self.owner.shared),
            id,
            inbox_closer,
            _acceptance: acceptance,
        })
    }

    fn start_surface(
        &self,
        mapper: Mapper<Surface, Message>,
        deliveries: DeliveryQueue<Message>,
    ) -> io::Result<Box<dyn RunningSource<Message>>> {
        let (publisher, mut slot) = surface_slot();
        let publisher = Arc::new(publisher);
        let id = {
            let mut terminal = lock(&self.owner.shared.terminal);
            let initial = Surface::from_window_size(terminal.window_size()?);
            let id = self.register_surface(Arc::clone(&mapper), Arc::clone(&publisher));
            let mut sizes = lock(&self.owner.shared.surface_sizes);
            sizes.pending = Some(initial.size);
            publisher.publish(mapper(initial));
            slot.try_accept(&deliveries);
            sizes.accepted = sizes.pending.take();
            id
        };
        // The reader could publish a newer resize after the terminal lock is
        // released. Accepting whichever observation is then latest is correct;
        // notifying the runtime while holding the terminal lock is not.
        let acceptance_deliveries = deliveries.clone();
        let acceptance_shared = Arc::clone(&self.owner.shared);
        let acceptance = self.executor.spawn(Box::pin(async move {
            while slot.ready().await {
                let mut sizes = lock(&acceptance_shared.surface_sizes);
                if slot.try_accept(&acceptance_deliveries) {
                    sizes.accepted = sizes.pending.take();
                }
            }
        }));
        Ok(Box::new(RunningSurface {
            shared: Arc::clone(&self.owner.shared),
            id,
            _acceptance: acceptance,
        }))
    }

    fn register_input(&self, mapper: Mapper<Input, Message>, sender: Sender<Message>) -> usize {
        let mut sources = lock(&self.owner.shared.sources);
        let id = sources.next_id;
        sources.next_id = sources.next_id.wrapping_add(1);
        sources.input = Some(InputEndpoint { id, mapper, sender });
        drop(sources);
        self.owner.shared.source_changed.notify_all();
        id
    }

    fn register_surface(
        &self,
        mapper: Mapper<Surface, Message>,
        publisher: Arc<SurfaceMessagePublisher<Message>>,
    ) -> usize {
        let mut sources = lock(&self.owner.shared.sources);
        let id = sources.next_id;
        sources.next_id = sources.next_id.wrapping_add(1);
        sources.surface = Some(SurfaceEndpoint {
            id,
            mapper,
            publisher,
        });
        drop(sources);
        self.owner.shared.source_changed.notify_all();
        id
    }
}

impl<T, Message> Clone for TerminalSourceSpawner<T, Message> {
    fn clone(&self) -> Self {
        Self {
            owner: Arc::clone(&self.owner),
            executor: Arc::clone(&self.executor),
            fallback: Arc::clone(&self.fallback),
        }
    }
}

impl<T, Message> SourceSpawner<Message> for TerminalSourceSpawner<T, Message>
where
    T: EventSource + TerminalQuery + Send + 'static,
    Message: Send + 'static,
{
    fn start(
        &self,
        source: Source<Message>,
        deliveries: DeliveryQueue<Message>,
    ) -> io::Result<Box<dyn RunningSource<Message>>> {
        match source.kind {
            SourceKind::Input(mapper) => Ok(self.start_input(mapper, deliveries)),
            SourceKind::Surface(mapper) => self.start_surface(mapper, deliveries),
            SourceKind::TerminalErrors(_) => Ok(Box::new(PassiveSource)),
            kind => self.fallback.start(
                Source {
                    key: source.key,
                    kind,
                },
                deliveries,
            ),
        }
    }

    fn failure(&self) -> Pin<Box<dyn std::future::Future<Output = io::Error> + Send + '_>> {
        Box::pin(self.owner.shared.failure.wait())
    }
}

impl<T, Message> Drop for Owner<T, Message> {
    fn drop(&mut self) {
        {
            let mut sources = lock(&self.shared.sources);
            sources.stopping = true;
        }
        self.shared.source_changed.notify_all();
        if let Some(worker) = lock(&self.worker).take() {
            let _ = worker.join();
        }
    }
}

struct RunningInput<T, Message> {
    shared: Arc<Shared<T, Message>>,
    id: usize,
    inbox_closer: SourceInboxCloser<Message>,
    _acceptance: Box<dyn Execution>,
}

impl<T, Message> RunningSource<Message> for RunningInput<T, Message>
where
    T: EventSource + TerminalQuery + Send + 'static,
    Message: Send + 'static,
{
    fn refresh(&mut self, source: Source<Message>) {
        let SourceKind::Input(mapper) = source.kind else {
            return;
        };
        let mut sources = lock(&self.shared.sources);
        if let Some(input) = &mut sources.input
            && input.id == self.id
        {
            input.mapper = mapper;
        }
    }
}

impl<T, Message> Drop for RunningInput<T, Message> {
    fn drop(&mut self) {
        let mut sources = lock(&self.shared.sources);
        if sources
            .input
            .as_ref()
            .is_some_and(|input| input.id == self.id)
        {
            sources.input = None;
        }
        drop(sources);
        // Closing the receiver directly releases a reader blocked by bounded
        // backpressure before its async acceptance task is canceled.
        self.inbox_closer.close();
    }
}

struct RunningSurface<T, Message> {
    shared: Arc<Shared<T, Message>>,
    id: usize,
    _acceptance: Box<dyn Execution>,
}

impl<T, Message> RunningSource<Message> for RunningSurface<T, Message>
where
    T: EventSource + TerminalQuery + Send + 'static,
    Message: Send + 'static,
{
    fn refresh(&mut self, source: Source<Message>) {
        let SourceKind::Surface(mapper) = source.kind else {
            return;
        };
        let mut sources = lock(&self.shared.sources);
        if let Some(surface) = &mut sources.surface
            && surface.id == self.id
        {
            surface.mapper = mapper;
        }
    }
}

impl<T, Message> Drop for RunningSurface<T, Message> {
    fn drop(&mut self) {
        let mut sources = lock(&self.shared.sources);
        if sources
            .surface
            .as_ref()
            .is_some_and(|surface| surface.id == self.id)
        {
            sources.surface = None;
        }
    }
}

struct PassiveSource;

impl<Message> RunningSource<Message> for PassiveSource {
    fn refresh(&mut self, _source: Source<Message>) {}
}

fn read_events<T, Message>(shared: Arc<Shared<T, Message>>)
where
    T: EventSource + TerminalQuery,
    Message: Send + 'static,
{
    loop {
        let sources = wait_for_source(&shared);
        if sources.stopping {
            return;
        }
        drop(sources);

        let event = {
            let mut terminal = lock(&shared.terminal);
            match terminal.poll_event_timeout(READ_POLL_INTERVAL) {
                Ok(Some(event)) => observe(&shared, &mut *terminal, event),
                Ok(None) => Ok(None),
                Err(error) => Err(error),
            }
        };

        match event {
            Ok(Some(observation)) => observation.deliver(),
            Ok(None) => {}
            Err(error) => {
                shared.failure.set(error);
                return;
            }
        }
    }
}

enum Observation<Message> {
    Input {
        input: Input,
        mapper: Mapper<Input, Message>,
        sender: Sender<Message>,
    },
    Surface {
        surface: Surface,
        mapper: Mapper<Surface, Message>,
        publisher: Arc<SurfaceMessagePublisher<Message>>,
        sizes: Arc<Mutex<SurfaceSizes>>,
    },
}

type SurfaceDeliveryEndpoint<Message> = (
    Mapper<Surface, Message>,
    Arc<SurfaceMessagePublisher<Message>>,
);

impl<Message: Send + 'static> Observation<Message> {
    fn deliver(self) {
        match self {
            Self::Input {
                input,
                mapper,
                sender,
            } => {
                let _ = sender.blocking_send(mapper(input));
            }
            Self::Surface {
                surface,
                mapper,
                publisher,
                sizes,
            } => {
                let size = surface.size;
                let mut sizes = lock(&sizes);
                sizes.pending = Some(size);
                publisher.publish(mapper(surface));
            }
        }
    }
}

fn observe<T, Message>(
    shared: &Shared<T, Message>,
    terminal: &mut T,
    event: Event,
) -> io::Result<Option<Observation<Message>>>
where
    T: TerminalQuery,
{
    match event {
        Event::Key(event) => Ok(input_observation(shared, Input::Key(event))),
        Event::Paste(text) => Ok(input_observation(shared, Input::Paste(text))),
        Event::Focus(change) => Ok(input_observation(shared, Input::Focus(change))),
        Event::Mouse(event) => Ok(input_observation(shared, Input::Mouse(event))),
        Event::Resize(_) => {
            let Some((mapper, publisher)) = surface_endpoint(shared) else {
                return Ok(None);
            };
            let surface = Surface::from_window_size(terminal.window_size()?);
            Ok(Some(Observation::Surface {
                surface,
                mapper,
                publisher,
                sizes: Arc::clone(&shared.surface_sizes),
            }))
        }
        _ => Ok(None),
    }
}

fn input_observation<T, Message>(
    shared: &Shared<T, Message>,
    input: Input,
) -> Option<Observation<Message>> {
    let sources = lock(&shared.sources);
    sources.input.as_ref().map(|endpoint| Observation::Input {
        input,
        mapper: Arc::clone(&endpoint.mapper),
        sender: endpoint.sender.clone(),
    })
}

fn surface_endpoint<T, Message>(
    shared: &Shared<T, Message>,
) -> Option<SurfaceDeliveryEndpoint<Message>> {
    let sources = lock(&shared.sources);
    sources.surface.as_ref().map(|endpoint| {
        (
            Arc::clone(&endpoint.mapper),
            Arc::clone(&endpoint.publisher),
        )
    })
}

fn wait_for_source<'a, T, Message>(
    shared: &'a Shared<T, Message>,
) -> MutexGuard<'a, Sources<Message>> {
    let mut sources = lock(&shared.sources);
    while !sources.stopping && sources.input.is_none() && sources.surface.is_none() {
        sources = shared
            .source_changed
            .wait(sources)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
    }
    sources
}

impl Failure {
    fn set(&self, error: io::Error) {
        *lock(&self.error) = Some(error);
        self.notify.notify_waiters();
    }

    async fn wait(&self) -> io::Error {
        loop {
            let notified = self.notify.notified();
            if let Some(error) = lock(&self.error).take() {
                return error;
            }
            notified.await;
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[path = "terminal_source_tests.rs"]
mod tests;
