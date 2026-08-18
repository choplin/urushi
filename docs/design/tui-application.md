# The TUI Application Value

The Rust shape of a full-screen application: the `Application` trait, the
`Effect` and `Subscription` values it returns, the `Key` that identifies a
replaceable effect or a running subscription, how an application requests
shutdown, and the bounds on each type.
[`tui-architecture.md`](../tui-architecture.md) states the trait under
"Application model"; this file holds the exact forms and why they are these and
not the nearby alternatives.

## The rule

### Application

```rust
pub trait Application {
    type Model;
    type Message: Send + 'static;

    fn init(&self) -> (Self::Model, Effect<Self::Message>);
    fn update(&self, model: &mut Self::Model, message: Self::Message)
        -> Effect<Self::Message>;
    fn view(&self, model: &Self::Model) -> View;
    fn subscriptions(&self, model: &Self::Model) -> Subscription<Self::Message>;
}
```

`Self` is the program's description; `Model` is the state the runtime owns and
lends to `update` mutably, one call per accepted message. `view` returns
[`urushi::view::View`](../../urushi/src/view/model.rs), as
[`tui-view.md`](tui-view.md) settles.

### Effect

An `Effect<Message>` is built from these constructors and composes without an
executor type appearing in the application:

| Constructor | Carries |
| --- | --- |
| `Effect::none()` | nothing |
| `Effect::perform(work)` | blocking work, `FnOnce() -> Message + Send + 'static`, run off the update thread |
| `Effect::future(future)` | asynchronous work, `Future<Output = Message> + Send + 'static` |
| `Effect::perform_latest(key, work)` | `perform`, replaceable under `key` |
| `Effect::future_latest(key, future)` | `future`, replaceable under `key` |
| `Effect::batch(effects)` | several effects started concurrently |
| `Effect::shutdown()` | the request to stop the runtime |
| `effect.map(f)` | the same effect with its message passed through `f: Fn(A) -> B + Send + Sync + 'static` |

`batch` promises nothing about the order its members' completions arrive in.
Work that must follow other work returns its next effect from the `update`
that receives the first completion; there is no sequencing combinator.

Two `*_latest` effects with the same `Key` are the same replaceable work:
starting the second replaces the first, and the runtime suppresses the replaced
completion under the freshness rule in [`tui-effects.md`](tui-effects.md).

Which executor runs a closure or polls a future is a runtime concern behind a
replaceable boundary; the effect value names none.

### Subscription

A `Subscription<Message>` declares a source that stays alive and sends messages
for as long as the application keeps declaring it. Nothing reaches `update`
from a source the application did not declare — terminal input included.

| Constructor | Source |
| --- | --- |
| `Subscription::none()` | nothing |
| `Subscription::input(f)` | terminal key and text input, `f: Fn(Input) -> Message + Send + Sync + 'static` |
| `Subscription::surface(f)` | surface observations, `f: Fn(Surface) -> Message + Send + Sync + 'static` |
| `Subscription::interval(period, f)` | a timer, `f: Fn(Instant) -> Message + Send + Sync + 'static` |
| `Subscription::stream(key, stream)` | an application-defined source that is a `Stream<Item = Message> + Send + 'static` |
| `Subscription::run(key, f)` | an application-defined asynchronous source, `f: FnOnce(Sender<Message>) -> Fut + Send + 'static`, `Fut: Future<Output = ()> + Send` |
| `Subscription::run_blocking(key, f)` | an application-defined blocking source, `f: FnOnce(Sender<Message>) + Send + 'static`, run on its own thread |
| `Subscription::terminal_errors(f)` | failures the terminal reports while drawing, `f: Fn(io::Error) -> Message + Send + Sync + 'static` |
| `Subscription::batch(subscriptions)` | several sources |
| `subscription.map(f)` | the same source with its message passed through `f: Fn(A) -> B + Send + Sync + 'static` |

The function an application passes turns the source's own value into the
application's `Message`; the runtime does not know that type and cannot
deliver without one.

Every subscription has an identity, a `Key`. After each `update` the runtime
reconciles the declaration against the sources it is running: a declared
subscription whose key is running keeps running, one whose key is not running
starts, and a running one whose key is no longer declared stops.
Runtime-provided constructors derive the key themselves — `input` and
`surface` are singletons and `interval` is keyed by its period — and an
application-defined source is given its key by the application, so a key built
from a model value restarts the source when that value changes. The mapping
function is not part of the identity: when a running subscription is declared
again, messages from then on pass through the function of the most recent
declaration.

