# TUI Runtime Architecture

This document defines the target architecture for a full-screen TUI runtime in
Urushi.
It is a normative design for planned work, not a description of code that is
already implemented.
[`architecture.md`](architecture.md) remains the source of truth for the
repository as it exists today.

The runtime adds a TEA-style application framework above Ratatui while leaving
Ratatui responsible for widgets, layout, buffers, backends, and cell-level
diffing.
It extends the existing `urushi-tui` Ratatui adapter without making
the existing renderer-neutral line model depend on Ratatui.

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
navigation, modal stacks, key bindings, or commands.
Those remain ordinary model and message logic.

## Relationship to the implemented architecture

The current architecture has three boundaries that the TUI subsystem must
preserve.

First, `style`, `text`, `theme`, the existing line-oriented `view`, and reusable
components remain semantic modules without terminal lifecycle ownership.
The TUI runtime may consume their values but they must not depend on the
runtime.

Second, the existing modules in [`urushi-tui`](../urushi-tui/) remain output adapters.
They convert Urushi styles and box-model values into Ratatui representations
and draw into a supplied buffer.
The runtime orchestrates those adapters; the adapters do not acquire
application state, event handling, or terminal ownership.

Third, [`urushi-prompt`](../urushi-prompt/) remains a prompt-specific crate.
Its line-oriented editing, submission, viewport, cursor, and cleanup policy do
not become the default policy for full-screen applications.
Sharing lower-level terminal facilities in the future must not merge the two
interaction models.

`urushi-tui` is the provisional crate boundary for this subsystem. Runtime
implementation may refine its internal modules without moving Ratatui concerns
back into the core `urushi` crate.

## Architecture at a glance

The application describes state transitions and declarations.
The runtime owns execution and terminal resources.

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

`Application` and `Runtime` are deliberately separate.
An application can be composed, passed around, and tested as a description of
a program.
The runtime interprets that description and owns all live execution state.

## Application model

Conceptually, an application contains four functions:

```text
Application<Model, Message, View> {
  init          : () -> (Model, Effects<Message>)
  update        : (Model, Message) -> (Model, Effects<Message>)
  view          : &Model -> View
  subscriptions : &Model -> Subscriptions<Message>
}
```

This notation defines responsibility rather than the final Rust signature.
The concrete API may use function pointers, closures, or generic parameters,
but `Application` remains a pure value rather than a mutable object with
lifecycle methods.

The runtime owns the live `Model`.
It invokes `update` once for every accepted message in delivery order and
replaces the live model with the returned value.
`update` does not perform terminal I/O or mutate runtime resources.

`subscriptions` declares long-lived message sources as a function of the
current model.
The runtime reconciles the declaration with running sources and delivers their
events through the same admission and ordering path as other messages.

`view` returns a declarative value that a renderer can draw with Ratatui.
It must be cheap enough to evaluate at a normal drawing opportunity because
expensive preparation belongs in effects.

### Meaning of View

Within this document, `View` means the declarative render input produced by a
TUI application's `view` function.
The final Rust representation is intentionally open.
It may be an application-owned value, a function that composes Ratatui widgets,
or another renderer input that can be consumed during a draw.

This term does not silently redefine the existing
[`urushi::view::View`](../urushi/src/view/model.rs), which is a concrete ordered
collection of styled lines and spans.
That line-oriented type can be embedded or adapted where useful, but it is not
required to become the canonical full-screen layout tree.
Urushi also does not introduce an independent resolved render tree merely to
mirror another framework.

## Effects and expensive preparation

An `Effect<Message>` describes work to be interpreted by the runtime.
It cannot update the model directly.
Its observable result returns as a message and passes through normal delivery.

Effects include filesystem and Git access, syntax highlighting, document
layout, image rasterization, and other work that would make `update` or `view`
too expensive.
This separation allows every key input to advance the logical model without
starting physical rendering or heavy preparation for every intermediate state.

The runtime must support at least these execution policies:

- ordinary one-shot work whose completion is delivered in source order;
- long-lived work represented as a subscription; and
- keyed latest-only work for replaceable preparation.

For latest-only work, the runtime may suppress a completion only when the
runtime itself knows that the corresponding execution was cancelled or
replaced.
For ordinary effects, the application decides in `update` whether a completion
still applies to the current model.
A generation identifier in the application's message and model is one possible
way to make that decision; it is not a runtime-wide damage model.

In a document viewer, for example, changing the document or theme can start a
new keyed layout or rasterization effect.
Moving the viewport can reuse the prepared asset already stored in the model
and render it at a new position without starting that preparation again.

## Admission and delivery

Message handling has two distinct stages.

An admission policy belongs to a message source and decides whether an incoming
item is accepted, delayed with backpressure, replaced, or rejected.
Once accepted, a delivery receives one position in a single runtime-wide
order.
The runtime never inspects application message variants to infer admission
policy.

