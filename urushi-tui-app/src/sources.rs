//! Production adapters for non-terminal subscriptions.

use std::future::poll_fn;
use std::io;
use std::sync::{Arc, Mutex, MutexGuard};

use super::delivery::{Admission, DeliveryQueue, Sender, SourceInboxCloser, source_inbox};
use super::effect::Mapper;
use super::executor::{Clock, Execution, Executor, RunningSource, SourceSpawner};
use super::source::Signal;
use super::subscription::{DynamicMapper, Source, SourceKind};

/// Starts application-defined, timer, and process-signal subscriptions.
pub(crate) struct RuntimeSourceSpawner<Message> {
    executor: Arc<dyn Executor>,
    clock: Arc<dyn Clock>,
    _message: std::marker::PhantomData<fn() -> Message>,
}

impl<Message> RuntimeSourceSpawner<Message> {
    pub(crate) fn new(executor: Arc<dyn Executor>, clock: Arc<dyn Clock>) -> Self {
        Self {
            executor,
            clock,
            _message: std::marker::PhantomData,
        }
    }

    fn inbox(&self, admission: Admission, deliveries: DeliveryQueue<Message>) -> Inbox<Message>
    where
        Message: Send + 'static,
    {
        let (sender, mut inbox) = source_inbox(admission);
        let closer = inbox.closer();
        let acceptance = self.executor.spawn(Box::pin(async move {
            while inbox.ready().await {
                inbox.try_accept(&deliveries);
            }
        }));
        Inbox {
            sender,
            closer,
            acceptance,
        }
    }
}

impl<Message> SourceSpawner<Message> for RuntimeSourceSpawner<Message>
where
    Message: Send + 'static,
{
    fn start(
        &self,
        source: Source<Message>,
        deliveries: DeliveryQueue<Message>,
    ) -> io::Result<Box<dyn RunningSource<Message>>> {
        match source.kind {
            SourceKind::Interval { period, map } => {
                let inbox = self.inbox(Admission::default(), deliveries);
                let mapper = Arc::new(Mutex::new(map));
                let producer_mapper = Arc::clone(&mapper);
                let sender = inbox.sender.clone();
                let clock = Arc::clone(&self.clock);
                let producer = self.executor.spawn(Box::pin(async move {
                    loop {
                        let at = clock.sleep(period).await;
                        let message = lock(&producer_mapper)(at);
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                }));
                Ok(Box::new(SpawnedSource {
                    closer: inbox.closer,
                    _acceptance: inbox.acceptance,
                    _producer: producer,
                    refresh: Refresh::Interval(mapper),
                }))
            }
            SourceKind::Signal { signal, map } => {
                let mut listener = PlatformSignal::new(signal)?;
                let inbox = self.inbox(Admission::default(), deliveries);
                let mapper = Arc::new(Mutex::new(map));
                let producer_mapper = Arc::clone(&mapper);
                let sender = inbox.sender.clone();
                let producer = self.executor.spawn(Box::pin(async move {
                    while listener.recv().await {
                        let message = lock(&producer_mapper)(signal);
                        if sender.send(message).await.is_err() {
                            break;
                        }
                    }
                }));
                Ok(Box::new(SpawnedSource {
                    closer: inbox.closer,
                    _acceptance: inbox.acceptance,
                    _producer: producer,
                    refresh: Refresh::Signal(mapper),
                }))
            }
            SourceKind::Stream {
                admission,
                mut stream,
                map,
            } => {
                let inbox = self.inbox(admission, deliveries);
                let current = map.clone();
                let sender = inbox.sender.clone();
                let producer = self.executor.spawn(Box::pin(async move {
                    while let Some(message) =
                        poll_fn(|context| stream.as_mut().poll_next(context)).await
                    {
                        if sender.send(current.apply(message)).await.is_err() {
                            break;
                        }
                    }
                }));
                Ok(Box::new(SpawnedSource {
                    closer: inbox.closer,
                    _acceptance: inbox.acceptance,
                    _producer: producer,
                    refresh: Refresh::Application(map),
                }))
            }
            SourceKind::Run {
                admission,
                start,
                map,
            } => {
                let inbox = self.inbox(admission, deliveries);
                let current = map.clone();
                let sender = inbox
                    .sender
                    .clone()
                    .contramap(Arc::new(move |message| current.apply(message)));
                let producer = self.executor.spawn(start(sender));
                Ok(Box::new(SpawnedSource {
                    closer: inbox.closer,
                    _acceptance: inbox.acceptance,
                    _producer: producer,
                    refresh: Refresh::Application(map),
                }))
            }
            SourceKind::RunBlocking {
                admission,
                start,
                map,
            } => {
                let inbox = self.inbox(admission, deliveries);
                let current = map.clone();
                let sender = inbox
                    .sender
                    .clone()
                    .contramap(Arc::new(move |message| current.apply(message)));
                let producer = self
                    .executor
                    .spawn_blocking(Box::new(move || start(sender)));
                Ok(Box::new(SpawnedSource {
                    closer: inbox.closer,
                    _acceptance: inbox.acceptance,
                    _producer: producer,
                    refresh: Refresh::Application(map),
                }))
            }
            SourceKind::Input(_) | SourceKind::Surface(_) | SourceKind::TerminalErrors(_) => {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "terminal source reached fallback",
                ))
            }
        }
    }
}

