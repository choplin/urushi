---
title: Terminal graphics
description: Add Kitty or Sixel image regions to an Urushi View while preserving text fallback.
---

`urushi-graphics` adds raster images to the shared presentation model. An image
first reserves an anchored rectangle in an ordinary `View`; after layout,
the selected graphics protocol overlays pixels in that resolved rectangle. The
same View already contains fallback text for terminals without usable graphics.

This is separate from [Canvas](/docs/core/canvas/): Canvas draws graphemes in
terminal cells, while terminal graphics send pixel data through Kitty or Sixel.

## See the graphics paths

<div class="overview-catalog">
  <a href="#quickstart">
    <pre>┌──────────────┐
│ pixel image  │
│    [logo]    │
└──────────────┘</pre>
    <strong>Image region with fallback</strong>
    <span>The same View reserves cells for Kitty/Sixel pixels or readable text.</span>
  </a>
  <a href="/docs/graphics/rendering-and-lifecycle/#own-a-lifecycle-in-another-host">
    <pre><span class="demo-accent">Kitty</span>  retained
<span class="demo-warning">Sixel</span>  repaint
Text   fallback</pre>
    <strong>Explicit lifecycle ownership</strong>
    <span>One-shot output, Urushi runtime, or a caller-owned interactive host.</span>
  </a>
</div>

[Run the complete terminal graphics quickstart ↓](#quickstart)

## What the graphics surface provides

| Need | API or owner | Result |
|---|---|---|
| Put RGBA pixels in layout | `Image` + `ImagePresentation` | A fixed-cell View region with text fallback |
| Write one complete result and return | `render_view` | Stateless Kitty, Sixel, or fallback selection |
| Keep images across full-screen frames | `urushi-tui-app` with `graphics` | Runtime-owned selection, repaint, recovery, and cleanup |
| Integrate another interactive host | `KittyLifecycle` or `SixelLifecycle` | Caller-owned protocol state and frame policy |

## Quickstart

This complete Unix example displays one red pixel over an eight-by-three-cell
region. A supported terminal shows the scaled image; any other terminal keeps
the `[logo]` fallback.

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-terminal = "0.1.0"
```

```rust
use urushi_graphics::{
    CellSize, Image, ImagePresentation, PixelSize, render_view,
};
use urushi_terminal::backend::native::NativeTerminal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = Image::rgba(
        "logo-placement",
        "logo-rgba",
        PixelSize::new(1, 1),
        [196, 68, 52, 255],
    )?
    .fallback("[logo]");

    let view = ImagePresentation::new()
        .compose(&image, CellSize::new(8, 3));

    let mut terminal = NativeTerminal::open()?;
    render_view(&mut terminal, &view)?;
    Ok(())
}
```

The fallback cells occupy the same layout rectangle as the image:

```text title="Fallback output"
[logo]


```

`Image::rgba` validates that the byte length is `width × height × 4`. The first
key identifies this placement; the second identifies reusable pixel content.
`CellSize` controls layout in terminal cells, not source-pixel dimensions.

## Choose the presentation lifecycle

| Program shape | Start with |
|---|---|
| One rendered result followed by process exit | `render_view` |
| An Urushi TEA application | `Runtime::graphics` |
| A custom event loop or renderer | A protocol lifecycle owned by that host |
| Existing Ratatui application | Its loop must drive a protocol lifecycle; the adapter does not do so automatically |

Read [Rendering and lifecycle](/docs/graphics/rendering-and-lifecycle/) for
protocol selection, runtime configuration, clipping, and host responsibilities.
Use the [Graphics reference](/docs/graphics/reference/) for the complete public
surface.
