# Grid

How a grid aligns column widths across rows and draws the lines between its
cells: the claim a column makes, the `Length` a column may carry, the padding
every cell takes, and why the separators belong to the container rather than to
the cells. [`view-model.md`](../view-model.md) summarizes this under "Sizing at
a glance"; how a `Row` or `Column` divides an area is
[`area-sharing.md`](area-sharing.md), the clamp each cell applies to the width
it is handed is [`box-sizing.md`](box-sizing.md), and which edges a border
draws is [`border-edges.md`](border-edges.md).

## The rule

A grid is a rectangle of cells and one style:

```rust
pub enum View {
    // ...
    Grid(GridStyle, Vec<Vec<View>>),
}

pub struct GridStyle {
    border, border_top, border_right, border_bottom, border_left,
    border_column, border_row,
    border_header: Option<bool>,
    border_foreground, border_background,
    columns: Vec<Option<Length>>,
    cell_padding: Sides,
}
```

Every row holds the same number of cells. A grid does not pad a short row: an
absent cell has no style to fill it with, so an empty cell is supplied by
whatever composes the grid.

### Column widths

Every cell in a column resolves under one width, and that width is decided
once. For each column the grid forms a claim:

- its **kind** from the column's `Length`, auto where none is stated;
- its **demand** from the greatest intrinsic width among the cells in that
  column;
- its **floor** from the greatest floor among them.

Those claims divide the grid's width by the rule
[`area-sharing.md`](area-sharing.md) already defines: stated sizes take their
size, auto columns their intrinsic size, `Fill` columns divide the remainder by
weight, and when the claims exceed the area they shrink in that same order,
each floored at its own floor.

A `Length` on a column is the vocabulary a `BlockStyle` states about a box, and
it carries the same meaning: `Cells` is an absolute request, `Fill` a share of
the remainder, auto the intrinsic size. It marks intent rather than fixing a
formula — a grid derives the demand and the floor from the cells beneath the
column, where a `Row` reads them from the child itself.

Nothing is renegotiated. A cell that resolves narrower than its column leaves
the remainder unused, and the alignment fill of its own box places it, exactly
as a `Row` child below its share is placed.

### Row heights

A row's height is the greatest height among its cells, as in a `Row`. A shorter
cell is padded with blank rows and placed by its own vertical alignment.

### Cell padding

`cell_padding` applies to every cell that states no padding of its own. A cell
that states one replaces it rather than adding to it, which is the rule the
style model applies wherever a specific value meets a general one.

A grid does not need uniform padding to keep its columns aligned: a column is
as wide as its widest cell, whatever padding that cell carries. `cell_padding`
therefore states the ordinary case — one spacing for the whole grid — rather
than an invariant the alignment rests on.

### Separators

The grid draws every line: its four outer edges, the line between two columns,
and the line between two rows, from one glyph set and one border color. The
outer edges follow [`border-edges.md`](border-edges.md) — an enabled edge
contributes one row or one column, a disabled one contributes nothing.
`border_column` and `border_row` each contribute one column or row between
neighbours and never at the ends.

`border_header` is the one exception, and it is about a row rather than about
the lines. A grid that states it declares that its first row is a header, and
the gap below that row leaves `border_row`'s hands in both directions: a rule
below the header where no other gap carries one, or no rule below it where
every other gap does. A grid that states nothing has no header, and its first
gap is an ordinary gap.

The glyph at an intersection is derived, not stated. Each intersection has four
incident directions; which of them carry a line follows from the edge
switches and from where the intersection sits, and the glyph — corner, tee,
or cross — follows from those four facts. `Border` already names each one.

Lines are drawn after every width is decided, so a grid narrowed by its area
shrinks its columns and its lines together. They cannot disagree, because
neither exists until the widths do.

## Why the container draws the separators

A cell knows its own four edges. The glyph at an interior intersection is a
statement about a neighbourhood — which lines arrive from which directions —
and the container is the only node that has one.

Letting the cells carry the edges is arithmetically possible: disable one of
every adjacent pair so each line is drawn once, and every intersection still
lands in exactly one cell's corner, with no doubling and no gap. What it cannot
do is choose the glyph. A corner is where two of a box's *own* edges meet, so a
box that draws one has no vocabulary for a third or fourth line arriving from
outside it. Every interior intersection would come out as a corner where a tee
or a cross belongs.

Supplying the glyph per cell closes that, and is rejected below. Deriving it in
the container closes it without adding a property, and removes the possibility
of a disagreement: there is no second party stating a glyph for the same cell.

