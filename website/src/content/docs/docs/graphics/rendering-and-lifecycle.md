---
title: Rendering and lifecycle
description: Select one-shot, runtime-owned, or caller-owned Kitty and Sixel presentation.
---

Every path starts with the same `Image` and `ImagePresentation`. The difference
is who owns terminal capability observation and graphics state after the View
has resolved.

## Compose image regions

An image has two identities:

- the placement key tracks one logical image region across layouts and frames;
- the asset key identifies immutable RGBA content that a renderer may reuse.

```rust
use urushi::{Align, Available, TextStyle, View, resolve};
use urushi_graphics::{
    CellSize, Image, ImagePresentation, PixelSize, image_placements,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = Image::rgba(
        "avatar-placement",
        "avatar-v3",
        PixelSize::new(2, 1),
        [255, 0, 0, 255, 0, 0, 255, 255],
    )?
    .fallback("[avatar]");

    let image_view = ImagePresentation::new()
        .fallback_style(TextStyle::new().italic())
        .compose(&image, CellSize::new(12, 4));

    let view = View::column(
        Align::Left,
        [View::text("Profile", TextStyle::new()), image_view],
    );
    let resolved = resolve(&view, Available::NONE)?;
    let placement = image_placements(&view, &resolved, None)[0];
    assert_eq!(placement.logical_size().width(), 12);
    assert_eq!(placement.logical_size().height(), 4);
    Ok(())
}
```

The composed View has five rows. The fallback is written into the same
12-by-4 anchored rectangle that a graphics protocol will cover:

```text title="Resolved cell output before a graphics overlay"
Profile
[avatar]



```

When Kitty or Sixel presentation succeeds, pixels cover the last four rows;
with text selection or no usable protocol, the cell output above remains the
visible result.

