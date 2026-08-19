# TUI Runtime Architecture

This document defines the architecture of Urushi's full-screen TUI runtime,
the subsystem [`architecture.md`](architecture.md) places in `urushi-tui`.

The runtime adds an application framework in the style of The Elm Architecture
(TEA) above Ratatui while leaving Ratatui responsible for widgets, layout,
buffers, backends, and cell-level diffing. It extends the `urushi-tui` Ratatui
adapter without making the renderer-neutral view model depend on Ratatui.

Four topics have files under [`design/`](design/), linked from the sections
that summarize them: what the TUI view is
([`design/tui-view.md`](design/tui-view.md)); how messages are admitted,
ordered, and drawn, including the `Sync` and startup barriers
([`design/tui-delivery-ordering.md`](design/tui-delivery-ordering.md)); how
effects run and how their freshness is decided
([`design/tui-effects.md`](design/tui-effects.md)); and who owns the terminal,
the frame, and session restoration
([`design/tui-terminal-ownership.md`](design/tui-terminal-ownership.md)).

## Goals

The TUI subsystem must provide:

- an application as a pure program value built from `init`, `update`, `view`,
  and `subscriptions`;
- ordered delivery of input, subscription events, and effect results;
- lightweight logical updates even when input arrives faster than the terminal
  can redraw;
- explicit effects for expensive preparation and external I/O;
- coordinated logical surface updates and rendering;
- one owner for frame scheduling, terminal output, and session restoration;
- deterministic tests for application logic and runtime ordering; and
- enough extension points for both ordinary Ratatui applications and future
  applications that combine cell output with terminal graphics.

The initial design does not provide application semantics such as focus,
navigation, modal stacks, key bindings, or commands. Those remain ordinary
model and message logic.

## Relationship to the rest of the architecture

Three boundaries in [`architecture.md`](architecture.md) constrain the TUI
subsystem.

First, `style`, `text`, `theme`, `view`, and reusable components are semantic
modules without terminal lifecycle ownership. The TUI runtime may consume their
values, but they must not depend on the runtime.

Second, the adapter modules in [`urushi-tui`](../urushi-tui/) are output
adapters. They convert Urushi styles and box-model values into Ratatui
representations and draw into a supplied buffer. The runtime orchestrates those
adapters; the adapters do not acquire application state, event handling, or
terminal ownership.

Third, [`urushi-prompt`](../urushi-prompt/) is a prompt-specific crate. Its
line-oriented editing, submission, viewport, cursor, and cleanup policy do not
become the default policy for full-screen applications. Sharing lower-level
terminal facilities in the future must not merge the two interaction models,
prompt-style line editing and full-screen application.

`urushi-tui` is the crate boundary for this subsystem. Its internal modules may
be refined without moving Ratatui concerns back into the core `urushi` crate.

## From event to terminal output

The application describes state transitions and declarations. The runtime owns
execution and terminal resources.

```text
event sources ---------> admission policies ----+
effect completions ----> admission policies ----+--> ordered Delivery queue
subscription events ---> admission policies ----+              |
                                                               v
                                                     Application.update
                                                        |           |
                                                        v           v
                                                      Model       Effects
                                                        |           |
                                                        |           +--> effect executor
                                                        v
                                                     Application.view
                                                        |
                                                        v
                                                      TUI View
                                                        |
                                                        v
                                                     Renderer
                                                        |
                                                        v
                                      Terminal.draw(borrowed Frame) --> Backend
```

`Application` and `Runtime` are deliberately separate. An application can be
composed, passed around, and tested as a description of a program; the runtime
interprets it and owns all live execution state.

## Application model

An application is a value implementing one trait:

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

`Self` is the description of the program — its configuration, its theme, the
roots it operates on — and `Model` is the state the runtime owns; the two are
distinct types. `Application` is a pure value rather than a mutable object with
lifecycle methods: every method takes `&self`, and nothing an application does
mutates runtime resources.

The runtime owns the live `Model`. It invokes `update` once for every accepted
message in the runtime-wide accepted order, lending the model mutably for that
call. `update` does not perform terminal I/O or mutate runtime resources;
everything it wants done outside the model it returns as an `Effect`, including
the request to shut down.

