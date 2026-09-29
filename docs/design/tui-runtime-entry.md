# Running a TUI Application

How an application is started, what the entry point returns, where the executor
that runs effects comes from, and what the runtime does with an error of its
own — a failed draw or a panic in `update`.
[`tui-architecture.md`](../tui-architecture.md) summarizes this under
"Rendering and runtime ownership"; this file holds the exact contract.

## The rule

### The entry point

```rust
let model = urushi_tui::run(app)?;
```

`run` takes an `Application`, drives it on the calling thread until it shuts
down, and returns the final `Model`. It blocks: `init`, every `update`, and
every `view` run on the thread that called it, which is why `Model` needs no
`Send` bound. With no other arrangement, `run` opens the real terminal, reads
the real input, and drives effects on a Tokio current-thread executor of its
own. Future effects and asynchronous subscriptions share that executor with the
application loop. The draw scheduler owns its state transitions and uses the
runtime-provided timer while the application loop waits on the scheduler as one
event source. Blocking effects and synchronous physical presentation use
Tokio's blocking pool, so neither stops delivery processing.

`run(app)` is the short spelling of a builder that lets each replaceable
boundary be supplied:

```rust
urushi_tui::Runtime::new(app)
    .executor(executor)   // where closures and futures run
    .terminal(terminal)   // the Terminal implementation frames go to
    .backend(backend)     // the physical session, input, and query connection
    .clock(clock)         // what timers and intervals read
    .mouse(true)          // a session option; see tui-terminal-ownership.md
    .run()?
```

The builder is the one public surface for both a program that already has an
executor — it passes a Tokio handle — and a harness that passes a deterministic
executor, an in-memory frame terminal, a fake physical backend, and a
controllable clock. The split between `terminal` and `backend` preserves the
same ownership boundary as production: a `Terminal` receives committed frames,
while one complete `TerminalBackend` owns session modes, input, output, and
mutable queries. Every boundary has the real default — the Ratatui-backed
terminal over Crossterm, Tokio, the system clock — and a program that supplies
none gets what `run` gives. The session options — raw mode, alternate screen,
the input modes, mouse capture — sit on the same builder; their defaults are in
[`tui-terminal-ownership.md`](tui-terminal-ownership.md).

Those physical-terminal defaults are provided by the default `crossterm`
feature. A build that enables `runtime` without `crossterm` retains the full
runtime but must supply its physical connection with `.backend(backend)` before
`.run()` becomes available.

### The executor boundary

`Executor` is a runtime-owned trait with the shape effects need and nothing
more: run a blocking closure off the update thread, drive a future, and return
for each a handle the runtime drops to replace the work. Tokio is the one
implementation the runtime ships behind the `runtime` feature; the trait
exists so that a test can substitute a deterministic executor and so that
another executor can be supplied later without a change to any application
type. No executor's task or handle type appears in `Effect`, `Subscription`,
or `Application`.

### Errors of the runtime's own

`run` returns `Result<Model, Error>`. The variants an application can meet:

| `Error` | When |
| --- | --- |
| `Terminal(io::Error)` | the terminal could not be entered, read, queried, drawn to, or restored, and no subscription took the failure |
| `Runtime(io::Error)` | the runtime executor or its presentation worker could not be created or continue |

A failure to draw a frame is delivered rather than fatal when the application
declared `Subscription::terminal_errors(f)`: the failure reaches `update` as
`f(error)`, an `Async` message like any other, the frame that failed is not
committed, and the runtime goes on to draw again at the next opportunity. The
application decides in `update` — ignore, record, save and shut down. Without
that subscription the failure ends the run with `Error::Terminal`. The general
rule is that an error the runtime cannot hand to anyone is fatal, and the
terminal session is restored on every exit path the same way.

Input reads and surface queries have no application error subscription. Their
failure therefore follows the general fatal rule and returns
`Error::Terminal`; terminal restoration still runs before `run` returns.

A panic in `update`, `view`, `subscriptions`, or `init` is not caught. It
unwinds through `run`; `TerminalSession` restores the terminal during the
unwind, as [`tui-terminal-ownership.md`](tui-terminal-ownership.md) requires.

## Why one builder behind `run`

An application that wants nothing but a terminal should write one line, and a
test that wants to replace the terminal, the clock, and the executor should
not need a second API to do it. A builder whose every field has the real
default serves both, and `run(app)` is that builder with nothing set. The
alternative — a `run` that constructs everything and a separate test-only
constructor — puts two entry points in front of one runtime and lets them
drift.

`run` blocks rather than returning a future because the model lives on its
thread. A future that could be moved to another thread would need `Model:
Send`, and a future that could not be moved would be an async function that
must not be spawned — a constraint no signature states. A blocking `run` states
it. A program already inside a Tokio runtime hands its handle to the builder
and calls `run` from a thread it can afford to block.

## Why the executor is a boundary with one implementation

The runtime's contract with an effect is ordering, replacement, and wake-up,
none of which is Tokio's; a trait says so and keeps executor types out of
application types. But an executor an application would actually swap in does
not exist yet, and a second implementation maintained on speculation is a cost
with no user. So the boundary is real and the implementation is one, and the
harness — the second implementation that does exist — proves the boundary is
wide enough.

## Why a draw failure is delivered only on request

Most programs cannot do anything with a terminal that stopped accepting
output, and for them a fatal error with the session restored is the right
outcome, with no code to write. A program that can — one holding unsaved work
— needs the failure as a message so it can save and shut down on its own
terms. A subscription is the runtime's one way of asking "do you want these
messages?", so it is how the choice is stated, and its absence is the answer
for the programs that have nothing to say.