## Why a header is a row rather than a line

A table's ordinary shape is a rule below the header and nothing between the
rows — the two are independent, so one switch cannot carry both. Making the
extra switch a second kind of line would misplace it: the glyph, the colour,
and the intersections are the same as every other horizontal rule, and the only
thing that differs is *which gap* it falls in.

What is actually new is the row. A header is a row the grid knows is one, and
naming it that way is what lets the switch be a tri-state: unstated means there
is no header, and the first gap is an ordinary gap. That distinction is what a
plain `bool` cannot express, and it is load-bearing — without it a grid whose
rows all carry rules could not withhold the one below its header.

The grid still learns nothing about what a header contains. It does not style
the row, repeat it, or exclude it from the column claims; the row is a row.

## Why a column carries a `Length`

Without one, every column is auto, and a grid can only ever be as wide as its
content. "This column is exactly this wide" and "this column takes what is
left" — the two ordinary intentions a layout has about a column — become
inexpressible.

The shrink order is the sharper case. A column of fixed markers and a column of
prose are both auto, so a narrow area takes cells from both in proportion, when
the entire point of the fixed column is that it gives none up. Stating `Cells`
on it puts it last in the shrink order, which is the behaviour
[`area-sharing.md`](area-sharing.md) already defines for a box that states its
size. The vocabulary that expresses this exists; the grid only has to accept it.

That a stated column still shrinks when the area leaves no alternative is
deliberate, and is the same rule every other stated size follows. A grid that
refused would resolve past its area and meet the degenerate crop, which is the
post-hoc cut the sizing model exists to avoid.

## Why a grid is a rectangle

A ragged grid would need the grid to invent the missing cells, and an invented
cell has no style: the background it fills, the padding it takes, and the
alignment it applies would all be the grid guessing on behalf of a caller who
never stated them. Requiring the rectangle moves that decision to the only
place that can answer it, at the cost of one loop in whatever composes the
grid.

## What belongs above a grid

A grid aligns and draws. What a cell contains, which rows are present, how a
value becomes a cell, and which style a cell takes belong to whatever composes
it — see [`component-model.md`](../component-model.md).

Keeping them apart is what lets a composed view carry no size at all. A
component that measures its own content and writes the result into the tree —
as an absolute length, as pre-wrapped text, or as a string of repeated
glyphs — has decided a layout before the area is known, and the pass has no
way to refuse it, because the contents of a text leaf are opaque to it. The
grid gives that intent a form the pass can read, so a component states which
cells share a column and the pass decides how wide it is.

## Rejected designs

- **A component that receives the `Available` area and sizes itself.** A
  component composed inside a `Row` cannot know its share while composing: that
  share depends on what its siblings resolve to, which the pass settles later.
  The rule would hold only for a component used at the root, and one that is
  correct only at the root does not compose.

- **A subtree exempt from re-fitting.** Its baked sizes would survive the area
  and be cut afterwards — the post-hoc crop [`box-sizing.md`](box-sizing.md)
  rejects, reintroduced as an opt-in. A bound that arrives after a box is
  assembled can only cut its frame open, which is the artifact making the area
  an input removed.

- **Per-cell border edges, with corner glyphs stated on `BlockStyle`.** It puts
  a property on every box whose correct use no signature states, and it lets
  two adjacent boxes state different glyphs for the line between them with
  nothing to resolve the disagreement. Derivation in the container answers what
  this would have asked an author to supply.

- **Grid properties on `BlockStyle`.** `border_column` and `border_row` mean
  nothing to a box with one child.
  [`style-value-model.md`](style-value-model.md) keeps the property vocabulary
  closed and every property meaningful in it.

- **A `cell_padding` that adds to a cell's own padding.** Reading a cell's
  style would no longer tell you the padding that cell has, and layering one
  value over another is the patch semantics the style model excludes.

- **Ragged rows padded by the grid.** Argued under "Why a grid is a rectangle".

- **A separate line kind for the header rule.** It would state a second glyph
  set, a second colour, and a second set of intersections for a line that is
  drawn exactly like the others. The header rule differs in where it falls, not
  in what it is.

- **A `bool` header switch.** It cannot distinguish "there is no header" from
  "there is a header and no rule below it", and the two differ whenever
  `border_row` is enabled.

- **A per-gap list of switches, symmetric with `columns`.** It expresses the
  header case and much that no caller wants, at the cost of making the ordinary
  grid state a vector whose length tracks its row count.
