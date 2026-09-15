# Border Edges

Which edges of a border are drawn, how the glyph set and the four edge switches
relate, and what each enabled edge contributes to the box. [`style-model.md`](../style-model.md)
states the rule; the sizing rules that consume the frame's contribution are
[`box-sizing.md`](box-sizing.md). A presentation-owned line network is not a
collection of adjacent block borders; its occupancy and junctions belong to
the bound Canvas item drawing that network, as [`canvas.md`](canvas.md)
defines.

## The rule

Border edge visibility has named builders and getters:

```rust
let separator = BlockStyle::new()
    .border(Border::NORMAL)
    .border_top(false)
    .border_right(false)
    .border_bottom(true)
    .border_left(false);

assert!(separator.is_border_bottom_enabled());
```

`border(Border)` sets the glyph set and nothing else. Which sides are drawn is
four independent effective values, all `true` by default, so a style that has
never touched them draws all four sides once a glyph set is present; calling
`border` does not rewrite them, and `border_left(false).border(Border::ROUNDED)`
still has no left edge. Re-enable an edge explicitly with its builder, such as
`border_left(true)`. `without_border` removes only the glyph set; it does not
rewrite the four side values, which remain inactive until a border is added
again.

Each enabled edge contributes to the box as follows:

- An enabled top or bottom edge contributes one row.
- An enabled left or right edge contributes one column.
- A corner glyph represents the intersection of two enabled incident edges, so
  it is drawn only when both those edges are enabled. A box states no glyph for
  a line arriving from outside it: a corner is where two of its *own* edges
  meet. A presentation drawing a line network derives its junctions from that
  network instead of joining the borders of several cells.
- The horizontal glyph repeats across the padded content width, and every
  emitted row has the same outer width: the padded content width plus the
  enabled vertical-edge columns.

For example, a top edge without a left edge starts with the top horizontal
glyph rather than the top-left corner.

A border with all four sides disabled contributes no rows or columns and is
layout-equivalent to no border. Border foreground and background colors apply
uniformly to every enabled edge. The direct ANSI renderer and the Ratatui
widget use this same geometry, because both consume the same resolved
rectangle. A Ratatui `Rect` smaller than the block is an `Available` bound the
box resolves under, so the frame closes inside the area; only the degenerate
rules of [`box-sizing.md`](box-sizing.md) ever crop an edge.
