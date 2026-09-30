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

## Use images in the Urushi TUI runtime

The full-screen runtime can own image presentation across frames alongside its
cell output. Depend on the image model directly and enable the integration
feature:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-tui-app = { version = "0.1.0", features = ["graphics"] }
```

An application still returns an ordinary `View` containing an
`ImagePresentation`. Select automatic or explicit protocol behavior on the
runtime:

```rust
use urushi_tui_app::{GraphicsPreference, Runtime};

let final_model = Runtime::new(application)
    .graphics(GraphicsPreference::Auto)
    .run()?;
# let _ = final_model;
# Ok::<(), urushi_tui_app::Error>(())
```

For Kitty, the runtime retains uploads and placements between frames. Sixel is
immediate-mode terminal output: the runtime caches encoded cell-row bands but
emits the complete visible image scene on every successful presentation. The
same terminal owner coordinates repaint, recovery, and cleanup for either
protocol and restores the full-screen session. An explicit Kitty or Sixel
preference returns an error when the selected protocol cannot be used.

Hosts outside `urushi-tui-app` can own the public `kitty::KittyLifecycle` or
`sixel::SixelLifecycle` values themselves. They remain responsible for deciding
when to prepare, present, repaint, scroll, and clean up retained protocol state.
