# Styles as Effective Values

This document records why Urushi's style contract, defined in
[`style-model.md`](../style-model.md), shapes a style as an immutable
collection of effective presentation values with a closed property vocabulary —
rather than a patch, an instruction list, or a completed ANSI state transition.

## Why effective values, not instructions

A style whose entries are effective values means one thing wherever it is read:
a renderer consumes the values present and emits whatever its backend needs to
realize them. A style whose entries are instructions — "turn bold off" — only
means something relative to a prior state, so every reader must agree on what
that state is. Removing a modifier from an immutable value therefore removes
the value; it does not preserve an ANSI off-code instruction. A renderer that
maintains prior terminal state diffs previous and next effective styles itself,
and `TextStyle::paint` and `BlockStyle::render` surround emitted styling with a
final ANSI reset, so no stored removal instruction is needed anywhere.

## Why a closed vocabulary and one generic `remove`

Like Lip Gloss, Urushi treats a style as an immutable value containing a set of
rules. Lip Gloss tracks property presence separately and exposes many
property-specific `Unset*` methods. Urushi instead exposes a closed enum pair
per type and one generic `remove`, while retaining named builders for common
construction. The enums make the complete property vocabulary discoverable and
give generic code an exhaustive match, and one `remove` cannot drift from a
parallel family of `Unset*` methods. The named builders are thin wrappers over
`add` for the same reason: two entry points defining two behaviors is the
failure mode being avoided.

## Why no patch operation

This deliberately differs from noctui's incremental add/sub modifier sets and
`patch` operation. Urushi does not adopt that representation because reusable
changes can be ordinary `fn(TextStyle) -> TextStyle` transforms, and no current
Urushi boundary requires a separately inspectable patch value; a patch data
type would be introduced only for a concrete need to serialize or inspect such
changes. The absence of a patch representation is also what keeps
parent-to-child style inheritance out of the view model, as
[`view-block-model.md`](view-block-model.md) records.