A delivery contains one or more messages and a delivery mode:

```text
Delivery<Message> {
  mode: Async | Sync,
  messages: NonEmpty<Message>,
}
```

Messages within a delivery retain their order.
Deliveries from all sources are processed in the runtime-wide accepted order.
A `Sync` delivery does not overtake an earlier accepted `Async` delivery.

Initial source policies should support these common cases:

| Source | Admission behavior |
| --- | --- |
| Terminal key and text input | Bounded FIFO with reader backpressure |
| Surface observations | Latest value may replace an unaccepted observation |
| Ordinary effect completion | FIFO |
| Cancelled latest-only effect | Suppress the known-cancelled completion |
| Shutdown | Separate runtime control path |

Key repeat is not aggregated by default.
Each accepted key event reaches `update`, so selection, cursor, viewport, and
other logical state stay responsive and deterministic.
An application or source-specific policy may aggregate input when its own
semantics permit that optimization.

## Async delivery and draw scheduling

`Async` is the default delivery mode.
Key input, text input, cursor movement, selection changes, viewport movement,
and ordinary effect completions normally use it.
A message is not `Sync` merely because it changes visible content.

The runtime processes accepted messages independently of physical drawing.
It may consume several messages and then draw only the latest resulting model
when the terminal is ready.
This is draw coalescing, not message coalescing: no accepted logical transition
is skipped.

Draw scheduling is a runtime policy rather than an application command.
The core application API therefore has no `request_draw`, `plan_draw`, public
`Damage`, or public `Invalidation` type.
The application expresses changes through its model, effects, and view.

The initial runtime also does not require `View` equality as a prerequisite for
skipping a draw.
Ratatui already compares cell buffers and emits only changed cells.
Caching or equality checks above that layer should be introduced only after
measurement identifies a material benefit and a correct ownership boundary.

## Sync delivery as a render barrier

`Sync` is an exceptional delivery mode for changes to the logical rendering
environment.
Examples include terminal dimensions, cell pixel dimensions, or a graphics
capability change that the application must reflect in its model before the
corresponding frame is built.

When the runtime accepts a `Sync` delivery, it:

1. cancels any pending draw that has not started;
2. processes all earlier accepted deliveries in order;
3. applies every message in the `Sync` batch through `update` without drawing
   intermediate models;
4. evaluates `view` once from the model after the complete batch; and
5. draws that view with the matching logical rendering-environment snapshot
   before processing later deliveries.

If another `Sync` delivery arrives during a draw, the runtime does not interrupt
the draw already in progress.
It queues the new delivery as the next generation in the global order.

This barrier guarantees agreement among the delivered environment facts, the
model after `update`, and the logical snapshot used to build the frame.
It does not freeze the operating system's physical terminal surface during the
draw and does not claim that terminal output is an atomic transaction.

### Surface information belongs in state transitions

Physical rendering constraints are handled by the runtime, terminal, Ratatui,
and backend.
An application only needs surface information when that information affects
application semantics, such as layout choices or a viewport measured in cells.

Such information enters through a subscription as a message.
The application stores the logical facts it needs in its model, and `view`
reads them from that model like any other state.
`view` does not receive an implicit `ViewContext` or physical `Surface` input.

The runtime cannot use a revision number alone to decide whether an application
has handled a surface change because the runtime does not understand arbitrary
application messages.
The `Sync` delivery contract provides the required coordination without
inspecting those messages.

## Startup barrier

The first frame must not be built from placeholder surface information when an
initial surface observation is available.
The runtime therefore establishes a startup barrier before the first draw.

During startup, the runtime:

1. calls `init` and evaluates the initial subscriptions;
2. collects initial `Sync` deliveries from those subscriptions;
3. applies their messages in deterministic order, batching related surface
   facts where possible;
4. reevaluates model-dependent subscriptions when those messages change the
   subscription set;
5. repeats until no new initial `Sync` delivery is produced; and
6. evaluates `view` once and performs the first draw.

Initial `Async` items are staged during this process.
They enter normal admission and receive global ordering only after the first
draw, so they cannot make the initial rendering environment inconsistent.

The concrete implementation must bound startup reconciliation and detect a
cycle or budget exhaustion.
The public failure behavior for such a case remains an API design decision.

## Rendering ownership

The shared rendering vocabulary is `View`, `Renderer`, `Frame`, `Terminal`,
`TerminalSession`, and `Backend`.
These names describe ownership boundaries; they do not require Urushi to
reimplement Ratatui types that already satisfy them.

### Renderer

A `Renderer` translates a TUI `View` into drawing operations during
`Terminal.draw`.
It may compose Ratatui widgets and reuse the existing Urushi Ratatui adapters.
It does not own the model, scheduling, a backend, or session restoration.

### Frame

