---
title: Compose blocks and layouts
description: Arrange terminal content with blocks, rows, columns, dimensions, alignment, and overflow rules.
---

A `View` is a renderer-neutral presentation tree. Build leaves from text, wrap
them in blocks, then combine them into rows or columns.

## Stack content in a column

```rust
use urushi::{Align, TextStyle, View};

let view = View::column(
    Align::Left,
    [
        View::text("Build", TextStyle::new().bold()),
        View::text("Complete", TextStyle::new()),
    ],
);
```

```text title="Rendered output"
Build
Complete
```

`View::column` stacks children and aligns them horizontally. `View::row` places
children side by side and takes a `VerticalAlign`.

## Add box-model geometry

The declared width includes the border. Padding keeps the wrapped text away
from every edge:

```rust
use urushi::{BlockStyle, Border, Color, Length, Overflow, TextStyle, View};

let panel = BlockStyle::new()
    .border(Border::ROUNDED)
    .border_foreground(Color::BRIGHT_BLACK)
    .padding((1, 2))
    .width(Length::Cells(32))
    .overflow(Overflow::Wrap);

let view = View::block(
    panel,
    View::text("A status message that can wrap.", TextStyle::new()),
);
```

The resolved view is:

```text title="Rendered output"
╭──────────────────────────────╮
│                              │
│  A status message that can   │
│  wrap.                       │
│                              │
╰──────────────────────────────╯
```

`BlockStyle` owns margin, border, padding, dimensions, alignment, overflow, and
the style of the geometry it creates. Its text style does not implicitly flow
into children; every child carries its own complete style.

## Size against available space

Fixed cell lengths are not the only option. `Length::fill(weight)` distributes
remaining space among siblings. Minimum and maximum dimensions constrain the
result after demand and available space are considered.

Use `measure` when you need the natural size without a constraint. Use
`resolve` when you need the actual cell rectangle for a known area.

```rust
use urushi::{Available, resolve};

let resolved = resolve(&view, Available::columns(80))?;
println!("{} × {}", resolved.size().width(), resolved.size().height());
# Ok::<(), urushi::LayoutError>(())
```

For the panel above, this prints:

```text title="Program output"
32 × 6
```

For ordinary stdout and stderr, prefer `println_view` and its peers; they detect
the width and resolve for you.

## Build titled panels

`View::titled_block` places a title in the top border. The supplied
`BlockStyle` must have a top border edge.

```rust
use urushi::{BlockStyle, Border, TextStyle, View};

let view = View::titled_block(
    BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
    "Files",
    View::text("target/release/app", TextStyle::new()),
);
```

```text title="Rendered output"
┌ Files ─────────────┐
│ target/release/app │
└────────────────────┘
```

See [Styles and views](/docs/core/styles-and-views/) for the model behind these
types and [Terminal width and CJK text](/docs/core/text-width/) for measurement
rules.
