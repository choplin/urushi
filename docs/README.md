# Documentation Policy

Everything under `docs/` is written for developers changing Urushi. Documentation
for users of the library belongs in the top-level [`README`](../README.md) and,
later, in dedicated user documentation — not here.

This file defines how `docs/` is organized. It does not list or summarize
individual documents; adding or removing a document must not require editing
this file.

## Top level: what and how

Top-level documents explain what the codebase is and how it works.
[`architecture.md`](architecture.md) is the entry point; every other top-level
document covers one coherent unit — a component, a module, a subsystem — and is
reachable by following links from `architecture.md`.

A top-level document describes the code as it exists today. A document that
instead defines a target architecture for planned work must say so explicitly
and defer to `architecture.md` as the source of truth for the current state.

## `design/`: why

Each file under [`design/`](design/) records one design decision: the rule
itself and the reasoning behind it, including rejected alternatives and past
history where they explain the choice.

These are not ADRs. When a decision changes, its file is rewritten in place to
describe the current decision; the file always reads as the present rationale,
not as a dated record. The history of changes lives in the decision log.

## `decision-log.md`: history

[`decision-log.md`](decision-log.md) is a table of decisions over time. Each
row is one decision made or changed, linking to the document that records it.
Because design documents are rewritten in place, this log is the only record of
when decisions were made and what they replaced.

## Single source of truth

Every statement has exactly one home. A document that needs a fact settled
elsewhere links to it instead of restating it. In particular: top-level
documents state rules and link to `design/` for the reasoning; `design/`
documents own the why and do not re-specify contracts that a top-level document
defines.