struct Inbox<Message> {
    sender: Sender<Message>,
    closer: SourceInboxCloser<Message>,
    acceptance: Box<dyn Execution>,
}

struct SpawnedSource<Message> {
    closer: SourceInboxCloser<Message>,
    _acceptance: Box<dyn Execution>,
    _producer: Box<dyn Execution>,
    refresh: Refresh<Message>,
}

enum Refresh<Message> {
    Interval(Arc<Mutex<Mapper<std::time::Instant, Message>>>),
    Signal(Arc<Mutex<Mapper<Signal, Message>>>),
    Application(DynamicMapper<Message>),
}

impl<Message: Send + 'static> RunningSource<Message> for SpawnedSource<Message> {
    fn refresh(&mut self, source: Source<Message>) {
        match (&self.refresh, source.kind) {
            (Refresh::Interval(current), SourceKind::Interval { map, .. }) => {
                *lock(current) = map;
            }
            (Refresh::Signal(current), SourceKind::Signal { map, .. }) => {
                *lock(current) = map;
            }
            (Refresh::Application(current), SourceKind::Stream { map, .. })
            | (Refresh::Application(current), SourceKind::Run { map, .. })
            | (Refresh::Application(current), SourceKind::RunBlocking { map, .. }) => {
                current.replace(&map);
            }
            _ => {}
        }
    }
}

impl<Message> Drop for SpawnedSource<Message> {
    fn drop(&mut self) {
        self.closer.close();
    }
}

#[cfg(unix)]
mod unix_signal {
    use std::io::{self, Read};
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::sync::Mutex;

    use super::Signal;

    static REGISTRATION: Mutex<()> = Mutex::new(());
    static INTERRUPT_WRITER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
    static TERMINATE_WRITER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
    static HANGUP_WRITER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
    static QUIT_WRITER: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

    pub(super) struct PlatformSignal {
        reader: tokio::io::unix::AsyncFd<UnixStream>,
        _registration: Registration,
    }

    struct Registration {
        signal: libc::c_int,
        previous: libc::sigaction,
        _writer: UnixStream,
    }

    impl PlatformSignal {
        pub(super) fn new(signal: Signal) -> io::Result<Self> {
            let signal = signal_number(signal);
            let slot = writer_slot(signal);
            let (reader, writer) = UnixStream::pair()?;
            reader.set_nonblocking(true)?;
            writer.set_nonblocking(true)?;

            let _guard = REGISTRATION
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if slot.load(std::sync::atomic::Ordering::Relaxed) != -1 {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "this process signal already has an urushi subscription",
                ));
            }

            let mut previous = MaybeUninit::uninit();
            // SAFETY: `previous` points to writable storage and a null second
            // argument asks sigaction to inspect without changing the handler.
            if unsafe { libc::sigaction(signal, std::ptr::null(), previous.as_mut_ptr()) } == -1 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: the successful sigaction call initialized `previous`.
            let previous = unsafe { previous.assume_init() };

            // SAFETY: zero is a valid starting representation for sigaction;
            // sigemptyset initializes its mask before the value is installed.
            let mut action: libc::sigaction = unsafe { std::mem::zeroed() };
            action.sa_sigaction = dispatch as *const () as usize;
            action.sa_flags = libc::SA_RESTART;
            // SAFETY: `action.sa_mask` is valid writable storage.
            if unsafe { libc::sigemptyset(&mut action.sa_mask) } == -1 {
                return Err(io::Error::last_os_error());
            }

            slot.store(writer.as_raw_fd(), std::sync::atomic::Ordering::Relaxed);
            // SAFETY: `action` contains an async-signal-safe handler and an
            // initialized mask. The previous action was captured above.
            if unsafe { libc::sigaction(signal, &action, std::ptr::null_mut()) } == -1 {
                slot.store(-1, std::sync::atomic::Ordering::Relaxed);
                return Err(io::Error::last_os_error());
            }

