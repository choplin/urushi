---
title: Combine Components and Canvas
description: Place a semantic component View at coordinates and add Canvas annotations.
---

Compose the component first, then store its resulting View in an immutable
Canvas item. The component keeps its meaning and presentation while Canvas adds
explicit placement or geometry.

```rust
use std::io;

use urushi::{
    Canvas, CanvasContext, CanvasItem, LineGlyphs, LineNetwork, Position, Size,
    Table, TextStyle, ThemePreset, View,
};

#[derive(Debug, Clone, PartialEq)]
struct Dashboard {
    table: View,
}

impl CanvasItem for Dashboard {
    fn draw(&self, canvas: &mut CanvasContext) {
        canvas.view(
            Position::new(1, 1),
            self.table.clone(),
            Some(22),
            Some(5),
        );

        let mut connector = LineNetwork::new(
            LineGlyphs::ROUNDED,
            TextStyle::new(),
        );
        connector
            .horizontal(0, 0..=10)
            .vertical(0, 0..=2);
        canvas.line_network(connector);
    }
}

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let table = Table::text()
        .headers(["Job", "State"])
        .row(["build", "ready"]);

    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(24, 6))
            .item(Dashboard {
                table: theme.table(&table),
            }),
    );

    urushi::println_view(&view)
}
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="A themed Table View placed over a Canvas line network"><code>╭──────────
│┌───────┬───────┐
││ <span class="ansi-bold" style="color:#cdd6f4">Job</span>   │ <span class="ansi-bold" style="color:#cdd6f4">State</span> │
 ├───────┼───────┤
 │ <span style="color:#cdd6f4">build</span> │ <span style="color:#cdd6f4">ready</span> │
 └───────┴───────┘</code></pre>

Use `view` rather than flattening the table into strings. That preserves the
table's layout, styles, wide-character handling, and anchors. The placed View
contains an intrinsically measured component Canvas, but a Canvas View placed
inside another Canvas still requires explicit width and height allocations.
The placed View defaults to `Composition::Replace`; record background lines
before it when the component should cover them, or choose `view_with` for
another rule.
