---
title: Define a custom CanvasItem
description: Store immutable drawing data and record responsive commands after Canvas sizing.
---

Implement `CanvasItem` when drawing should be reusable or depend on the final
Canvas size. Keep the value immutable; record all frame commands in `draw`.

## Draw relative to the final size

```rust
use std::io;

use urushi::{
    Canvas, CanvasContext, CanvasItem, Grapheme, Position, Size, TextStyle,
    View,
};

#[derive(Debug, Clone, PartialEq)]
struct BottomRightLabel {
    text: String,
}

impl CanvasItem for BottomRightLabel {
    fn draw(&self, canvas: &mut CanvasContext) {
        let size = canvas.size();
        canvas.rectangle(
            Position::new(0, 0),
            size.width(),
            size.height(),
            Grapheme::new("·"),
            TextStyle::new(),
        );

        let width = urushi::PrintableText::new(&self.text).width();
        let x = size.width().saturating_sub(width) as i64;
        let y = size.height().saturating_sub(1) as i64;

        canvas.text(
            Position::new(x, y),
            self.text.clone(),
            TextStyle::new().bold(),
        );
    }
}

fn main() -> io::Result<()> {
    let view = View::canvas(
        Canvas::new()
            .extent(Size::new(12, 4))
            .item(BottomRightLabel {
                text: "ready".into(),
            }),
    );

    urushi::println_view(&view)
}
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="A bold label positioned at the Canvas bottom-right"><code>············
·          ·
·          ·
·······<span class="ansi-bold">ready</span></code></pre>

The item reads the settled 12×4 size, draws its boundary, and derives the
label's bottom-right position from that size. A different parent allocation
moves both without changing the item type.

`Canvas::item` additionally requires the concrete value to implement `Clone`
and `PartialEq`. Together with the `CanvasItem` bounds, stored items are
`Debug + Clone + PartialEq + Send + Sync + 'static`. This keeps a complete
`View` cloneable, comparable, and safe to move between runtime owners.

## Recover adapter-owned items

An adapter that owns a concrete item type can inspect those values without
fallible downcasting:

```rust
# use urushi::{Canvas, CanvasContext, CanvasItem};
# #[derive(Debug, Clone, PartialEq)]
# struct BottomRightLabel { text: String }
# impl CanvasItem for BottomRightLabel { fn draw(&self, _: &mut CanvasContext) {} }
let canvas = Canvas::new().item(BottomRightLabel {
    text: "ready".into(),
});

for label in canvas.items::<BottomRightLabel>() {
    assert_eq!(label.text, "ready");
}
```

```text title="Verification"
items::<BottomRightLabel>() yields the stored label with text "ready".
```

Core layout does not inspect item meaning. `items::<T>()` is intended for an
adapter that owns both the item type and its specialized rendering behavior.

## Keep application state outside the item

A Canvas item represents immutable frame data, not a nested application. Input
handling, animation state, focus, and event subscriptions belong to the caller.
Build a new item value when that state changes and return a new `View` from the
next render.