            Ok(Self {
                reader: tokio::io::unix::AsyncFd::new(reader)?,
                _registration: Registration {
                    signal,
                    previous,
                    _writer: writer,
                },
            })
        }

        pub(super) async fn recv(&mut self) -> bool {
            loop {
                let mut ready = match self.reader.readable().await {
                    Ok(ready) => ready,
                    Err(_) => return false,
                };
                let mut byte = [0];
                match ready.try_io(|inner| {
                    let mut reader = inner.get_ref();
                    reader.read(&mut byte)
                }) {
                    Err(_) => continue,
                    Ok(Ok(0)) => return false,
                    Ok(Ok(_)) => return true,
                    Ok(Err(_)) => return false,
                }
            }
        }
    }

    impl Drop for Registration {
        fn drop(&mut self) {
            let _guard = REGISTRATION
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // SAFETY: `previous` came from sigaction for this signal and stays
            // alive for the duration of the restoring call.
            unsafe {
                libc::sigaction(self.signal, &self.previous, std::ptr::null_mut());
            }
            writer_slot(self.signal).store(-1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    extern "C" fn dispatch(signal: libc::c_int) {
        let writer = writer_slot(signal).load(std::sync::atomic::Ordering::Relaxed);
        if writer != -1 {
            let byte = [1_u8];
            // SAFETY: the descriptor remains open until after the previous
            // handler is restored. write is async-signal-safe on Unix.
            unsafe {
                libc::write(writer, byte.as_ptr().cast(), byte.len());
            }
        }
    }

    const fn signal_number(signal: Signal) -> libc::c_int {
        match signal {
            Signal::Interrupt => libc::SIGINT,
            Signal::Terminate => libc::SIGTERM,
            Signal::Hangup => libc::SIGHUP,
            Signal::Quit => libc::SIGQUIT,
        }
    }

    fn writer_slot(signal: libc::c_int) -> &'static std::sync::atomic::AtomicI32 {
        match signal {
            libc::SIGINT => &INTERRUPT_WRITER,
            libc::SIGTERM => &TERMINATE_WRITER,
            libc::SIGHUP => &HANGUP_WRITER,
            libc::SIGQUIT => &QUIT_WRITER,
            _ => unreachable!("unsupported signal"),
        }
    }

    #[cfg(test)]
    mod tests {
        use std::os::unix::process::ExitStatusExt;
        use std::process::Command;

        use super::*;

        extern "C" fn prior_handler(_: libc::c_int) {}

        #[test]
        fn dropping_listener_restores_previous_handler() {
            let _guard = REGISTRATION
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let signal = libc::SIGHUP;
            let mut original = MaybeUninit::uninit();
            // SAFETY: the pointers refer to valid action storage.
            assert_eq!(
                unsafe { libc::sigaction(signal, std::ptr::null(), original.as_mut_ptr()) },
                0
            );
            // SAFETY: sigaction initialized `original` after succeeding.
            let original = unsafe { original.assume_init() };
            // SAFETY: zeroed sigaction plus an initialized mask is installable.
            let mut prior: libc::sigaction = unsafe { std::mem::zeroed() };
            prior.sa_sigaction = prior_handler as *const () as usize;
            // SAFETY: `prior.sa_mask` is writable and `prior` is then valid.
            assert_eq!(unsafe { libc::sigemptyset(&mut prior.sa_mask) }, 0);
            assert_eq!(
                // SAFETY: both action pointers are valid for this call.
                unsafe { libc::sigaction(signal, &prior, std::ptr::null_mut()) },
                0
            );
            drop(_guard);

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .unwrap();
            let _runtime = runtime.enter();
            let listener = PlatformSignal::new(Signal::Hangup).unwrap();
            drop(listener);

            let mut restored = MaybeUninit::uninit();
            // SAFETY: `restored` points to writable action storage.
            assert_eq!(
                unsafe { libc::sigaction(signal, std::ptr::null(), restored.as_mut_ptr()) },
                0
            );
            // SAFETY: sigaction initialized `restored` after succeeding.
            let restored = unsafe { restored.assume_init() };
            assert_eq!(restored.sa_sigaction, prior.sa_sigaction);

            // SAFETY: restore the test process's original action.
            assert_eq!(
                unsafe { libc::sigaction(signal, &original, std::ptr::null_mut()) },
                0
            );
        }

        #[test]
        fn dropping_listener_restores_default_termination() {
            const CHILD: &str = "URUSHI_SIGNAL_RESTORE_CHILD";
            if std::env::var_os(CHILD).is_some() {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_io()
                    .build()
                    .unwrap();
                let _runtime = runtime.enter();
                drop(PlatformSignal::new(Signal::Terminate).unwrap());
                // SAFETY: raising SIGTERM in this dedicated child process is
                // the behavior under test and cannot affect the parent runner.
                unsafe {
                    libc::raise(libc::SIGTERM);
                }
                panic!("SIGTERM remained intercepted after listener drop");
            }

            let status = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "sources::unix_signal::tests::dropping_listener_restores_default_termination",
                ])
                .env(CHILD, "1")
                .status()
                .unwrap();
            assert_eq!(status.signal(), Some(libc::SIGTERM));
        }
    }
}

