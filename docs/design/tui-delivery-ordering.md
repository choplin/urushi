# TUI Delivery Ordering and Draw Scheduling

How the runtime admits messages from each source, orders them, decides when to
draw, and guards the frame with the `Sync` render barrier and the startup
barrier. [`tui-architecture.md`](../tui-architecture.md) states the model under
"Admission, delivery, and drawing"; this file holds the exact steps, the
per-source behaviors, and how each guarantee is verified.

## The rule

### Admission

Initial source policies should support these common cases:

| Source | Admission behavior |
| --- | --- |
| Terminal key and text input | Bounded FIFO with reader backpressure |
| Surface observations | Latest value may replace an unaccepted observation |
| Ordinary effect completion | FIFO |
| Canceled latest-only effect | Suppress the known-canceled completion |
| Shutdown | Separate runtime control path |

By default the runtime delivers every key repeat separately. Each accepted key
event reaches `update`, so selection, cursor, viewport, and other logical state
stay responsive and deterministic. An application or source-specific policy may
aggregate input when its own semantics permit that optimization.

Messages within a delivery retain their order. Deliveries from all sources are
processed in the runtime-wide accepted order. A `Sync` delivery does not
overtake an earlier accepted `Async` delivery.

`Delivery` and its mode are runtime-internal. An application-defined source
(`Subscription::stream`, `run`, `run_blocking` in
[`tui-application.md`](tui-application.md)) is always `Async`; only the
runtime's own `surface` source produces `Sync` deliveries. What an application
states about a source is its `Admission`:

| `Admission` | Behavior | Under a `Sender` |
| --- | --- | --- |
| `Admission::bounded(n)` — the default | FIFO of at most `n` unaccepted items; the source waits when full | `send().await` / `blocking_send()` waits |
| `Admission::latest()` | one unaccepted slot; a newer item replaces an older one | `send` never waits |

A source declares it through the `*_with(key, admission, …)` constructor and
otherwise gets the bounded default. Effect completions and the runtime's own
sources carry the policies the source table at the top of this section gives
them and take no `Admission` from the application.

### Async delivery and draw scheduling

`Async` is the default delivery mode. Key input, text input, cursor movement,
selection changes, viewport movement, and ordinary effect completions normally
use it. A message is not `Sync` merely because it changes visible content.

The runtime processes accepted messages independently of physical drawing. It
may consume several messages and then draw only the latest resulting model when
the terminal is ready. This is draw coalescing, not message coalescing: no
accepted logical transition is skipped.

Draw scheduling is a runtime policy rather than an application command. The
core application API therefore has no `request_draw`, `plan_draw`, public
`Damage`, or public `Invalidation` type. The application expresses changes
through its model, effects, and view.

The initial runtime also does not require `View` equality as a prerequisite for
skipping a draw. Ratatui already compares cell buffers and emits only changed
cells. Caching or equality checks above that layer should be introduced only
after measurement identifies a material benefit and a correct ownership
boundary.

### Sync delivery as a render barrier

`Sync` is an exceptional delivery mode for changes to the logical rendering
environment. Examples include terminal dimensions, cell pixel dimensions, or a
graphics capability change that the application must reflect in its model
before the corresponding frame is built.

When the runtime accepts a `Sync` delivery, it:

1. cancels any pending draw that has not started;
2. processes all earlier accepted deliveries in order;
3. applies every message in the `Sync` batch through `update` without drawing
   intermediate models;
4. evaluates `view` once from the model after the complete batch; and
5. draws that view with the matching logical rendering-environment snapshot
   before processing later deliveries.

If another `Sync` delivery arrives during a draw, the runtime does not interrupt
the draw already in progress. It queues the new delivery at the next position in
the global order.

Shutdown inside the batch follows the rule that holds everywhere: the `update`
that returns `Effect::shutdown()` is the last `update`. The rest of the batch
is not applied, no view is evaluated, no frame is drawn, and the runtime
proceeds to the shutdown sequence in
[`tui-application.md`](tui-application.md). A failure to draw the barrier
frame is a terminal error and follows
[`tui-runtime-entry.md`](tui-runtime-entry.md): delivered to the application if
it subscribed to terminal errors, fatal otherwise.

