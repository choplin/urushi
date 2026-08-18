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

The concrete implementation must bound startup reconciliation and detect a
cycle or budget exhaustion. The public failure behavior when startup
reconciliation cycles or exhausts its budget remains an API design decision.

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

## Open representation choices

- the public form of source admission policies and of application-defined
  subscription sources;
- error and shutdown behavior during `Sync` processing; and
- startup reconciliation limits and error reporting.
