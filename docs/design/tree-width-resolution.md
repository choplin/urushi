# Tree Width Resolution

[`component-model.md`](../component-model.md) establishes that the canonical
Tree presentation binds an intrinsically sized Canvas item. This document
answers how that item reports width requirements and derives wrapping and
height after its parent selects a local width.

## Decision

Each visible typed value is formatted into plain text during composition. Each
visible child node then has a presentation-owned content origin, defined by its
depth and indentation geometry in
[`tree-lowering.md`](tree-lowering.md). The complete prefix before that origin
is protected from content wrapping.

For each explicit root or node line, width demand is its display width plus its
content origin. Width floor is the same origin plus the widest indivisible
content grapheme, or the origin alone when the content is empty. Root text has
origin zero. The Canvas sizing value reports the maximum demand and floor over
the bound hierarchy.

After the parent selects a local width, the snapshotted root text wraps within
that full width.
Node lines wrap only within `width - content_x`; the indentation and connector
prefix never participates in the wrapping flow. Explicit line boundaries stay
distinct, including a trailing empty line.

A pure depth-first row plan derived from the selected width supplies both the
intrinsic height and final drawing coordinates. Measurement and drawing must
invoke the same planning rule, so they cannot disagree about the number or
placement of wrapped rows.

## Reasoning

Demand preserves an unwrapped ordinary rendering when the parent can provide
it. Floor tells sibling width sharing the smallest width that still preserves
the Tree-owned structural prefix and one indivisible content unit. Treating
root content separately reflects that it has no incoming connector and avoids
charging it for indentation it does not use.

Deferring the row plan until width selection is necessary because composition
does not know a nested component's eventual share. Reusing that plan for height
and drawing makes height a consequence of exactly the wrapping that will be
shown, including CJK and emoji grapheme widths and explicit empty lines.

## Rejected alternatives

- **Wrap during composition.** Composition can know the root area at most, not
  the width assigned after parent layout and sibling sharing.
- **Let content wrap through the connector prefix.** This could overwrite or
  displace presentation-owned topology and give continuation rows a different
  content origin.
- **Use content bytes or scalar values for the floor.** Terminal layout must
  preserve an indivisible displayed grapheme, whose cell width is not implied
  by either representation.
- **Measure and draw with separate traversals.** Duplicate row logic can drift
  on explicit newlines, trailing empty lines, or width-sensitive wrapping.
