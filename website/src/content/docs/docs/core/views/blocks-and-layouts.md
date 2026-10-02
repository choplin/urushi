---
title: Rows and columns
description: Arrange Views horizontally or vertically and control their cross-axis alignment.
---

Rows and columns arrange already composed Views. Use
[Block geometry](/docs/core/styles/blocks/) for the rectangular frame around
one child.

## Build and print a layout

```rust
use std::io;

use urushi::{Align, BlockStyle, BlockTitle, Border, TextStyle, VerticalAlign, View};

fn main() -> io::Result<()> {
    let status = View::row(
        VerticalAlign::Center,
        [
            View::text("Status:", TextStyle::new().bold()),
            View::text(" ready", TextStyle::new()),
        ],
    );
    let panel = View::titled_block(
        BlockStyle::new().border(Border::ROUNDED).padding((0, 1)),
        BlockTitle::new("Build").align(Align::Center).padding(1),
        View::column(
            Align::Left,
            [View::text("Workspace", TextStyle::new()), status],
        ),
    );

    urushi::println_view(&panel)
}
```

```text title="Rendered output"
╭──── Build ────╮
│ Workspace     │
│ Status: ready │
╰───────────────╯
```

The block combines its child's width with horizontal padding and the border.

The order from outside to inside is margin, border, padding, then content.
`BlockStyle` controls dimensions, min/max constraints, alignment, and overflow
as well as the visible frame.

## Add a title

`titled_block` places one styled line in an enabled top border. The title never
wraps. It contributes to automatic width but is clipped when an explicit or
available width is narrower.

A titled block panics if its top border is absent or disabled.

```rust
# use urushi::{Align, BlockStyle, BlockTitle, Border, Length, TextStyle, View};
let view = View::titled_block(
    BlockStyle::new().border(Border::ROUNDED).width(Length::Cells(12)),
    BlockTitle::new("Long build title").align(Align::Center).padding(1),
    View::text("ready", TextStyle::new()),
);
# let _ = view;
```

```text title="Rendered output"
╭Long build╮
│ready     │
╰──────────╯
```

The title is clipped to the enabled top edge; it does not create a second row.

## Arrange children

- `row` lays children left to right and uses `VerticalAlign` on the cross axis.
- `column` lays children top to bottom and uses `Align` on the cross axis.

Cross-axis alignment matters when sibling rectangles have different sizes.
`Align::Left`, `Center`, and `Right` place narrower children inside a column;
`VerticalAlign::Top`, `Center`, and `Bottom` place shorter children inside a
row.

`Length::fill(weight)` on a child block divides remaining main-axis space among
fill siblings. Fixed, minimum, and maximum dimensions remain properties of the
child's `BlockStyle`.

The same two children make the cross-axis choice visible without changing
their own widths:

```rust
# use urushi::{Align, BlockStyle, Length, TextStyle, View};
# let short = || View::block(BlockStyle::new().width(Length::Cells(3)), View::text("x", TextStyle::new()));
# let wide = || View::block(BlockStyle::new().width(Length::Cells(7)), View::text("wide", TextStyle::new()));
let left = View::column(Align::Left, [wide(), short()]);
let center = View::column(Align::Center, [wide(), short()]);
let right = View::column(Align::Right, [wide(), short()]);
# let _ = (left, center, right);
```

```text title="Resolved cell placement"
Left       Center      Right
wide       wide        wide
x            x            x
```

For rows, `VerticalAlign::Top`, `Center`, and `Bottom` perform the same choice
on rows instead of columns. Main-axis `Length::fill(1)` claims equal shares;
weights `1` and `2` claim one-third and two-thirds of the remaining cells.

```rust
# use urushi::{Available, BlockStyle, Length, TextStyle, VerticalAlign, View, resolve};
# let tall = || View::block(BlockStyle::new().height(3), View::text("A", TextStyle::new()));
# let short = || View::text("B", TextStyle::new());
let top = View::row(VerticalAlign::Top, [tall(), short()]);
let center = View::row(VerticalAlign::Center, [tall(), short()]);
let bottom = View::row(VerticalAlign::Bottom, [tall(), short()]);
assert_eq!(resolve(&top, Available::NONE)?.size().height(), 3);
assert_eq!(resolve(&center, Available::NONE)?.size().height(), 3);
assert_eq!(resolve(&bottom, Available::NONE)?.size().height(), 3);

let weighted = View::row(VerticalAlign::Top, [
    View::block(BlockStyle::new().width(Length::fill(1)), View::empty()),
    View::block(BlockStyle::new().width(Length::fill(2)), View::empty()),
]);
assert_eq!(resolve(&weighted, Available::size(12, 1))?.size().width(), 12);
# Ok::<(), urushi::LayoutError>(())
```

| Setting | Resolved placement |
|---|---|
| `Top` | B is on row 0 of the 3-row sibling area |
| `Center` | B is on row 1 |
| `Bottom` | B is on row 2 |
| Fill weights `1:2` in 12 columns | 4 columns : 8 columns |
