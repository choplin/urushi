---
title: Styles and views
description: Understand how Urushi separates inline appearance, box geometry, composition, layout, and rendering.
---

Urushi represents presentation as values before selecting an output backend.
The main distinction is between style and view.

## `TextStyle` describes inline appearance

`TextStyle` contains foreground and background colors, attributes, underline,
and an optional OSC 8 hyperlink. It deliberately contains no geometry.

```rust
use urushi::{Color, TextStyle, UnderlineStyle};

let link = TextStyle::new()
    .foreground(Color::CYAN)
    .bold()
    .underline_style(UnderlineStyle::Single)
    .hyperlink("https://example.com");
```

Builders consume and return the value. Clone a base style before deriving a
variant.

## `BlockStyle` describes a rectangle

`BlockStyle` adds margin, border, padding, dimensions, alignment, and overflow.
Its own text style fills the geometry the block creates. It does not cascade
into child views.

```rust
use urushi::{BlockStyle, Border, Color};

let panel = BlockStyle::new()
    .background(Color::BLACK)
    .border(Border::ROUNDED)
    .border_foreground(Color::BRIGHT_BLACK)
    .padding((1, 2));
```

Keeping text and geometry separate makes invalid combinations unrepresentable:
an inline `TextStyle` cannot accidentally carry a border or padding.

## `View` describes composition

A `View` can contain text, a block, a row, a column, a grid, a canvas, a
viewport, or a keyed anchor block. `View::empty` and `View::anchor` are
convenient degenerate forms rather than additional node kinds. The tree does
not know whether it will become ANSI text, a Ratatui buffer, or another
renderer's cell rectangle.

```rust
use urushi::{Align, TextStyle, VerticalAlign, View};

let label = View::text("Status: ", TextStyle::new().bold());
let value = View::text("ready", TextStyle::new());

let row = View::row(VerticalAlign::Top, [label, value]);
let view = View::column(Align::Left, [row]);
```

```text title="Rendered output"
Status: ready
```

## Project content through a viewport

`Viewport` projects one child into a finite rectangle. The application owns the
origin as ordinary state; the view owns only the pure projection and its edge
behavior.

```rust
use urushi::{
    Available, Projection, ProjectionBoundary, TextStyle, View, Viewport,
    resolve,
};

let view = View::viewport(
    Viewport::horizontal(
        Projection::new(2, ProjectionBoundary::Preserve),
    ),
    View::text("abcdef", TextStyle::new()),
);
let resolved = resolve(&view, Available::size(3, 1))?;
# Ok::<(), urushi::LayoutError>(())
```

```text title="Three-cell viewport at column 2"
cde
```

`Preserve` keeps the requested origin even when that exposes blank space near
an edge. `Clamp` moves the origin back far enough to keep the finite viewport
filled when the child permits it. Navigation, scroll commands, and external
data loading remain application behavior.

## Resolution and rendering are separate

`resolve` performs width measurement, wrapping, box sizing, and placement under
an `Available` area. It returns a rectangular `ResolvedView` of styled terminal
graphemes. `render` then serializes that rectangle using explicit
`RenderSettings`.

This separation is the shared foundation across Urushi's surfaces. A component
or application composes presentation into a `View` once; stdout, redirected
plain text, prompt framing, and full-screen backends consume the same resolved
cell geometry without making their terminal lifecycle part of the tree.
