---
title: Draw paths and connected lines
description: Draw marker paths, rectangles, and junction-aware line networks.
---

Canvas provides two line models:

- marker paths draw one chosen grapheme along horizontal, vertical, or
  diagonal segments;
- `LineNetwork` joins horizontal and vertical segments with corners, tees, and
  crossings.

## Draw a line, polyline, or rectangle

```rust
# use urushi::{CanvasContext, Grapheme, Position, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
canvas.line(
    Position::new(0, 4),
    Position::new(4, 0),
    Grapheme::new("*"),
    TextStyle::new(),
);

canvas.polyline(
    [Position::new(5, 0), Position::new(8, 0), Position::new(8, 4)],
    Grapheme::new("+"),
    TextStyle::new(),
);
# }
```

In a 9×5 Canvas this produces:

```text title="Rendered output"
    *++++
   *    +
  *     +
 *      +
*       +
```

`polyline` connects each adjacent pair of points. `rectangle` is a closed,
cell-aligned marker path; zero width or height draws nothing.

For example, a five-by-three rectangle uses the requested width and height as
its complete outer extent:

```rust
# use urushi::{CanvasContext, Grapheme, Position, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
canvas.rectangle(
    Position::new(0, 0),
    5,
    3,
    Grapheme::new("#"),
    TextStyle::new(),
);
# }
```

```text title="Rendered output"
#####
#   #
#####
```

All endpoints are inclusive. Paths use `Composition::Overlay` unless the
corresponding `*_with` method specifies another rule.

## Derive corners and crossings

Use one `LineNetwork` when intersecting segments must choose box-drawing
glyphs from their combined connectivity:

```rust
# use urushi::{CanvasContext, LineGlyphs, LineNetwork, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
let mut network = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
network
    .horizontal(2, 0..=6)
    .vertical(3, 0..=4);

canvas.line_network(network);
# }
```

```text title="Rendered output"
   │
   │
───┼───
   │
   │
```

Only segments in the same `LineNetwork` form junctions. Separate network
commands compose as cells but do not merge connectivity.

Choose `LineGlyphs::NORMAL`, `ROUNDED`, `ASCII`, `THICK`, `DOUBLE`, or
`HIDDEN`, or construct a complete custom repertoire. Every repertoire
character must be printable and one cell wide.

```rust
# use urushi::LineGlyphs;
let repertoires = [
    LineGlyphs::NORMAL,
    LineGlyphs::ROUNDED,
    LineGlyphs::ASCII,
    LineGlyphs::THICK,
    LineGlyphs::DOUBLE,
    LineGlyphs::HIDDEN,
];
# let _ = repertoires;
```

```text title="The same corner in each repertoire"
NORMAL  ┌─   ROUNDED ╭─   ASCII  +-   THICK ┏━   DOUBLE ╔═   HIDDEN
        │            │           |          ┃          ║
```

## Continue a segment beyond its endpoint

`horizontal_with` and `vertical_with` accept `LineContinuations`. `START`,
`END`, and `BOTH` affect the glyph selected at an inclusive endpoint without
drawing another cell outside the supplied range. This is useful when rendering
one clipped portion of a larger network.

Here the first network ends at its top edge. The second declares that its
vertical segment continues upward beyond the visible range:

```rust
# use urushi::{CanvasContext, LineContinuations, LineGlyphs, LineNetwork, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
let mut ended = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
ended
    .horizontal(0, 2..=4)
    .vertical(2, 0..=2);
canvas.line_network(ended);

let mut continued = LineNetwork::new(LineGlyphs::NORMAL, TextStyle::new());
continued
    .horizontal(0, 8..=10)
    .vertical_with(8, 0..=2, LineContinuations::START);
canvas.line_network(continued);
# }
```

In an 11×3 Canvas, the two networks render side by side:

```text title="Rendered output"
  ┌──   ├──
  │     │
  │     │
```

`START` changes the visible endpoint from a corner to a tee. It does not draw
another cell above the Canvas.
