# Grid

How a Grid aligns child Views in shared columns: the claim a column makes, the
`Length` a column may carry, the padding every cell takes, and why Table
presentation and line drawing are outside this primitive.
[View Model](../view-model.md) summarizes the node; sibling distribution is
[Area Sharing](area-sharing.md), and the clamp each cell applies is
[Box Sizing](box-sizing.md).

## The rule

A Grid is a rectangle of child Views and one geometry style:

```rust
pub enum View {
    // ...
    Grid(GridStyle, Vec<Vec<View>>),
}

pub struct GridStyle {
    columns: Vec<Option<Length>>,
    cell_padding: Sides,
}
```

Every row holds the same number of cells. Grid does not invent a missing cell:
an absent child has no style, padding, alignment, or background, so the caller
supplies an empty View when that is the intended cell.

Grid has no border, separator, header, or component preset. A surrounding
`Block` supplies outer box geometry. A presentation that needs an internal
line network owns that drawing in an intrinsically sized
[Canvas](canvas.md).

### Column widths

Every cell in a column resolves under one width, and that width is decided
once. For each column Grid forms a claim:

- its **kind** comes from the column's `Length`, and is auto when none is
  stated;
- its **demand** is the greatest intrinsic width among the cells in that
  column; and
- its **floor** is the greatest width floor among them.

Those claims divide the available width by the rule in
[Area Sharing](area-sharing.md): stated sizes take their requested size, auto
columns their intrinsic size, and `Fill` columns divide the remainder by
weight. When claims exceed the area, they shrink in that order, each down to its
floor.

A `Length` on a column has the same meaning as one on a `BlockStyle`:
`Cells` is an absolute request, `Fill` is a share of the remainder, and an
absent value is intrinsic. The cells supply demand and floor because they are
the content that must fit.

Nothing is renegotiated. A cell that resolves narrower than its column leaves
the remainder to its own alignment fill, just as a child below a `Row` share
does.

### Row heights

A row's height is the greatest resolved height among its cells, as in a `Row`.
A shorter cell is filled to that height and placed by its own vertical
alignment.

### Cell padding

`cell_padding` applies to every cell that states no padding of its own. A
cell's stated padding replaces it rather than adding to it, following the
style-value model's effective-value rule.

Uniform padding is not required for alignment. A column's demand includes the
padding each cell actually takes, and one shared width is still selected for the
column.

## No spans

Grid has neither column span nor row span. Its public use is a rectangular
alignment of child Views, and no established direct Grid case requires one
child to cover several tracks. Adding span would introduce interval demand and
floor constraints and a different cell-addressing model without serving that
contract.

The former proposal used Tree indentation as the reason for column span. Tree
is semantic data with its own presentation, not evidence for widening Grid.
A future direct Grid use may reopen span only together with the precise
interval-sizing behavior it needs. A future Table colspan belongs first to the
Table data and presentation; it does not imply a Grid span.

## Grid and Table are independent

Grid is a layout container: it recursively resolves arbitrary child Views under
shared column widths. Table is semantic data interpreted by
`TablePresentation`. The canonical Table presentation binds its data, styles,
and area-dependent drawing behavior into a Canvas item, supplies that Canvas's
intrinsic sizing, and draws its cells and rules directly in the final viewport.

Similar width arithmetic does not create a lowering relation. The two may use
the same private sizing functions where their mechanics coincide, but neither
public model is the other's intermediate representation. In particular, Grid
never receives Table headers, `BOOKTABS`, `MARKDOWN`, row-rule switches, or
junction glyphs.

## Why Grid does not draw lines

Lines are part of a particular presentation, not a necessary consequence of
aligning children in shared tracks. Keeping them on Grid would require a public
vocabulary for edge occupancy, selective gaps, glyph repertoires, and junctions
even when the direct caller wants only alignment.

Table supplies the concrete use that needs those decisions, and Canvas lets the
Table presentation measure before allocation and draw after receiving the
final local size. The owner of the Table line network is therefore the bound
Table item. It decides which rules exist, reserves their rows and columns during
its own measurement, selects their glyph repertoires, and records their geometry
as one `LineNetwork` command per connected network when it draws. Line-network
rasterization derives each corner, tee, cross, or straight glyph from that
network's incident directions. Cells do not own junctions.

The exact absence, blank occupancy, repertoire choice, and command ordering
belong to the Table presentation rather than Grid. Direction union and glyph
selection belong to `LineNetwork` rasterization; applying its resulting cells
belongs to ordinary Canvas composition. See [Canvas](canvas.md) and [Component
Presentation Boundary](component-presentation.md).

## Effect on the superseded proposal

The canceled earlier proposal correctly identified several constraints:

- Table roles must not enter Grid vocabulary;
- line absence and an occupied blank line are distinct;
- one presentation must own the complete `LineNetwork` it records;
- Table and Block keep their public border policy; and
- a presentation must not receive the root area during composition.

This design retains those constraints but changes their owner. The bound Table
item and its sizing value, not Grid, own Table rules, occupancy, repertoire
choice, and line-network command ordering. `LineNetwork` rasterization owns the
generic mechanics that derive junctions from incident directions. Canvas composition
receives no area; ordinary resolution selects the Canvas size before the item
draws.

The rest of that proposal is not adopted:

- Grid span is omitted because its Tree motivation disappeared and no direct
  Grid use requires it.
- `LineSet`, `GridLineStyle`, per-line weights, and gap overrides are omitted
  because a geometry-only Grid has no line vocabulary.
- Table does not translate its presets or header rule into Grid.
- Block borders remain ordinary four-edge box geometry and do not acquire a
  grid-junction model.

The current decision is complete without that draft; the list above records
which of its conclusions survived and which did not.

## Rejected designs

- **Table lowering to Grid.** It forces Table's area-dependent cells and rule
  network into another public primitive's vocabulary and makes Grid the de
  facto Table presentation representation.
- **Lines retained on Grid for possible direct use.** No established direct
  Grid case requires them, while their presence adds geometry, glyph, topology,
  and junction contracts.
- **A header or selective Table-rule switch on Grid.** It exposes Table meaning
  on an unrelated primitive.
- **Cell-owned rules and junction glyphs.** Adjacent cells can disagree about a
  shared edge. The Table item instead records each complete connected rule
  geometry as one `LineNetwork`, which derives its glyphs before Canvas
  composition.
- **Column or row span now.** It introduces interval constraints and partial
  tracks without a current Grid requirement.
- **Presentation composition receiving `Available`.** A nested component does
  not know its share until its siblings are resolved. Canvas intrinsic sizing
  instead participates in the ordinary local layout pass.