An application-defined source is a `Stream`, or a function given a `Sender`
whose `send` waits when the source's admission policy says so. Each of the
three constructors has a `*_with(key, admission, …)` form that names an
`Admission` policy explicitly; without it the source is a bounded FIFO with
backpressure. The policies, and what a `Sender` does under each, are defined in
[`tui-delivery-ordering.md`](tui-delivery-ordering.md).

### Key

A `Key` is built from any hashable value and carries the value's type as well
as its hash:

```rust
Key::of(&value)      // value: Hash + 'static
Key::from("preview") // From<&'static str>
```

Keys built from equal values of the same type are equal, and keys of different
types never are. The key names no runtime type and no backend, so an
application's own enums are its natural keys.

### Shutdown

An application requests shutdown by returning `Effect::shutdown()` from
`update`. The request is read by the runtime when it interprets that return
value and never enters admission or delivery. On that request the runtime
starts none of the other effects in the same return value, discards the
completions of effects still in flight, stops every subscription, restores the
terminal session, and returns the final model to the caller that ran the
application. Work that must finish before the application exits — saving a
document, say — is an ordinary effect whose completion message is the `update`
that returns `shutdown`.

### Bounds

`Message: Send + 'static`. Effect closures and futures are `Send + 'static`;
functions the runtime may call for many messages — subscription mappers,
`map` — are `Send + Sync + 'static` as well. `Model` and `Application` carry no
bound: the runtime keeps the model on the thread that runs `update` and
`view`, and the entry point that runs an application blocks that thread rather
than handing the model to another.

## Why a trait whose `Self` is not the model

The application and its state are two types. `Self` holds what a program is
made of — configuration, a theme, the paths it operates on — and `Model` holds
what the runtime owns and hands to `update`. This is what lets one description
be started many times, in tests as much as in a program, without rebuilding
the configuration alongside the state, and it is what makes "the runtime owns
the live model" a statement about types rather than about discipline: the
runtime holds a `Model` and borrows an `Application`, and neither can stand in
for the other.

Rejected:

- **`Self` is the model** — `update(&mut self, Message)`, `view(&self)`, as
  iced writes it. Shortest to write, and configuration and state end up in one
  struct: starting the program again means constructing the state again, and
  the value the runtime returns at exit is the whole program rather than its
  state.
- **A struct of closures** — `Application { init, update, view, subscriptions }`
  with boxed or generic function fields. It needs no `impl` block, but pays for
  that with either boxing on every call or four type parameters, and it can be
  built on top of the trait later without the reverse being possible.

## Why `update` borrows the model mutably

`update` takes `&mut Model` and returns only the effect. The by-value form —
take the model, return the next one — is the functional reading of the same
responsibility, and it costs every arm of every `update` a `(model, effect)`
tuple. What it buys, an enum model transitioning by `match model { … }`, a
struct with an enum field gets under `&mut` as well. What `&mut` states that
by-value cannot is that this call may change the model and no other call may:
`view` and `subscriptions` take `&Model`, and the two receivers between them
say what the functional form has to say in prose.

## Why effects carry both blocking work and futures

The work the runtime exists to keep out of `update` — file and Git access,
highlighting, layout, rasterization — is blocking, so a closure is the direct
spelling; and an application whose I/O is already async should not have to
wrap it in a thread. Offering one and telling the other half to adapt would
either force an async executor on programs with no async work or push blocking
work through a `spawn_blocking` the application must reach for by name.

Neither constructor names an executor. What runs the closure and what polls the
future sits behind a boundary the runtime owns, so an application written
against one executor does not change when the boundary is implemented with
another.

Rejected:

- **Futures only** — every application acquires an executor dependency, and
  blocking work needs an executor-specific escape.
- **Closures only** — bubbletea's `Cmd`; async programs block a thread per
  effect or run their own executor beside the runtime.

## Why effects and subscriptions have `map`

A parent program whose `Message` wraps a child's — `Message::Editor(editor::Message)` —
calls the child's `update` and receives an `Effect<editor::Message>`. Without
`map` it cannot return that effect: the child would have to know the parent's
message type, which inverts the dependency and stops the child from being a
program of its own. `map` on `Effect` and on `Subscription` is the one runtime
facility a nested program structure needs; dispatch, projection, and intents
are the application's own arrangement and appear in no runtime type.

