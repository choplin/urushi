---
title: Layout
description: Size, arrange, align, clip, and project Views in terminal-cell space.
---

Layout determines **where a View's cells go and how much space they receive**.
It is not a second presentation model: layout operations construct and resolve
the same [`View`](/docs/core/views/) used by CLI output, prompts, TUIs, Canvas,
and the Ratatui adapter.

## Choose a layout tool

| Need | Use | Read |
|---|---|---|
| Margin, border, padding, dimensions, alignment, or overflow around one child | `View::block` with `BlockStyle` | [Block geometry](/docs/core/styles/blocks/) |
| Horizontal or vertical flow | `View::row` or `View::column` | [Rows and columns](/docs/core/views/blocks-and-layouts/) |
| Shared column widths without table semantics | `View::grid` with `GridStyle` | [Grid](/docs/core/views/grid/) |
| A finite window onto larger content | `View::viewport` | [Viewports and anchors](/docs/core/views/projection-and-anchors/) |
| Report final geometry to a cursor, image, or host renderer | `View::anchor` or `View::anchor_block` | [Viewports and anchors](/docs/core/views/projection-and-anchors/#report-a-resolved-region) |
| Explicit coordinates, overlap, or connected lines | `View::canvas` | [Canvas](/docs/core/canvas/) |

`BlockStyle` contains both geometry and the appearance of cells created by a
block. Its Rust name describes the value applied to a block; in this site its
margin, border, padding, dimensions, alignment, and overflow are documented
under Layout because those are layout tasks. Cell color and attributes remain
under [Styles](/docs/core/styles/).

## See the layout tools

<div class="overview-catalog">
  <a href="/docs/core/styles/blocks/">
    <pre>margin
┌─ border ─┐
│ padding  │
│ content  │
└──────────┘</pre>
    <strong>Block geometry</strong>
    <span>Margin, border, padding, dimensions, alignment, and overflow.</span>
  </a>
  <a href="/docs/core/views/blocks-and-layouts/">
    <pre>row:    A │ B │ C

column: A
        B</pre>
    <strong>Rows and columns</strong>
    <span>Flow, cross-axis alignment, fixed sizes, and weighted fill.</span>
  </a>
  <a href="/docs/core/views/grid/">
    <pre>Name    State
core    <span class="demo-success">ready</span>
prompt  ready</pre>
    <strong>Grid and viewport</strong>
    <span>Shared columns, finite projections, and resolved anchors.</span>
  </a>
</div>

[Run the complete Layout quickstart ↓](#quickstart)

## Quickstart

This complete example gives two children weighted horizontal space inside a
finite row:

```rust
use std::io;

use urushi::{BlockStyle, Border, Length, TextStyle, VerticalAlign, View};

fn main() -> io::Result<()> {
    let panel = |label, weight| {
        View::block(
            BlockStyle::new()
                .border(Border::ROUNDED)
                .width(Length::fill(weight)),
            View::text(label, TextStyle::new()),
        )
    };
    let row = View::row(
        VerticalAlign::Top,
        [panel("build", 1), panel("tests", 2)],
    );
    let view = View::block(BlockStyle::new().width(30), row);

    urushi::println_view(&view)
}
```

With 30 available columns, fill weights `1` and `2` divide the row into 10 and
20 columns:

```text title="Rendered output"
╭────────╮╭──────────────────╮
│build   ││tests             │
╰────────╯╰──────────────────╯
```

The row owns horizontal allocation. Each block then resolves its border and
child inside the width it receives. The resulting value is still one View and
can be rendered by any Urushi surface.

## Continue by task

- [Configure block geometry](/docs/core/styles/blocks/)
- [Compose rows and columns](/docs/core/views/blocks-and-layouts/)
- [Align cells in a grid](/docs/core/views/grid/)
- [Project content and report anchors](/docs/core/views/projection-and-anchors/)
- [Look up the complete layout and View API](/docs/core/views/reference/)
