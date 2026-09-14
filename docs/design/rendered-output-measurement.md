# Rendered Output and Display Width

How layout keeps one display-width definition without reparsing rendered
terminal strings. [`view-model.md`](../view-model.md) states the boundary under
“Plain text and rendered output” and “Display width”.

## The rule

The layout pass accepts renderer-neutral values. `PrintableLines` carries text
across rows, `PrintableText` represents one row, and `StyledText` aligns style
segments to grapheme boundaries. Plain-text width is the sum of the shared
per-grapheme width measure.

`resolve` records that geometry in `ResolvedView`. `render` consumes the
resolved rectangle and returns a final `String`; Urushi does not inspect that
string to recover layout information, and rendered output does not re-enter the
view tree. Horizontal and vertical composition therefore happen before
resolution, as `View::row` and `View::column`.

Passing escape sequences to a plain `Text` node is a contract violation. A
future raw-ANSI leaf may validate and normalize caller-declared rendered input,
but parsing it into semantic `TextStyle` and cells is a separate concern.

## Why width is the per-grapheme sum

A box fixes its width before its content is wrapped, because wrapping needs a
width to wrap to. “Text wrapped to `w` occupies at most `w`” therefore requires
the wrap decision and the width total to use the same measure. A
`ResolvedView` row is a sequence of grapheme tokens whose widths add to the
rectangle width, so the per-grapheme sum is the measure the model can preserve
through every stage.

This differs from applying `unicode-width` to a whole string for the small set
of ligatures that span grapheme boundaries, including LAM followed by ALEF.
Matching a particular terminal exactly would make width terminal-dependent;
that would be an explicit layout input rather than an ANSI-string heuristic.
