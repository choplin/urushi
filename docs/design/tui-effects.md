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
- long-lived work represented as a subscription; and
- keyed latest-only work for replaceable preparation.

Deciding whether a completion is still relevant is split between the runtime and
the application. For latest-only work, the runtime may suppress a completion
only when the runtime itself knows that the corresponding execution was
canceled or replaced. For ordinary effects, the application decides in `update`
whether a completion still applies to the current model.

A generation identifier in the application's message and model is one possible
way to make that decision; it is not a runtime-wide damage model.

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

## Open representation choices

- the executor boundary behind `Effect::perform` and `Effect::future`, and
  the entry point that runs an application, which must keep the model on the
  thread that runs `update` and `view` — the runtime's contracts describe
  ordering, cancellation, and wake-up behavior rather than exposing a
  particular executor's task handles throughout application types.
