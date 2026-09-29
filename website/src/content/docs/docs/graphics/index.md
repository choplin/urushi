---
title: Display terminal images
description: Compose an image region with fallback text and render it through Kitty or Sixel when supported.
---

Image support lives in `urushi-graphics`; the core crate remains independent
from pixel data and graphics protocols. Images still participate in the same
layout as text: the presentation reserves a generic anchored region, and the
graphics layer overlays an image only after that complete view resolves.

## Install the graphics surface

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-terminal = "0.1.0"
```

## Create an image view

`Image::rgba` takes checked RGBA bytes and two stable identities: one for the
placement and one for the pixel asset. The byte length must match the supplied
pixel dimensions.

```rust
use urushi_graphics::{CellSize, Image, ImagePresentation, PixelSize};

let image = Image::rgba(
    "logo-placement",
    "logo-rgba",
    PixelSize::new(1, 1),
    [196, 68, 52, 255],
)?.fallback("[logo]");

let view = ImagePresentation::new().compose(
    &image,
    CellSize::new(8, 3),
);
# Ok::<(), urushi_graphics::InvalidRgbaRaster>(())
```

Before a graphics protocol is selected, that eight-by-three-cell view contains
the text fallback:

```text title="Fallback cells"
[logo]


```

The returned `View` contains an anchored image region and visible fallback
text. It can participate in ordinary row, column, block, and layout
composition.

## Render against terminal capabilities

`render_view` resolves and writes the complete view, locates its image regions,
and queries the supplied terminal for positive capability evidence.

```rust
use urushi_graphics::render_view;

let protocol = render_view(&mut terminal, &view)?;
```

The return value is the protocol used, or `None` when the fallback remains.

## Protocol selection

Urushi applies this order:

1. Kitty, when Kitty graphics support is detected;
2. Sixel, when Sixel support and cell-pixel geometry are both available; or
3. the text fallback already present in the view.

A terminal may report both protocols; Kitty wins. The one-shot renderer is
stateless and treats the terminal's top-left cell as the view origin.

Retained uploads, deletion, scrolling slices, protocol-specific repaint, and
draw-failure recovery require a redraw lifecycle. Their reusable protocol state
belongs in `urushi-graphics`, while the interactive host that opts in owns that
state. The 0.1.0 public API shown here is the stateless, one-shot path and does
not yet manage those retained operations.
