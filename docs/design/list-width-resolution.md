# List Width Resolution

How a bound List reports intrinsic size without an available area and turns the
width later selected by its parent into visual rows. The horizontal prefix used
below is defined by [List Marker Layout](list-marker-layout.md).

## Width requirements include the complete prefix

For an item, `prefix` is its `content_x`: the nesting displacement plus its
sibling group's complete marker track. Content demand is the widest explicit
semantic line. Content floor is the widest indivisible grapheme in those lines,
or zero for empty content. The List reports:

```text
demand = max(prefix + content demand for every visible item)
floor  = max(prefix + content floor  for every visible item)
```

All additions use saturating arithmetic. An empty or wholly hidden List reports
zero width and zero height. Display width and indivisible graphemes follow the
shared [`View::Text`](../view-model.md) rules.

Including the prefix in both values makes its cost visible when Row, Column,
Grid, or Block distributes space. The floor protects the full indentation and
marker track plus one content grapheme whenever the parent can honor it. Below
that floor, the ordinary View cropping rule applies; List does not drop the
marker, split a wide grapheme, or invent a second narrow-width policy.

## The selected width determines rows

After the parent selects the Canvas width, one pure row-layout routine derives
both height and drawing. For each item, the content budget is the selected
width minus its complete prefix, using saturating subtraction. Each explicit
semantic line is a hard boundary and wraps within that budget using the same
grapheme-atomic rule as `View::Text`.

An item contributes at least one visual row, even when its content is empty.
Explicit-line splitting preserves the empty segment after a trailing `\n`, so
that segment becomes a continuation row. The first row receives the marker;
every row after it begins at the item's `content_x`, whether caused by an
explicit line break or width-driven wrapping. Children follow all visual rows
of their parent.

`height_requirements(width)` reports the resulting row count as demand and zero
as floor. A finite parent may therefore clip List rows according to the generic
height rule; the presentation does not reserve a particular item or
continuation row.

Measurement and drawing call the same row-layout routine. Resolving the same
composed `View` at another width reruns only this pure width-dependent work. It
does not recompose the List, reevaluate enumerators, or mutate semantic data.

For example, a two-cell marker track lets the same content resolve at different
widths without a padded prefix string:

```text
width 12          width 8
• alpha beta      • alpha
                    beta
```

With a two-cell nesting step, a two-cell marker track, and a two-cell CJK
grapheme, a depth-one item has a six-cell floor. At that width the content wraps
without splitting the grapheme:

```text
• p
  • 日
    本
    語
```

## Why height is not decided during composition

The component does not know its local width until siblings share their parent.
Pre-wrapping during composition would use the root width or another guessed
width and make the composed value invalid in a different container. Reporting
width first and height at the selected width preserves area-independent
composition while still giving the parent exact intrinsic requirements.
