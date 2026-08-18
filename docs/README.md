# Documentation Policy

Everything under `docs/` is written for developers changing Urushi. Documentation
for users of the library belongs in the top-level [`README`](../README.md) and,
later, in dedicated user documentation — not here.

This file defines how `docs/` is organized. It does not list or summarize
individual documents; adding or removing a document must not require editing
this file.

## Top level: the mental model

Top-level documents exist to give a reader the mental model of Urushi: the
design direction and the structure — what the parts are, how they relate, and
the rules that hold between them. [`architecture.md`](architecture.md) is the
entry point; every other top-level document covers one coherent unit — a
component, a module, a subsystem — and is reachable by following links from
`architecture.md`.

A top-level document is deliberately not exhaustive. It states the shape of a
unit and the principles that shape rests on, at the depth needed to hold the
model in mind — not every rule, edge case, procedure, or state transition the
implementation must satisfy. Detail beyond that depth defeats the purpose: a
reader building a mental model cannot see the shape through it. When a
top-level document grows a section whose precision serves the implementer
rather than the model, that section belongs under `design/`, and the top-level
document keeps a summary and a link.

A top-level document describes the architecture Urushi is built toward, not
the state of the code on a given day. Where the implementation has not reached
that architecture, the document is not annotated with the gap; closing the gap
is work on the code, and tracking it belongs to the issue tracker, not to
`docs/`.

A top-level document carries the background and goals a reader needs to
understand the shape it defines — the problem the unit solves and what it must
provide. A contract stated without its purpose is not comprehensible. What it
does not carry is the defense of that shape against the alternatives it was
chosen over.

## `design/`: one topic in depth

Each file under [`design/`](design/) treats one design topic in full: the rule
as it is precisely stated, the reasoning behind it, the rejected alternatives,
and past history where it explains the choice. A topic is a question that could
have been answered another way — how a box resolves its size, how a prompt
recovers a lost region, how the runtime orders deliveries — and its file is
where a reader goes once the mental model is in place and the exact contract
matters.

Placement follows from what the reader needs. If a passage is required to hold
the shape of a unit in mind, it belongs at the top level. If the reader can
hold the shape without it but needs it to implement, verify, or judge whether
the rule should change — precise procedures, edge cases, canonical forms,
worked examples, the defense against alternatives — it belongs here.

Split by topic, not by source document. A top-level document may hand detail to
several design files, and one design file may serve several top-level
documents.

These are not ADRs. When a decision changes, its file is rewritten in place to
describe the current decision; the file always reads as the present rule and
rationale, not as a dated record. The history of changes lives in the decision
log.

## `decision-log.md`: history

[`decision-log.md`](decision-log.md) is a table of decisions over time. Each
row is one decision made or changed, linking to the document that records it.
Because design documents are rewritten in place, this log is the only record of
when decisions were made and what they replaced.

## Single source of truth

Every statement has exactly one home. A document that needs a fact settled
elsewhere links to it instead of restating it. In particular: a top-level
document states a rule at the depth the mental model needs and links to
`design/` for its precise form and the justification of the choice behind it;
`design/` documents own that detail and do not restate the structural summary
a top-level document gives.
