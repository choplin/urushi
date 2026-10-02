---
title: Size and place a Canvas
description: Give a Canvas fallback extents or a finite parent allocation.
---

Canvas uses viewport sizing by default. On each axis, a finite allocation from
the parent wins. If an axis is unbounded, Canvas uses its explicit fallback
extent.

## Supply a fallback extent

Use `extent` when the Canvas must resolve without a finite parent:

```rust
use urushi::{Available, Canvas, Size, View, resolve};

let view = View::canvas(Canvas::new().extent(Size::new(20, 8)));
let resolved = resolve(&view, Available::NONE)?;

assert_eq!(resolved.size(), Size::new(20, 8));
# Ok::<(), urushi::LayoutError>(())
```

```text title="Resolved size"
fallback extent + unbounded parent → 20×8
```

Use `width` or `height` when only one unbounded axis needs a fallback:

```rust
use urushi::{Canvas, View};

let view = View::canvas(Canvas::new().width(20).height(8));
# let resolved = urushi::resolve(&view, urushi::Available::NONE)?;
# assert_eq!(resolved.size(), urushi::Size::new(20, 8));
# Ok::<(), urushi::LayoutError>(())
```

```text title="Resolved size"
width(20) + height(8) → 20×8
```

`CanvasSizing::viewport().extent(size)` expresses the same policy explicitly
and can be passed through `Canvas::sizing`.

## Let the parent allocate the surface

A finite parent allocation takes precedence over the fallback:

```rust
use urushi::{Available, Canvas, Size, View, resolve};

let view = View::canvas(Canvas::new().extent(Size::new(20, 8)));
let resolved = resolve(&view, Available::size(40, 12))?;

assert_eq!(resolved.size(), Size::new(40, 12));
# Ok::<(), urushi::LayoutError>(())
```

```text title="Resolved size comparison"
fallback only       → 20×8
parent allocation  → 40×12
```

This is useful in a full-screen frame or inside a block with finite dimensions.
The extent is not a fixed width or height; it is the answer only when that axis
is unbounded.

## Handle missing extents

Resolving a viewport-sized Canvas with neither a finite allocation nor a
fallback returns `LayoutErrorKind::CanvasExtent`. The error identifies the
first missing axis.

```rust
use urushi::{Available, Axis, Canvas, LayoutErrorKind, View, resolve};

let error = resolve(&View::canvas(Canvas::new()), Available::NONE)
    .expect_err("an unbounded Canvas needs a fallback extent");
assert_eq!(error.kind(), LayoutErrorKind::CanvasExtent);
assert_eq!(error.axis(), Axis::Width);
```

```text title="Error result"
Canvas requires an explicit Width on an unbounded axis
```

Application-defined `CanvasItem` values cannot provide intrinsic measurement.
Built-in component presentations may use an internal intrinsic policy, but
that policy is deliberately not a public extension point.

## Allocate Views placed inside Canvas

Canvas bounds are not an implicit allocation for a `view` command. Pass a
finite width or height when the placed View contains `Length::Fill` or another
area-dependent node on that axis:

```rust
# use urushi::{BlockStyle, CanvasContext, Length, Position, TextStyle, View};
# fn draw(canvas: &mut CanvasContext) {
let panel = View::block(
    BlockStyle::new().width(Length::fill(1)),
    View::text("ready", TextStyle::new()),
);

canvas.view(Position::new(0, 0), panel, Some(12), Some(1));
# }
```

```text title="Placed View allocation"
origin=(0, 0)  allocation=12×1  resolved width=12
```

Without the required allocation, resolution returns
`LayoutErrorKind::ViewAllocation` with `Axis::Width` for this panel.

```rust
use urushi::{
    Available, Axis, BlockStyle, Canvas, CanvasContext, CanvasItem, LayoutErrorKind,
    Length, Position, Size, TextStyle, View, resolve,
};

#[derive(Debug, Clone, PartialEq)]
struct FillPanel { width: Option<usize> }

impl CanvasItem for FillPanel {
    fn draw(&self, canvas: &mut CanvasContext) {
        let panel = View::block(
            BlockStyle::new().width(Length::fill(1)),
            View::text("ready", TextStyle::new()),
        );
        canvas.view(Position::new(0, 0), panel, self.width, Some(1));
    }
}

let ok = View::canvas(
    Canvas::new().extent(Size::new(12, 1)).item(FillPanel { width: Some(12) }),
);
assert_eq!(resolve(&ok, Available::NONE)?.size(), Size::new(12, 1));

let missing = View::canvas(
    Canvas::new().extent(Size::new(12, 1)).item(FillPanel { width: None }),
);
let error = resolve(&missing, Available::NONE).unwrap_err();
assert_eq!(error.kind(), LayoutErrorKind::ViewAllocation);
assert_eq!(error.axis(), Axis::Width);
# Ok::<(), urushi::LayoutError>(())
```

```text title="Allocation results"
Some(12) → placed View resolves to 12×1
None     → ViewAllocation on Width
```
