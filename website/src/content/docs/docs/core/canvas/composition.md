---
title: Control composition and clipping
description: Layer Canvas commands, define custom cell composition, and understand clipping.
---

Every Canvas command is rasterized independently and applied in recorded order.
Its `Composition` decides how each new contribution combines with the current
cell.

## Choose a composition rule

| Rule | Behavior |
|---|---|
| `Replace` | Missing symbol becomes a space; missing style becomes the default style |
| `Overlay` | Missing fields remain unchanged; supplied styles overlay the existing style |
| `Custom` | A pure function returns the complete resulting `CanvasCell` |

`view` defaults to `Replace`. `text`, `line`, `polyline`, `rectangle`,
`line_network`, and `cells` default to `Overlay`. Every command has a
corresponding `*_with` method for an explicit rule.

The following item draws the same background on three rows, then applies one
style-only contribution and one `*` symbol using each rule:

```rust
use urushi::{
    CanvasCell, CanvasContext, CanvasItem, CellContribution, Composition,
    Grapheme, Position, PositionedCell, TextStyle,
};

fn keep_symbol(
    existing: &CanvasCell,
    contribution: &CellContribution,
) -> CanvasCell {
    let style = contribution
        .get_style()
        .cloned()
        .unwrap_or_else(|| existing.get_style().clone());

    existing.clone().style(style)
}

#[derive(Debug, Clone, PartialEq)]
struct CompositionComparison;

impl CanvasItem for CompositionComparison {
    fn draw(&self, canvas: &mut CanvasContext) {
        let contributions = || {
            [
                PositionedCell::new(
                    Position::new(1, 0),
                    CellContribution::new()
                        .style(TextStyle::new().bold()),
                ),
                PositionedCell::new(
                    Position::new(2, 0),
                    CellContribution::new()
                        .symbol(Grapheme::new("*")),
                ),
            ]
        };

        for (y, rule) in [
            (0, Composition::Replace),
            (1, Composition::Overlay),
            (2, Composition::Custom(keep_symbol)),
        ] {
            canvas.text(
                Position::new(0, y),
                "ABCDE",
                TextStyle::new(),
            );
            canvas.cells_with(
                contributions().map(|cell| {
                    PositionedCell::new(
                        Position::new(cell.position.x, y),
                        cell.contribution,
                    )
                }),
                rule,
            );
        }
    }
}
```

In a 5×3 Canvas, the rows show `Replace`, `Overlay`, and `Custom` in that
order:

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Replace, Overlay, and Custom composition including bold style contributions"><code>A *DE
A<span class="ansi-bold">B</span>*DE
A<span class="ansi-bold">B</span>CDE</code></pre>

`Replace` turns the missing symbol in the first contribution into a space.
`Overlay` preserves that cell's `B` and replaces only the supplied `C` with
`*`. The custom function applies contributed styles but preserves every
existing symbol.

## Define custom composition

The custom function receives the complete existing cell and the sparse new
contribution. The comparison above passes `keep_symbol` as
`Composition::Custom(keep_symbol)`. The function must be a plain function
pointer; it cannot capture application state.

## Understand clipping

Commands may begin at negative coordinates or extend past any Canvas edge.
Rasterized cells outside the finite surface are discarded. Clipping does not
move the command and does not change Canvas size.

For example, this 6×4 rectangle begins two cells left and one cell above a 4×3
Canvas:

```rust
# use urushi::{CanvasContext, Grapheme, Position, TextStyle};
# fn draw(canvas: &mut CanvasContext) {
canvas.rectangle(
    Position::new(-2, -1),
    6,
    4,
    Grapheme::new("#"),
    TextStyle::new(),
);
# }
```

Only the cells inside the Canvas remain; the rectangle is not shifted into
view:

```text title="Rendered output"
   #
   #
####
```

Wide graphemes are atomic. Urushi never leaves half of a wide grapheme at an
edge. Writing over either cell clears the complete earlier grapheme before the
new cell is installed.

Anchors inside a placed View retain their signed, unclipped geometry and also
report their visible rectangle inside the resolved Canvas.
