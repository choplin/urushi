---
title: Project content and report anchors
description: Show a finite portion of a child and report resolved regions to another renderer.
---

Viewports and anchors solve different integration problems. A viewport projects
content; an anchor reports geometry.

## Project a larger child

```rust
use urushi::{
    Available, Projection, ProjectionBoundary, TextStyle, View, Viewport,
    resolve,
};

let view = View::viewport(
    Viewport::horizontal(Projection::new(2, ProjectionBoundary::Preserve)),
    View::text("abcdef", TextStyle::new()),
);
let resolved = resolve(&view, Available::size(3, 1))?;

assert_eq!(resolved.size().width(), 3);
# Ok::<(), urushi::LayoutError>(())
```

```text title="Three-cell viewport at column 2"
cde
```

`Preserve` uses the requested signed origin exactly, including blank space past
an edge. `Clamp` moves it so the child fills the viewport when possible.
Choose horizontal, vertical, or both-axis projection. Every projected axis
requires a finite allocation.

The application owns scroll state and changes the projection origin.

At an origin near the right edge, the boundary policy is the only difference:

```rust
# use urushi::{Projection, ProjectionBoundary, Viewport};
let preserve = Viewport::horizontal(Projection::new(5, ProjectionBoundary::Preserve));
let clamp = Viewport::horizontal(Projection::new(5, ProjectionBoundary::Clamp));
# let _ = (preserve, clamp);
```

```text title="Three-cell projection of abcdef"
Preserve: |f··|  (· is a blank cell beyond the child)
Clamp:    |def|
```

## Report a resolved region

```rust
use urushi::{Available, BlockStyle, Length, View, resolve};

let chart = View::anchor_block(
    "chart",
    BlockStyle::new().width(Length::Cells(20)).height(Length::Cells(8)),
    View::empty(),
);
let resolved = resolve(&chart, Available::NONE)?;
let region = resolved.anchor("chart").expect("chart anchor");

assert_eq!((region.width(), region.height()), (20, 8));
# Ok::<(), urushi::LayoutError>(())
```

```text title="Resolved anchor"
key=chart  x=0  y=0  width=20  height=8  visible=20×8
```

`anchor_block` behaves exactly like a block and reports its content rectangle.
`titled_anchor_block` adds a border title. `anchor` reports only a zero-size
origin, which is useful for cursor placement.

Anchors do not add focus, widget, or rendering semantics. The caller interprets
the opaque key and hands the rectangle to the foreign renderer.
