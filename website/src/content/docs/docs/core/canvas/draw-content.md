---
title: Draw text, Views, and cells
description: Place composed Views, plain text, and sparse cell contributions on a Canvas.
---

Use `view` for existing Urushi composition, `text` for one plain text flow, and
`cells` for sparse cell-level contributions.

## Place an existing View

Components also produce Views, so the same command places a styled block, a
table, a tree, or any other composition:

```rust
# use urushi::{BlockStyle, Border, CanvasContext, Position, TextStyle, View};
# fn draw(canvas: &mut CanvasContext) {
let panel = View::block(
    BlockStyle::new().border(Border::ROUNDED).padding((0, 1)),
    View::text("ready", TextStyle::new().bold()),
);

canvas.view(Position::new(2, 0), panel, None, None);
# }
```

Placed at `(2, 0)` in an 11×3 Canvas, the View keeps its own border, padding,
and text layout:

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="A bordered View placed two Canvas cells from the left"><code>  ╭───────╮
  │ <span class="ansi-bold">ready</span> │
  ╰───────╯</code></pre>

The two `Option<usize>` arguments are the View's width and height allocations.
Use `None` for intrinsically sized content. Pass `Some` when the View depends on
a finite allocation.

`view` uses `Composition::Replace`, so spaces in the resolved rectangle clear
earlier Canvas content. Use `view_with` to select another composition rule.

## Draw plain text

```rust
# use urushi::{CanvasContext, Color, Position, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
canvas.text(
    Position::new(3, 0),
    "build complete",
    TextStyle::new().foreground(Color::GREEN).bold(),
);
# }
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Green bold text at Canvas x coordinate 3"><code>   <span class="ansi-green ansi-bold">build complete</span></code></pre>

This is the complete output of a 17×1 Canvas. The leading cells correspond to
the `x` coordinate. A capable color terminal also renders the text green and
bold.

Text can contain newlines and tabs but not terminal control sequences or raw
ANSI. It uses `Composition::Overlay` by default.

## Contribute sparse cells

`CellContribution` can provide a symbol, a style, or both. Missing fields are
transparent under the default overlay rule:

```rust
# use urushi::{CanvasContext, CellContribution, Color, Grapheme, Position, PositionedCell, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
canvas.cells([
    PositionedCell::new(
        Position::new(0, 0),
        CellContribution::new().symbol(Grapheme::new("●")),
    ),
    PositionedCell::new(
        Position::new(3, 0),
        CellContribution::new()
            .symbol(Grapheme::new("●"))
            .style(TextStyle::new().foreground(Color::GREEN)),
    ),
]);
# }
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Two sparse markers, with the second marker green"><code>●  <span class="ansi-green">●</span></code></pre>

This is the complete output of a 4×1 Canvas. The cells between the two
contributions remain untouched. On a capable color terminal, the second marker
is green.

A sparse symbol must be exactly one printable grapheme. Symbols supplied to
cell paths and line glyph sets must additionally occupy exactly one terminal
cell.