`subscriptions` declares long-lived message sources as a function of the
current model — terminal input and surface facts among them, each with the
function that turns the source's value into the application's `Message`. The
runtime reconciles the declaration with running sources and delivers their
events through the same admission and ordering path as other messages.

`view` returns a declarative value that a renderer can draw with Ratatui. It
must be cheap enough to evaluate at a normal drawing opportunity; expensive
preparation belongs in effects.

The exact forms of `Effect` and `Subscription`, the `Key` that identifies a
replaceable effect or a running subscription, the bounds on each type, and why
the shape is this one are defined in
[`design/tui-application.md`](design/tui-application.md).

### Meaning of View

Within this document, `View` means the declarative render input produced by a
TUI application's `view` function, and that type is
[`urushi::view::View`](../urushi/src/view/model.rs) — the tree of text, block,
row, and column nodes that one layout pass resolves into a rectangle under an
available area. A TUI application's `view` returns the same value a plain-CLI
call site builds, and the runtime resolves it under the terminal's area.

The tree carries one thing full-screen use requires beyond what plain output
needs: an **anchor**, a box that also carries a key, whose resolved rectangle
the caller that knows what belongs there fills. It serves the two things a
grapheme rectangle cannot express — where the terminal cursor belongs, and
where an embedded Ratatui widget draws. A view carries no scroll offset, no
focus, and no redraw hint. The anchor's rule, and why one keyed box serves both
needs, are recorded in [`design/tui-view.md`](design/tui-view.md).

Whatever the TUI `View` becomes, it is `urushi::view::View` or a value that
embeds it; Urushi does not introduce a second, independent resolved render tree
beside the one `resolve` already produces merely because Ratatui or another TUI
framework has one.

## Admission, delivery, and drawing

Message handling has two distinct stages. An **admission policy** belongs to a
message source and decides whether an incoming item is accepted, delayed with
backpressure, replaced, or rejected; the runtime never inspects application
message variants to infer it. Once accepted, a **delivery** receives one
position in a single runtime-wide order. A delivery contains one or more
messages and a mode:

```text
Delivery<Message> {
  mode: Async | Sync,
  messages: NonEmpty<Message>,
}
```

Deliveries from all sources are processed in the runtime-wide accepted order; a
`Sync` delivery does not overtake an earlier accepted `Async` one.

`Async` is the default. The runtime processes accepted messages independently
of physical drawing: it may consume several messages and then draw only the
latest resulting model when the terminal is ready. This is draw coalescing, not
message coalescing — no accepted logical transition is skipped. Draw scheduling
is a runtime policy, not an application command; the application API has no
`request_draw`, no public damage or invalidation type, and no `View` equality
prerequisite for skipping a draw, because Ratatui's cell diff already does that
work.

`Sync` is the exceptional mode for changes to the logical rendering
environment — terminal dimensions, cell pixel dimensions, a graphics
capability. Accepting a `Sync` delivery is a render barrier: earlier deliveries
are processed, the whole batch is applied without intermediate draws, and
`view` is evaluated and drawn once with the matching environment snapshot
before later deliveries. The first frame is guarded the same way: a startup
barrier applies initial `Sync` deliveries to convergence before the first
draw, staging initial `Async` input until after it.

Surface information reaches the application only through a subscription as a
message, retained in the model when needed; `view` receives no implicit
context.

The per-source admission behaviors, the exact barrier and startup steps, and
how each is verified are defined in
[`design/tui-delivery-ordering.md`](design/tui-delivery-ordering.md).

## Effects

An `Effect<Message>` describes work to be interpreted by the runtime. It cannot
update the model directly; its observable result returns as a message and
passes through normal delivery. Effects carry filesystem and Git access, syntax
highlighting, document layout, image rasterization, and other work that would
make `update` or `view` too expensive, so that every key input can advance the
logical model without starting heavy preparation for every intermediate state.