This barrier guarantees agreement among the delivered environment facts, the
model after `update`, and the logical rendering-environment snapshot used to
build the frame. `Terminal` therefore does not let the backend resize the frame
on its own at draw time: a size change the backend reports while drawing is
not applied to that draw but enters admission as a `Sync` delivery, so the next
barrier draws with a snapshot and a frame that agree. It does not freeze the
operating system's physical terminal surface during the draw and does not
claim that terminal output is an atomic transaction.

### How surface information reaches the application

The runtime, terminal, Ratatui, and backend handle physical rendering
constraints. An application only needs surface information when that
information affects application semantics, such as layout choices or a viewport
measured in cells.

Such information enters through a subscription as a message. The application
stores the logical facts it needs in its model, and `view` reads them from that
model like any other state. `view` does not receive an implicit `ViewContext`
or physical `Surface` input.

The runtime cannot use a revision number alone to decide whether an application
has handled a surface change, because it does not understand arbitrary
application messages. The `Sync` delivery contract provides the required
coordination without inspecting those messages.

### Startup barrier

The first frame must not be built from placeholder surface information when an
initial surface observation is available. The runtime therefore establishes a
startup barrier before the first draw.

During startup, the runtime:

1. calls `init` and evaluates the initial subscriptions;
2. collects initial `Sync` deliveries from those subscriptions;
3. applies their messages in deterministic order, batching related surface
   facts where possible;
4. reevaluates model-dependent subscriptions when those messages change the
   subscription set;
5. repeats until no new initial `Sync` delivery is produced; and
6. evaluates `view` once and performs the first draw.

Initial `Async` deliveries are staged during this process. They enter normal
admission and receive global ordering only after the first draw, so they cannot
make the initial rendering environment inconsistent.

Startup reconciliation is bounded by a round limit the runtime owns; it is not
a configuration the application sets. When the limit is reached before the
initial `Sync` deliveries converge, the runtime does not draw: the entry point
returns `Error::StartupDidNotConverge`, and the terminal session is restored
as on any exit. With the runtime's `surface` source as the only producer of
`Sync` deliveries, and that source a singleton, reconciliation converges in
two rounds; the limit guards a runtime defect or a future `Sync` source, and
its value is the implementation's.

### A lightweight Ratatui application

For a lightweight Ratatui application, every accepted key event advances the
model. The runtime may process many such updates before the next draw, at which
point `view` reflects the latest model, and Ratatui emits the cell diff.
Filesystem work or syntax highlighting runs as effects, so key handling does
not wait for them.

## Verification

| Guarantee | Required evidence |
| --- | --- |
| Pure state transitions | Unit tests for `init`, `update`, and `view` without a terminal |
| Global ordering | Deterministic tests with interleaved input, subscription, and effect sources |
| Bounded admission | Saturation tests for bounded FIFO, backpressure, replacement, and cancellation |
| Async draw coalescing | Tests proving every update reaches `update` while the runtime draws only the latest model |
| Sync barrier | Tests proving no intermediate draw and no later delivery before the barrier draw |
| Startup | Tests for multiple initial `Sync` batches, staged `Async` input, and subscription convergence |

The runtime test harness should provide deterministic message sources, a
controllable clock, an in-memory backend, and observable effect scheduling.
Tests should assert externally meaningful ordering and presentation behavior,
not private task structure.

## Why `Delivery` and its mode stay internal

`Sync` exists for one thing: a fact about the rendering environment that the
next frame must already reflect. The runtime's `surface` source is the only
producer of such facts, so nothing an application declares needs the mode, and
exposing it would open the barrier to a source that merely wants its change
drawn promptly — which is what `Async` already does, minus the forced draw.
Should a rendering-environment source ever come from an application — a
graphics capability probe, say — a mode on `Admission` is an additive change.

## Why the startup limit is not configurable

The limit is reached only by a defect or by a `Sync` source that does not
exist yet; a knob for it would be a knob nobody turns. One error variant on the
entry point states the failure, and the value stays with the implementation.