A `Frame` is a borrowed, draw-scoped handle to terminal-owned working
presentation state.
At minimum, it provides access to the current cell buffer and the cursor request
for that draw.
It does not own the previous buffer, backend, diff algorithm, output stream, or
flush operation.

The final API may use `ratatui::Frame` directly or place an Urushi-owned adapter
around it.
Either choice must preserve this borrowed ownership model.

### Terminal

`Terminal` owns or delegates ownership of working and committed presentation
state, the backend, cell diffing, output, and flushing.
It treats a presentation as committed only after output succeeds.
The implementation must verify Ratatui's exact failure behavior and add local
recovery state if Ratatui does not provide this guarantee at the required
boundary.

Cursor position and visibility requested for one frame belong to the frame and
terminal path.
They are not application effects and are not restoration obligations by
themselves.

### TerminalSession

`TerminalSession` owns restoration obligations caused by entering the TUI
session, including raw mode, alternate-screen state, and cursor visibility when
the session changes it.
It restores the state it is responsible for on normal exit and on supported
error and interruption paths.

The session does not promise to reconstruct exact pre-session state that the
terminal cannot report reliably, such as an unknown original cursor position.
That limitation must remain explicit in the public contract.

### Backend

`Backend` is the physical terminal-output boundary.
Backend-specific types and failure rules stay behind adapters so applications
remain expressed in Urushi-owned concepts.
Clock, event source, and backend boundaries must be replaceable in tests.

## Runtime ownership and control

The runtime owns:

- the live model;
- source admission and the accepted delivery order;
- subscription reconciliation;
- effect execution and cancellation known to the runtime;
- draw scheduling and pending-draw cancellation;
- the terminal and terminal session; and
- runtime control such as shutdown.

Shutdown is a control-path concern rather than a privileged application message
variant.
The final API still needs to decide how an application requests shutdown and
how that request composes with effects and cleanup.

The runtime must remain independent of one mandatory async executor where
practical.
Its contracts describe ordering, cancellation, and wake-up behavior rather than
exposing a particular executor's task handles throughout application types.

## Representative application flows

### Lightweight Ratatui application

For a Revia-like application, every accepted key event advances the model.
The runtime may process many such updates before the next draw, at which point
`view` reflects the latest model and Ratatui emits the cell diff.
Filesystem work or syntax highlighting runs as effects, so key handling does
not wait for them.

### Prepared assets and viewport movement

For a Raden-like application, document changes start replaceable preparation
effects whose completed assets enter the model through messages.
Viewport movement remains a lightweight logical update.
The next draw reuses the existing prepared asset with the new viewport instead
of treating every movement as damage that requires preparation.

Cell output plus terminal graphics remains an extension boundary.
The first implementation should prove the cell-only runtime before promoting a
shared graphics contract.
When that work begins, it must design asset lifetime, cell-and-graphics commit,
partial-output recovery, and text-only fallback together.

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

## Verification strategy

The implementation should establish evidence at the boundary where each
guarantee is observed.

| Guarantee | Required evidence |
| --- | --- |
| Pure state transitions | Unit tests for `init`, `update`, and `view` without a terminal |
| Global ordering | Deterministic tests with interleaved input, subscription, and effect sources |
| Bounded admission | Saturation tests for bounded FIFO, backpressure, replacement, and cancellation |
| Async draw coalescing | Tests proving every update occurs while fewer latest-state draws are allowed |
| Sync barrier | Tests proving no intermediate draw and no later delivery before the barrier draw |
| Startup | Tests for multiple initial `Sync` batches, staged `Async` input, and subscription convergence |
| Effect freshness | Tests for ordinary stale results and runtime-known latest-only cancellation |
| Terminal failures | Tests proving committed state advances only after successful output |
| Session restoration | Integration tests for normal exit, error, interruption, and partial setup failure |
| Real application fit | Revia-like and Raden-like dogfood scenarios |

The runtime test harness should provide deterministic event sources, a
controllable clock, an in-memory backend, and observable effect scheduling.
Tests should assert externally meaningful ordering and presentation behavior,
not private task structure.

## Deferred API decisions

The architecture fixes responsibilities and semantics but leaves these Rust API
choices open until implementation planning:

- the concrete generic and closure representation of `Application`;
- the concrete TUI `View` and `Renderer` types;
- whether `Frame` is Ratatui's type or a narrow Urushi adapter;
- the public form of source admission policies and keyed latest-only effects;
- executor integration without making one executor mandatory;
- error and shutdown behavior during `Sync` processing;
- startup reconciliation limits and error reporting; and
- the graphics presentation and recovery model.

These decisions may refine representation but must not collapse the ownership
boundaries or ordering guarantees defined above.

## Keeping this document current

Update this document whenever implementation changes an application/runtime
responsibility, delivery guarantee, rendering owner, or lifecycle invariant.
As parts of this design become implemented, update
[`architecture.md`](architecture.md) in the same change so that its description
of the current repository remains accurate.