An effect carries either blocking work or a future, names no executor, and
composes by `batch` and `map`; the runtime supports one-shot work, long-lived
work as a subscription, and keyed latest-only work for replaceable preparation.
Whether a completion is still relevant is split: the runtime suppresses a
completion only when it knows the execution was canceled or replaced; otherwise
the application decides in `update`. The policies, the freshness rule, and the
representative flows are defined in [`design/tui-effects.md`](design/tui-
effects.md).

## Rendering and runtime ownership

The shared rendering vocabulary is `View`, `Renderer`, `Frame`, `Terminal`,
`TerminalSession`, and `Backend`. These names describe ownership boundaries;
they do not require Urushi to reimplement Ratatui types that already satisfy
them.

| Name | Owns |
| --- | --- |
| `Renderer` | Resolving the view once per frame, writing the `ResolvedView` into the frame's buffer, and serving the anchored rectangles that resolution reported. Not the model, scheduling, a backend, or session restoration. |
| `Frame` | A borrowed, draw-scoped handle to the current cell buffer and the cursor request. Not the previous buffer, backend, diff, output stream, or flush. |
| `Terminal` | Working and committed presentation state, the backend, cell diffing, output, and flushing. A presentation is committed only after output succeeds. |
| `TerminalSession` | Restoration obligations caused by entering the session — raw mode, alternate screen, cursor visibility — on normal exit and on supported error and interruption paths. |
| `Backend` | The physical terminal-output boundary, behind adapters, replaceable in tests. |

The runtime itself owns the live model; source admission and the accepted
delivery order; subscription reconciliation; effect execution and cancellation
known to the runtime; draw scheduling and pending-draw cancellation; the
terminal and terminal session; and runtime control such as shutdown. Shutdown
is a control-path concern rather than a privileged application message variant:
an application requests it by returning `Effect::shutdown()` from `update`, and
the runtime reads the request from that return value.

An application is started with `run(app)`, which blocks the calling thread,
drives `update` and `view` there, and returns the final model; a builder
behind it lets the executor, backend, and clock be supplied, and is how tests
replace them. Effects run behind an executor boundary the runtime owns, with
Tokio as its one shipped implementation; no executor type appears in an
application. An error the runtime cannot hand to the application — a draw
failure it did not subscribe to, a startup that does not converge — ends the
run with the session restored. The entry point, the executor boundary, and the
error rule are defined in
[`design/tui-runtime-entry.md`](design/tui-runtime-entry.md).

Cell output plus terminal graphics remains an extension boundary: the first
implementation proves the cell-only runtime before promoting a shared graphics
contract.

What each owner does at the boundary — the commit guarantee Ratatui does not
give, cursor restoration, what a session cannot promise to restore — and the
representation choices left open are defined in
[`design/tui-terminal-ownership.md`](design/tui-terminal-ownership.md).

## Architectural invariants

Implementation of the TUI subsystem must preserve these invariants:

1. `Application` is a pure program value, while `Runtime` owns live execution
   state and resources.
2. Every accepted message reaches `update` exactly once in global delivery
   order unless runtime shutdown terminates processing under a documented rule.
3. Admission policy belongs to the source and never depends on runtime
   inspection of application message variants.
4. Logical model updates are independent of physical draw frequency.
5. Expensive preparation and external I/O enter through effects and return
   through messages.
6. `Async` is the default; `Sync` is reserved for a logical rendering-environment
   barrier.
7. A completed `Sync` batch produces one view and one draw before later
   deliveries are processed.
8. Application-visible surface information reaches `update` through a
   subscription message and is retained in the model when needed.
9. The framework does not expose draw planning or a universal damage model as
   an application responsibility.
10. `Frame` is borrowed and draw-scoped; terminal presentation history and
    output remain terminal-owned.
11. Session setup and restoration have one explicit owner.
12. Existing semantic modules and `urushi-tui` adapters keep the dependency
    direction documented in [`architecture.md`](architecture.md).

The architecture fixes responsibilities and semantics but leaves the concrete
Rust API open until implementation planning; each design file lists the
choices it leaves open. Those decisions may refine representation but must not
collapse the ownership boundaries or ordering guarantees defined above.
