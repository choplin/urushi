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

Messages within a `Sync` delivery retain their order. Deliveries from all
sources are processed in the runtime-wide accepted order. A `Sync` delivery
does not overtake an earlier accepted `Async` delivery.

`Delivery` and its variants are runtime-internal. `Async` contains exactly one
message; only `Sync` contains a non-empty batch. An application-defined source
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

The runtime surface source does not pass through the generic application-source
inbox. When an observation occurs, the source applies the current subscription
mapper and publishes the resulting application message into one replaceable,
unaccepted slot. Accepting that slot places a `Sync` delivery directly into the
runtime-wide order. This keeps both mapper timing and surface replacement policy
with the only producer that needs the render barrier, while
`Admission::latest()` remains available for application-defined snapshot
sources whose accepted messages are ordinary `Async` deliveries.

### Implementation synchronization

The delivery implementation uses these synchronization paths:

| Path | State protected by its mutex | Waiting and notification | Acceptance |
| --- | --- | --- | --- |
| Application source inbox | Unaccepted FIFO, endpoint closure, receiver Waker, and blocked async-sender Wakers | An async sender stores a Waker when a bounded inbox is full; a blocking sender waits on a condition variable. Accepting a value wakes both forms after releasing locks. | The inbox lock remains held while the oldest message is appended to the global queue as `Async`. |
| Surface slot | One replaceable mapped message, endpoint closure, and receiver Waker | Publication never waits. It replaces the slot and wakes the runtime after releasing the slot lock. | The slot lock remains held while the message is appended to the global queue as `Sync`. |
| Delivery queue | The runtime-wide FIFO and its async receiver Waker | The async runtime waits with a Waker; a blocking runtime waits on a condition variable. Insertion takes the Waker, releases the queue lock, then signals both forms. | Insertion under the queue mutex assigns the global delivery position. |
| Latest-only effect | Whether one execution is pending, accepted, or canceled | Neither completion nor cancellation waits for capacity. Runtime notification happens after the freshness lock is released. | Cancellation and queue insertion are ordered by the freshness mutex, so an accepted completion cannot be removed retroactively. |

When a transition needs both a producer-local mutex and the delivery-queue
mutex, it always acquires the producer lock first. The delivery queue never
acquires a producer lock. No Waker is invoked while either mutex is held,
because waking may synchronously execute scheduler code.

### Async delivery and draw scheduling

`Async` is the default delivery variant. Key input, text input, cursor movement,
selection changes, viewport movement, and ordinary effect completions normally
use it, and each accepted message occupies its own delivery. A message is not
`Sync` merely because it changes visible content.

The runtime processes accepted messages independently of physical drawing. It
may consume several messages and then draw only the latest resulting model when
the terminal is ready. This is draw coalescing, not message coalescing: no
accepted logical transition is skipped.

The scheduler has three phases: `Idle`, `Drawing`, and `CoolingDown`. It also
has one coalesced dirty bit. Completing `init` or an `update` invalidates the
scheduler directly on the application thread; invalidation is not a delivery
or a runtime-event message. The transitions are:

1. invalidating `Idle` makes a draw ready immediately;
2. starting that draw consumes dirty and enters `Drawing`;
3. invalidation during `Drawing` only sets dirty;
4. completion always enters `CoolingDown` until the completion time plus the
   configured minimum interval;
5. invalidation during `CoolingDown` only sets dirty; and
6. the scheduler starts and receives the cooldown timer itself; when it fires,
   dirty starts one draw from the latest model, while clean returns to `Idle`.

The default interval is approximately one 30 fps frame period. It limits
physical presentation frequency while the application is busy; it does not
delay an update that reaches an idle scheduler.

Draw scheduling is a runtime policy rather than an application command. The
core application API therefore has no `request_draw`, `plan_draw`, public
`Damage`, or public `Invalidation` type. The application expresses changes
through its model, effects, and view.

The initial runtime also does not require `View` equality as a prerequisite for
skipping a draw. `Terminal` already compares cell buffers and emits only
changed cells. Caching or equality checks above that layer should be introduced
only after measurement identifies a material benefit and a correct ownership
boundary.

### Sync delivery as a render barrier

`Sync` is an exceptional delivery variant for changes to the logical rendering
environment. Examples include terminal dimensions, cell pixel dimensions, or a
graphics capability change that the application must reflect in its model
before the corresponding frame is built.

When the runtime accepts a `Sync` delivery, it:

1. records a fence at the same queue linearization point as acceptance;
2. processes all earlier accepted deliveries in order while admitting no new
   draw;
3. applies every message in the `Sync` batch through `update` without drawing
   intermediate models;
4. releases that delivery's fence after the complete batch; and
5. evaluates the scheduler before processing a later delivery.

If another `Sync` delivery arrives during a draw, the runtime does not interrupt
the admitted draw. Acceptance fences the next draw and gives the new delivery
its position in the global order.

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
initial surface observation is available. Startup uses the ordinary Sync fence
rather than a second scheduling mode.

During startup, the runtime:

1. calls `init` synchronously to create the initial model;
2. starts the initial subscriptions, whose runtime-owned surface source accepts
   its known initial observation synchronously;
3. invalidates the scheduler for the initial model; and
4. enters the normal loop.

The accepted initial `Sync` keeps the first scheduling decision fenced. The
runtime applies it through `update`, reconciles the resulting subscriptions,
releases the fence, and evaluates the scheduler before accepting another
delivery. Initial `Async` deliveries need no staging; they retain their normal
accepted positions, and any one processed before the initial `Sync` still
cannot cause a draw while that fence is pending.

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
| Async draw coalescing | Tests proving every update reaches `update`, idle invalidation draws immediately, and busy invalidations produce one latest-model draw after cooldown |
| Sync barrier | Tests proving acceptance fences new draws, batches have no intermediate draw, and an in-flight draw is not interrupted |
| Startup | Tests proving `init` and the known initial surface `Sync` are reflected in the first view through the ordinary fence |

The runtime test harness should provide deterministic message sources, a
controllable clock, a deterministic presentation boundary backed where needed
by an in-memory `Terminal`, and observable effect scheduling. Tests should
assert externally meaningful ordering and presentation behavior, not private
task structure.

## Why `Delivery` and its variants stay internal

`Sync` exists for one thing: a fact about the rendering environment that the
next frame must already reflect. The runtime's `surface` source is the only
producer of such facts, so nothing an application declares needs the mode, and
exposing it would open the barrier to a source that merely wants its change
drawn promptly — which is what `Async` already does, minus the forced draw.
Should a rendering-environment source ever come from an application — a
graphics capability probe, say — a barrier choice on `Admission` is an additive
change.

## Why startup has no convergence mode

The set of runtime sources that can produce `Sync` is fixed, and startup has one
known initial surface observation. Treating arbitrary application traffic as a
startup fixed point would add staging, a round budget, and an error state to a
case the source model does not create. Accepting the known observation before
the first scheduling decision gives startup the same fence semantics as every
later surface change.