#[cfg(unix)]
use unix_signal::PlatformSignal;

#[cfg(windows)]
struct PlatformSignal(tokio::signal::windows::CtrlC);

#[cfg(windows)]
impl PlatformSignal {
    fn new(signal: Signal) -> io::Result<Self> {
        if signal != Signal::Interrupt {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this process signal is unavailable on Windows",
            ));
        }
        tokio::signal::windows::ctrl_c().map(Self)
    }

    async fn recv(&mut self) -> bool {
        self.0.recv().await.is_some()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use super::*;
    use crate::delivery::Delivery;
    use crate::executor::{TokioClock, TokioExecutor};
    use crate::subscription::Subscription;
    use crate::testing::Ready;

    #[test]
    fn production_spawner_drives_async_blocking_stream_and_interval_sources() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let executor: Arc<dyn Executor> = Arc::new(TokioExecutor::new(runtime.handle().clone()));
        let spawner = RuntimeSourceSpawner::new(
            Arc::clone(&executor),
            Arc::new(TokioClock) as Arc<dyn Clock>,
        );
        let deliveries = DeliveryQueue::new();

        let sources = [
            Subscription::run("async", |sender| async move {
                let _ = sender.send(1).await;
            }),
            Subscription::run_blocking("blocking", |sender| {
                let _ = sender.blocking_send(2);
            }),
            Subscription::stream("stream", Ready::new([3])),
            Subscription::interval("interval", Duration::from_millis(1), |_| 4),
        ];
        let running: Vec<_> = sources
            .into_iter()
            .map(|subscription| {
                let source = subscription
                    .into_sources()
                    .pop()
                    .expect("one source was declared");
                spawner
                    .start(source, deliveries.clone())
                    .expect("production source starts")
            })
            .collect();

        let received = runtime.block_on(async {
            let mut received = BTreeSet::new();
            while received.len() != 4 {
                let delivery = tokio::time::timeout(Duration::from_secs(1), deliveries.next())
                    .await
                    .expect("source delivers before timeout");
                let Delivery::Async(message) = delivery else {
                    panic!("application sources are asynchronous deliveries");
                };
                received.insert(message);
            }
            received
        });
        assert_eq!(received, BTreeSet::from([1, 2, 3, 4]));
        drop(running);
    }

    #[test]
    fn application_source_refresh_uses_the_latest_subscription_mapper() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let executor: Arc<dyn Executor> = Arc::new(TokioExecutor::new(runtime.handle().clone()));
        let spawner = RuntimeSourceSpawner::new(
            Arc::clone(&executor),
            Arc::new(TokioClock) as Arc<dyn Clock>,
        );
        let deliveries = DeliveryQueue::new();
        let (source_tx, source_rx) = std::sync::mpsc::channel();
        let initial = Subscription::run_blocking("source", move |sender| {
            source_tx.send(sender).expect("test receives source sender");
        })
        .map(|value| value + 10)
        .into_sources()
        .pop()
        .expect("one source was declared");
        let mut running = spawner
            .start(initial, deliveries.clone())
            .expect("production source starts");

        let sender = source_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("blocking source starts");
        sender.blocking_send(1).expect("first message is admitted");
        assert_eq!(runtime.block_on(deliveries.next()), Delivery::Async(11));

        let refreshed = Subscription::run_blocking("source", |_| {}).map(|value: i32| value + 100);
        running.refresh(
            refreshed
                .into_sources()
                .pop()
                .expect("one source was declared"),
        );
        sender
            .blocking_send(2)
            .expect("message after refresh is admitted");
        assert_eq!(runtime.block_on(deliveries.next()), Delivery::Async(102));
    }
}