The View participates in rows, columns, blocks, viewports, and clipping before
any protocol runs. Its anchor reports both the logical rectangle and the
visible clipped rectangle. Read [Viewports and anchors](/docs/core/views/projection-and-anchors/#report-a-resolved-region)
for that boundary.

## Render once

`render_view` owns one complete presentation. It queries terminal size and
capabilities, resolves and writes the cell layer, then overlays every visible
image. It retains no protocol state after returning.

Selection is deterministic:

| Evidence | Result |
|---|---|
| Kitty is positively confirmed | Kitty |
| Otherwise Sixel and cell-pixel geometry are available | Sixel |
| Neither path is usable | Existing text fallback |

Kitty wins when both protocols are available. The terminal's top-left cell is
the View origin. The returned value is the protocol used, or `None` when only
the fallback was written.

Use this path for a command that draws once and exits. It is not a frame loop:
it cannot reconcile retained uploads, clear stale placements, or recover a
later frame after partial protocol output.

## Let the Urushi runtime own frames

For a full-screen application, depend on the image model directly and enable
the runtime integration:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-tui-app = { version = "0.1.0", features = ["graphics"] }
```

The application continues to return an ordinary View containing image regions.
Configure protocol policy on the runtime that already owns drawing and terminal
restoration:

```rust
use urushi_tui_app::{GraphicsPreference, Runtime};

fn run<A: urushi_tui_app::Application>(
    application: A,
) -> Result<A::Model, urushi_tui_app::Error> {
    let final_model = Runtime::new(application)
        .graphics(GraphicsPreference::Auto)
        .run()?;
    Ok(final_model)
}
```

`Auto` prefers Kitty, then usable Sixel, then text. `Kitty` and `Sixel` require
that exact protocol and fail startup when the terminal cannot provide it.
`Text` disables graphics and keeps fallbacks.

On Unix the graphics-enabled default runtime uses Urushi's native
bidirectional terminal connection. It queries positive Kitty and Sixel evidence
before its input reader starts, so the example above can select a protocol
without a separate backend. On platforms where that native connection is not
available, the portable default backend supplies no positive graphics evidence:
`Auto` selects text, and an explicit protocol needs a custom `TerminalBackend`
whose `TerminalQuery` reports it.

The runtime owns the protocol consequences:

- Kitty uploads and placements are retained and reconciled across frames;
- Sixel encoded cell-row bands are cached, while every successful presentation
  emits the complete visible image scene;
- resize, draw failure, fallback redraw, cleanup, and terminal restoration are
  coordinated with the cell frame.

The [TUI runtime guide](/docs/tui/runtime/#add-terminal-images) shows where this
option fits into the application lifecycle.

## Own a lifecycle in another host

An existing Ratatui loop, a caller-driven `urushi-tui::Screen`, or another
interactive renderer remains the terminal owner. The Ratatui adapter only draws
cells; it does not send image protocol commands.

Such a host keeps one `kitty::KittyLifecycle` or `sixel::SixelLifecycle` for one
terminal presentation. For each frame it must:

1. resolve the same View used to produce the cell frame;
2. clear or redraw protocol state when the scene or surface requires it;
3. present the complete desired image scene after the cell layer;
4. handle partial output failure and retry policy; and
5. call `clear` before resize teardown, terminal release, or exit.

Kitty reconciles persistent terminal uploads and placements. Sixel is
immediate-mode: `present` emits every visible image band, and `clear` clears the
screen so the host must redraw the complete cell frame before presenting again.
These APIs expose mechanism; frame ordering remains the host's responsibility.

This is the minimum compilable Kitty host boundary. The host resolves exactly
the View whose cells it drew, retains one lifecycle between frames, and clears
it before the surface is invalidated or the terminal is released:

```rust
use std::io;
use urushi::{Available, View, resolve};
use urushi_graphics::kitty::KittyLifecycle;
use urushi_terminal::{CommandWriter, PixelSize as CellPixelSize};

struct GraphicsHost {
    kitty: KittyLifecycle,
}

impl GraphicsHost {
    fn new() -> Self {
        Self { kitty: KittyLifecycle::new() }
    }

    fn present(
        &mut self,
        view: &View,
        available: Available,
        cell_pixels: Option<CellPixelSize>,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        let resolved = resolve(view, available).map_err(io::Error::other)?;
        // Draw the cell frame derived from `resolved` before this call.
        self.kitty.present(view, &resolved, cell_pixels, terminal)
    }

    fn clear(
        &mut self,
        terminal: &mut (impl CommandWriter + ?Sized),
    ) -> io::Result<()> {
        self.kitty.clear(terminal)
    }
}
```

| Host event | Required order | Resulting protocol state |
|---|---|---|
| ordinary frame | resolve → draw cells → `present` | unchanged uploads are reused; placements are reconciled |
| resize | `clear` → resize/redraw → `present` | old placements are deleted before new geometry is committed |
| exit or terminal release | `clear` → restore session | every image possibly owned by this lifecycle is deleted |
| failed `present` | retain lifecycle, then retry `present` or `clear` | possible partial state is conservatively reset on the next call |

For Sixel, retain `SixelLifecycle` and pass a non-zero `CellPixelSize` to
`present`. On a changed scene call `clear`, redraw the complete cell frame,
then call `present`; Sixel presentation is immediate-mode rather than retained.

## Clipping and fallbacks

Image placement follows the resolved anchor, including negative positioning,
viewport projection, and clipping. A fully hidden or empty placement emits no
image. A partially visible placement maps the visible cell rectangle back to a
source-pixel crop when cell-pixel geometry is known.

Fallback text is not a separate error screen. It is already present in the
View. A graphics presenter masks those cells only when it successfully owns the
image scene; recovery can therefore redraw the complete cell frame as text.

The following complete placement check applies a one-cell horizontal clip to
a two-cell image. A 20-pixel-wide source therefore exposes its right 10 pixels:

```rust
use urushi::{
    Available, Canvas, CanvasContext, CanvasItem, Position, Size, TextStyle,
    VerticalAlign, View, resolve,
};
use urushi_graphics::{
    CellSize, Image, ImagePresentation, PixelPosition, PixelSize,
};
use urushi_terminal::PixelSize as CellPixelSize;

#[derive(Debug, Clone, PartialEq)]
struct ShiftLeft(View);

impl CanvasItem for ShiftLeft {
    fn draw(&self, context: &mut CanvasContext) {
        context.view(Position::new(-1, 0), self.0.clone(), Some(2), Some(2));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let image = Image::rgba(
        "photo", "photo-rgba", PixelSize::new(20, 20), vec![255; 20 * 20 * 4],
    )?
    .fallback("[photo]");
    let image_view = ImagePresentation::new().compose(&image, CellSize::new(2, 2));
    let clipped = View::canvas(
        Canvas::new()
            .extent(Size::new(1, 2))
            .item(ShiftLeft(image_view)),
    );
    let view = View::row(
        VerticalAlign::Top,
        [View::text("x", TextStyle::new()), clipped],
    );
    let resolved = resolve(&view, Available::NONE)?;
    let placement = image
        .placement(&resolved, Some(CellPixelSize::new(8, 16)))
        .unwrap();

    assert_eq!(placement.logical_origin(), Position::new(0, 0));
    assert_eq!(placement.logical_size(), Size::new(2, 2));
    assert_eq!(placement.visible_origin(), Some(Position::new(1, 0)));
    assert_eq!(placement.visible_size(), Some(Size::new(1, 2)));
    assert_eq!(placement.source_offset(), Some(PixelPosition::new(10, 0)));
    assert_eq!(placement.source_size(), Some(PixelSize::new(10, 20)));
    Ok(())
}
```

| Path | Visible result for the clipped region |
|---|---|
| Kitty | destination cells `(1,0)..(2,2)`, cropped from source pixels `(10,0)..(20,20)` |
| Sixel | the same visible crop, emitted as complete visible cell-row bands |
| Text fallback | the same one-cell viewport clips the fallback cells; no protocol bytes are emitted |
