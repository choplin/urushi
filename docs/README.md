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

Placement is determined by what a passage lets the reader understand, not by
whether it mentions an interface or implementation mechanism. An API shape,
data flow, or implementation boundary belongs at the top level when it is
needed to understand the unit's larger design. The same material belongs under
`design/` when its purpose is to specify or defend one exact answer after that
larger design is already understood.

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

A developer-tooling or delivery subsystem may also keep its bounded operating
contract in its top-level document when that contract is part of understanding
the subsystem as a whole. Keep the repository README to the ordinary path and
put credentials, automation boundaries, failure behavior, and recovery detail
in the subsystem document. This exception does not make top-level documents
general-purpose runbooks or a place for unrelated procedures.

## `design/`: one design issue per file

Each file under [`design/`](design/) answers exactly one design issue in full:
one reader-facing question that could have been answered another way, such as
how a box resolves its size, how a prompt recovers a lost region, or how the
runtime orders deliveries. The file states the precise rule, its reasoning,
rejected alternatives, and past history where that history explains the
choice. It is where a reader goes once the mental model is in place and that
exact contract matters.

A design issue is not a tracker Issue or another unit of work. One tracker
Issue may resolve or revise several design issues, and a later tracker Issue
may revise the file that continues to own an existing design issue. Do not
collect decisions into one file merely because they were made during the same
tracker Issue. Conversely, do not split one design issue across several files
merely because its implementation touches several subsystems.

Placement follows from what the reader needs. If a passage is required to hold
the shape of a unit in mind, it belongs at the top level. If the reader can
hold the shape without it but needs it to implement, verify, or judge whether
the rule should change — precise procedures, edge cases, canonical forms,
worked examples, the defense against alternatives — it belongs here.

Split by the question a reader needs answered, not by tracker Issue, source
document, or implementation unit. A top-level document may hand detail to
several design files, and one design file may serve several top-level
documents.

These are not ADRs. When a decision changes, its file is rewritten in place to
describe the current decision; the file always reads as the present rule and
rationale, not as a dated record. The decision log preserves the superseded
decision and why Urushi revised it.

## `decision-log.md`: design decision history

[`decision-log.md`](decision-log.md) preserves design decisions whose historical
context would otherwise disappear when a design document is rewritten in
place. A row records that Urushi chose or revised a durable design rule among
meaningful alternatives, summarizes why, and links to the document that owns
the current rule and rationale.

The log does not record implementation or documentation activity. Implementing,
completing, testing, or refactoring an existing decision does not add a row;
neither does synchronizing documentation with code or summarizing a change or
release. If the design rule and its rationale did not change, the log does not
change.

## Single source of truth

Every statement has exactly one home. A document that needs a fact settled
elsewhere links to it instead of restating it. In particular: a top-level
document states a rule at the depth the mental model needs and links to
`design/` for its precise form and the justification of the choice behind it;
`design/` documents own that detail and do not restate the structural summary
a top-level document gives.
