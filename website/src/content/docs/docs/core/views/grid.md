---
title: Align content in a grid
description: Share column widths across rows without adopting table semantics.
---

`View::grid` aligns cells into shared columns. Use it for one-off geometry. Use
a [Table component](/docs/core/components/table/) when the data needs headers,
table border policy, offsets, or reusable row presentation.

```rust
use std::io;

use urushi::{GridStyle, TextStyle, View};

fn main() -> io::Result<()> {
    let cell = |text| View::text(text, TextStyle::new());
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
        [
            [cell("Package"), cell("Status")],
            [cell("urushi"), cell("ready")],
        ],
    );

    urushi::println_view(&view)
}
```

```text title="Rendered output"
 Package  Status
 urushi   ready
```

## Set column lengths and padding

`GridStyle::columns` accepts one optional `Length` per column. `None` and
entries past the end of the list use the cells' intrinsic width. A fixed column
keeps its claim while auto columns shrink; a fill column consumes remaining
width when the grid receives a finite allocation.

```rust
use std::io;

use urushi::{BlockStyle, GridStyle, Length, TextStyle, View};

fn main() -> io::Result<()> {
    let cell = |text| View::text(text, TextStyle::new());
    let emphasized = |text| {
        View::block(
            BlockStyle::new().padding((0, 2)),
            View::text(text, TextStyle::new()),
        )
    };
    let view = View::grid(
        GridStyle::new()
            .cell_padding((0, 1))
            .columns([Some(Length::Cells(8)), None]),
        [
            [cell("Key"), cell("Value")],
            [emphasized("mode"), cell("release")],
        ],
    );

    urushi::println_view(&view)
}
```

```text title="Rendered output"
 Key     Value
  mode   release
```

Compared with the default, each setting changes one part of the cell geometry:

```text title="Baseline and configured grid"
default padding       configured columns and override
Key Value              Key     Value
mode release            mode   release
                       ^^^^^^^^ fixed 8-cell first column
                      ^^ cell-owned padding around mode
```

Cell padding belongs to the cell rectangle. A cell whose `BlockStyle` states
padding replaces the grid default; the two paddings are not added. The widest
cell, including its selected padding, still determines the shared column width.

## Keep the grid rectangular

Every row must contain the same number of cells. A ragged grid violates the
constructor contract; measuring or resolving it panics in debug builds:

```rust should_panic
use urushi::{GridStyle, TextStyle, View};

let cell = |text| View::text(text, TextStyle::new());
let view = View::grid(
    GridStyle::new(),
    [vec![cell("one"), cell("two")], vec![cell("three")]],
);
let _ = urushi::measure(&view);
```

```text title="Result"
panic: every grid row must have the same number of cells
```

Add an explicit empty `View` when a rectangular layout intentionally contains
a blank cell. Do not rely on a missing cell being invented.

## Choose Grid or Table

Grid supplies no headers, border rules, row meaning, or presentation policy.
Choose it when the shared columns are merely part of one View's geometry.
Choose `Table` when rows are semantic data, headers and separators carry
meaning, rows need offsets, or a reusable presentation should control the
result.