Not provided: a sequencing combinator. Work that must follow other work returns
its next effect from the `update` that receives the first completion, which is
already how an application observes the first completion at all.

## Why one `Key` for replacement and reconciliation

Latest-only effects and subscriptions both need the runtime to recognize "the
same one again", and neither can be compared as a value because both hold
closures. A `Key` built from any hashable value with its type recorded serves
both: an application keys its replaceable layout by `Layout(doc_id)` and its
file watcher by `Watch(root)`, and a key that embeds a model value restarts the
subscription when the value changes with no code beyond the declaration.

Rejected:

- **String keys** — writable, but every distinct instance becomes a formatted
  string and nothing keeps `"layout"` from meaning two things. `From<&'static
  str>` keeps the short spelling for the cases where a name is enough.
- **A key type parameter on the runtime** — one more type on every runtime
  type, to gain a guarantee the type-plus-hash key already gives.

## Why input is a subscription and not always on

The runtime reads terminal input, but it cannot deliver a key event to
`update` without a function from that event to the application's `Message`,
which only the application can write. That function is what a subscription
constructor takes, so the same declaration path serves input, surface facts,
timers, and application-defined sources, and a program that must not receive
input in some state — an external editor owns the terminal — declares no input
subscription in that state.

Rejected: an `on_input` method on `Application`, always consulted. It removes
one line from every program and adds a fifth method and a second delivery path
for one source.

## Why an application source is a stream, and also a function

A long-lived source is a stream: it yields values over time and the consumer
sets the pace. The libraries an application reaches for — a terminal event
reader, a filesystem watcher, a socket, a timer — already hand out `Stream`s,
so `Subscription::stream(key, s)` takes them with no glue, and back-pressure
falls out of the runtime polling only when it has room, with nothing for the
source to write. A stream and a function handed a `Sender` are interconvertible
— a channel one way, a `while let … send().await` the other — so neither is
more capable; what the function form adds is one shape for a source the
application writes itself, asynchronous or blocking, since `run` and
`run_blocking` differ only in whether the body awaits. All three are thin over
one runtime path: a spawned task that pulls or is pushed and hands each item to
the source's admission policy.

Rejected as the only form: the `Sender` function alone — every library stream
needs a loop to forward it; the stream alone — a blocking source needs a
thread-and-channel helper the runtime would have to provide anyway, and an
asynchronous source written by hand needs a channel to become a stream.

## Why the latest mapper wins

When a subscription with a running key is declared again, its mapping function
may have changed — a closure that captured a model value now captures a newer
one. From that reconcile on, messages pass through the latest declaration's
function. The alternative, fixing the function at start, is simpler to
implement and lets a stale capture keep running with nothing to say it did; a
program should not capture model state in a mapper, but the runtime should be
correct when one does.

## Why shutdown is an effect and not a message

`Effect::shutdown()` is returned from `update` like any other effect, so it
lives where a program's other consequences live, and it is not a `Message`, so
it never enters admission or delivery and the runtime never has to recognize a
privileged variant. Its composition rule follows from that: it is read from
`update`'s return value, and from there the runtime stops — no sibling effect
starts, in-flight completions are discarded, subscriptions stop, the session is
restored, and the final model is returned. Work that must precede exit is an
effect whose completion is the `update` that shuts down.

Rejected:

- **A `should_exit(&Model) -> bool` the runtime polls** — declarative, but
  every program grows a flag in its model to say what one returned value can
  say.
- **A transition struct returned from `update`** — `{ effect, exit }` on every
  arm, for a field that is almost always false.
- **A quit message** — bubbletea's `QuitMsg`; the runtime would inspect
  application messages, which the admission contract forbids.

## Why the bounds are what they are

Effects complete on other threads and tasks and must send their message back,
so `Message: Send + 'static` is forced. The closure bounds are the strict
reading of where the runtime may run them; a bound can be relaxed later
without breaking any program and cannot be added later without breaking some,
so uncertain cases lean strict. `Model` is left unbounded on purpose: `update`
and `view` run on one thread and the entry point that runs an application
keeps the model there, which is a constraint on that entry point's shape
rather than on the model.
