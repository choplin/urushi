# TUI Effects and Expensive Preparation

How an effect runs, which execution policies the runtime supports, and how the
runtime and the application divide the decision of whether a completion is
still relevant. [`tui-architecture.md`](../tui-architecture.md) states the
model under "Effects"; this file holds the policies, the freshness rule, and
the representative flow.

## The rule

An `Effect<Message>` describes work to be interpreted by the runtime. It cannot
update the model directly. Its observable result returns as a message and
passes through normal delivery.

Effects include filesystem and Git access, syntax highlighting, document
layout, image rasterization, and other work that would make `update` or `view`
too expensive. This separation allows every key input to advance the logical
model without starting physical rendering or heavy preparation for every
intermediate state.

The runtime must support at least these execution policies:

- ordinary one-shot work whose completion is delivered in source order;
- one-shot work delayed by a stated duration, timed by the runtime's `Clock`;
- long-lived work represented as a subscription; and
- keyed latest-only work for replaceable preparation.

Deciding whether a completion is still relevant is split between the runtime and
the application. For latest-only work, the runtime may suppress a completion
only when the runtime itself knows that the corresponding execution was
canceled or replaced. For ordinary effects, the application decides in `update`
whether a completion still applies to the current model.

A generation identifier in the application's message and model is one possible
way to make that decision; it is not a runtime-wide damage model.

What the runtime knows is replacement, and only replacement. A `*_latest`
effect started under a `Key` that another `*_latest` effect is still running
under replaces it: a future is dropped, which ends it; a closure that has not
started is not started, and one already running runs to its end and has its
completion discarded; an unfired delay is dropped, and its wait begins again
from the replacement's duration, which is what makes `after_latest` a
debounce. Nothing else suppresses a completion. There is no
`Effect::cancel(key)`: an application that no longer wants a result either
replaces the work or lets the completion reach `update` and reads it against
the model, which is the check it needs anyway for a completion the runtime
had no reason to suppress.

Effects run behind an `Executor` boundary the runtime owns: blocking closures
are handed to it to run off the update thread, futures to be driven, and each
returns a handle the runtime drops to replace the work. The runtime's default
executor is Tokio, and the harness described in
[`tui-delivery-ordering.md`](tui-delivery-ordering.md) supplies a
deterministic one; how an application supplies its own is defined in
[`tui-runtime-entry.md`](tui-runtime-entry.md).

In a document viewer, for example, changing the document or theme can start a
new keyed layout or rasterization effect. Moving the viewport can reuse the
prepared asset already stored in the model and render it at a new position
without starting that preparation again.

### Prepared assets and viewport movement

For an application with prepared assets, document changes start replaceable
preparation effects whose completed assets enter the model through messages.
Viewport movement remains a lightweight logical update. The next draw reuses
the existing prepared asset with the new viewport instead of treating every
movement as damage that requires preparation.

## Verification

| Guarantee | Required evidence |
| --- | --- |
| Effect freshness | Tests for ordinary stale results and runtime-known latest-only cancellation |

Effect scheduling must be observable from the runtime test harness described
in [`tui-delivery-ordering.md`](tui-delivery-ordering.md).

## Why there is no explicit cancel

Suppression that the runtime performs on its own knowledge is exactly one
event, replacement, and the runtime can be right about it every time. Every
other reason a completion is stale — the model moved on, the file list changed
under the cursor, the pane closed — is a fact of the application's semantics,
which the application already has to check when a completion arrives, since no
replacement happened. A `cancel(key)` would spare that check in one of those
cases and change nothing in the others; and for a closure already running it
would not even stop the work. One habit, checking a completion against the
model, covers all of it. Should a use appear, `cancel` is an additive
constructor.
