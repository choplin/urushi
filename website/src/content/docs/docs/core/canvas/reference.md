---
title: Canvas reference
description: Complete reference for Canvas types, commands, defaults, constraints, and errors.
---

This page lists the public Canvas API exported by `urushi`.

It is the task-oriented index to that API. Use the generated Rust API
reference for every exact signature, generic bound, trait implementation, and
source link:

- [Complete `urushi` API](/api/urushi/index.html)
- [`Canvas`](/api/urushi/struct.Canvas.html),
  [`CanvasSizing`](/api/urushi/struct.CanvasSizing.html), and
  [`CanvasItem`](/api/urushi/trait.CanvasItem.html)
- [`CanvasContext`](/api/urushi/struct.CanvasContext.html),
  [`Position`](/api/urushi/struct.Position.html), and
  [`Size`](/api/urushi/struct.Size.html)
- [`Composition`](/api/urushi/enum.Composition.html),
  [`CanvasCell`](/api/urushi/struct.CanvasCell.html),
  [`CellContribution`](/api/urushi/struct.CellContribution.html), and
  [`PositionedCell`](/api/urushi/struct.PositionedCell.html)
- [`LineNetwork`](/api/urushi/struct.LineNetwork.html),
  [`LineContinuations`](/api/urushi/struct.LineContinuations.html), and
  [`LineGlyphs`](/api/urushi/struct.LineGlyphs.html)

## Surface and sizing

### `Canvas`

| API | Meaning |
|---|---|
| `Canvas::new()` | Empty Canvas using `CanvasSizing::viewport()` |
| `.sizing(policy)` | Replace the complete sizing policy |
| `.width(width)` | Fallback width for an unbounded axis |
| `.height(height)` | Fallback height for an unbounded axis |
| `.extent(Size)` | Set both fallback extents |
| `.item(value)` | Append one owned item to drawing order |
| `.items::<T>()` | Iterate directly owned items of concrete type `T` |

`Canvas::default()` is equivalent to `Canvas::new()`.

### `CanvasSizing`

| API | Meaning |
|---|---|
| `CanvasSizing::viewport()` | Consume finite parent allocation and use fallbacks on unbounded axes |
| `.width(width)` | Set the unbounded-axis width fallback |
| `.height(height)` | Set the unbounded-axis height fallback |
| `.extent(Size)` | Set both fallbacks |

Public callers can construct only viewport sizing. Intrinsic sizing is reserved
for built-in presentations.

### Geometry

| Type | Public API |
|---|---|
| `Position` | public `x: i64`, `y: i64`; `Position::new(x, y)` |
| `Size` | `ZERO`, `new`, `width`, `height`, `is_empty` |

Coordinates use signed terminal cells relative to the Canvas's top-left cell.

## Items and context

### `CanvasItem`

```rust
# use std::fmt::Debug;
# use urushi::CanvasContext;
pub trait CanvasItem: Debug + Send + Sync + 'static {
    fn draw(&self, context: &mut CanvasContext);
}
```

Values passed to `Canvas::item` must also implement `Clone + PartialEq`.

### `CanvasContext`

| API | Default composition | Notes |
|---|---|---|
| `size()` | — | Final Canvas `Size` |
| `bounds()` | — | `(Position::new(0, 0), size)` |
| `view(origin, view, width, height)` | `Replace` | Width and height are optional allocations |
| `view_with(..., composition)` | explicit | Placed View may carry anchors |
| `text(origin, text, style)` | `Overlay` | Plain text; no terminal controls |
| `text_with(..., composition)` | explicit | Same text contract |
| `line(from, to, marker, style)` | `Overlay` | Inclusive horizontal, vertical, or diagonal segment |
| `line_with(..., composition)` | explicit | Marker must be one printable, one-cell grapheme |
| `polyline(points, marker, style)` | `Overlay` | Joins every adjacent point pair |
| `polyline_with(..., composition)` | explicit | Zero or one point produces zero or one cell |
| `rectangle(origin, width, height, marker, style)` | `Overlay` | Closed cell-aligned path; zero extent draws nothing |
| `rectangle_with(..., composition)` | explicit | Same marker constraint |
| `line_network(network)` | `Overlay` | Junction-aware horizontal and vertical segments |
| `line_network_with(network, composition)` | explicit | Separate commands do not merge connectivity |
| `cells(cells)` | `Overlay` | Sparse `PositionedCell` values |
| `cells_with(cells, composition)` | explicit | Applies one rule to all supplied cells |

## Cells and composition

### `Composition`

| Variant | Result |
|---|---|
| `Replace` | Complete cell from the contribution; absent symbol/style become space/default |
| `Overlay` | Preserve absent fields and overlay supplied style values |
| `Custom(fn(&CanvasCell, &CellContribution) -> CanvasCell)` | Application-defined complete cell |

### `CanvasCell`

| API | Meaning |
|---|---|
| `new(symbol, style)` | Complete cell |
| `get_symbol()` | Current symbol |
| `get_style()` | Current complete `TextStyle` |
| `symbol(symbol)` | Replace the symbol |
| `style(style)` | Replace the complete style |

### `CellContribution`

| API | Meaning |
|---|---|
| `new()` | No symbol and no style |
| `symbol(symbol)` | Supply a symbol |
| `style(style)` | Supply a complete style contribution |
| `get_symbol()` | Optional supplied symbol |
| `get_style()` | Optional supplied style |

`PositionedCell::new(position, contribution)` associates a contribution with a
signed coordinate. Its `position` and `contribution` fields are public.

## Line networks

### `LineNetwork`

| API | Meaning |
|---|---|
| `new(glyphs, style)` | Empty network with one repertoire and style |
| `horizontal(y, columns)` | Inclusive horizontal segment |
| `horizontal_with(y, columns, continuations)` | Segment with endpoint incidence |
| `vertical(x, rows)` | Inclusive vertical segment |
| `vertical_with(x, rows, continuations)` | Segment with endpoint incidence |

Descending or otherwise empty inclusive ranges add no segment.

### `LineContinuations`

`NONE`, `START`, `END`, and `BOTH` control whether incidence continues beyond
the ascending start or end of a segment. They affect endpoint glyph selection
without drawing an outside cell.

### `LineGlyphs`

Built-in repertoires are `NORMAL`, `ROUNDED`, `ASCII`, `THICK`, `DOUBLE`, and
`HIDDEN`. A custom value supplies all 16 public glyph fields: isolated; four
ends; vertical and horizontal; four corners; four tees; and cross.

## Clipping and failure conditions

| Condition | Result |
|---|---|
| Canvas axis has neither finite allocation nor fallback extent | `LayoutErrorKind::CanvasExtent` |
| Placed View needs an allocation not supplied to `view` | `LayoutErrorKind::ViewAllocation` |
| Text contains a terminal control sequence | Construction or resolution panics according to the plain-text contract |
| Cell symbol is not exactly one printable grapheme | Panics |
| Path marker or line glyph is not exactly one printable, one-cell grapheme | Panics |
| Command extends beyond Canvas bounds | Outside cells are clipped |
| Wide grapheme would be split at an edge | Complete grapheme is omitted or cleared; no half-cell remains |

Canvas items never expand the surface and do not supply intrinsic measurement.
