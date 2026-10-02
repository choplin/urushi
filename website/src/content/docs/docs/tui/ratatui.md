---
title: Ratatui adapter
description: Draw Urushi views in an existing Ratatui application through the optional adapter crate.
---

`urushi-adapter-ratatui` adapts Urushi views and styles to a buffer owned by an
existing Ratatui application. It does not define Urushi's `Application` or
`View` model, and it does not install a terminal runtime.

## Install only the adapter

Add the adapter when an existing Ratatui application needs Urushi views and
styles.

```toml
[dependencies]
ratatui = "0.30"
urushi = "0.1.0"
urushi-adapter-ratatui = "0.1.0"
```

## Draw a complete view

`ViewWidget` resolves a view under the supplied `Rect` and writes it into the
frame buffer.

```rust
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use urushi::{BlockStyle, Border, Color, TextStyle, VerticalAlign, View};
use urushi_adapter_ratatui::ViewWidget;

fn main() {
    let view = View::row(
        VerticalAlign::Center,
        [
            View::text("status: ", TextStyle::new()),
            View::block(
                BlockStyle::new()
                    .border(Border::ROUNDED)
                    .border_foreground(Color::GREEN),
                View::text("ok", TextStyle::new().foreground(Color::GREEN)),
            ),
        ],
    );
    let area = Rect::new(0, 0, 12, 3);
    let mut buffer = Buffer::empty(area);
    ViewWidget::new(&view).render(area, &mut buffer);

    for y in area.top()..area.bottom() {
        let row = (area.left()..area.right())
            .map(|x| buffer.cell((x, y)).unwrap().symbol())
            .collect::<String>();
        println!("{row}");
    }
}
```

For this `view`, a status label beside a rounded block produces these cells
inside the target rectangle:

```text title="Ratatui buffer"
        ╭──╮
status: │ok│
        ╰──╯
```

The target rectangle is an input to Urushi's layout pass, not a crop applied
after layout. Box geometry is made to fit that area, and wide graphemes are
never split.

## Draw one themed block

Use `RatatuiStyleExt::widget` when an existing layout needs a single Urushi
block rather than a complete view tree.

```rust
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use urushi::{Align, PanelRole, ThemePreset};
use urushi_adapter_ratatui::RatatuiStyleExt as _;

let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let panel = theme
    .block_style(PanelRole::PanelFocused)
    .width(18)
    .align(Align::Center);
let area = Rect::new(0, 0, 18, 3);
let mut buffer = Buffer::empty(area);
panel.widget("Saved").render(area, &mut buffer);

assert_eq!(buffer.cell((6, 1)).unwrap().symbol(), "S");
```

In that 18-by-3 target rectangle, the resolved block is:

```text title="Rendered block (18 × 3)"
╭────────────────╮
│     Saved      │
╰────────────────╯
```

`TextStyle` conversion carries foreground, background, and supported text
attributes. Geometry remains on `BlockStyle` and is resolved by Urushi.

### Know which style information survives

Ratatui cells cannot represent every Urushi text-style field. Conversion is
deterministic rather than backend-dependent:

| Urushi `TextStyle` capability | Ratatui cell result |
|---|---|
| Foreground and background | Preserved, including ANSI, indexed, and RGB colors |
| Bold, dim, italic, slow/rapid blink, reversed, hidden, crossed out | Preserved as the corresponding Ratatui modifier |
| Single, double, curly, dotted, or dashed underline | Collapsed to Ratatui's plain `UNDERLINED` modifier |
| Underline color | Discarded |
| Fraktur, framed, encircled, overlined | Discarded because Ratatui has no corresponding modifier |
| OSC 8 hyperlink target and parameters | Discarded because a Ratatui cell has no hyperlink field |

This complete check shows both a supported conversion and a deliberate loss:

```rust
use ratatui::style::{Color as RatatuiColor, Modifier};
use urushi::{Color, TextStyle, UnderlineStyle};
use urushi_adapter_ratatui::RatatuiStyle;

fn main() {
    let logical = TextStyle::new()
        .foreground(Color::RED)
        .bold()
        .underline_style(UnderlineStyle::Double)
        .underline_color(Color::BLUE);
    let converted = RatatuiStyle::from(&logical).into_inner();

    assert_eq!(converted.fg, Some(RatatuiColor::DarkRed));
    assert!(converted.add_modifier.contains(Modifier::BOLD));
    assert!(converted.add_modifier.contains(Modifier::UNDERLINED));

    let linked = TextStyle::new().hyperlink("https://example.com");
    assert_eq!(
        RatatuiStyle::from(&linked),
        RatatuiStyle::from(&TextStyle::new()),
    );
}
```

```text title="Adapter-visible result"
foreground: dark red
modifiers:  bold | underlined
underline shape/color: not represented
hyperlink: not represented
```

If underline shape, underline color, or a hyperlink carries meaning, keep an
equivalent visible label or symbol in the View instead of relying on that style
field alone.

## Resolve once when you need placements

Resolve manually when the application needs the resolved dimensions or
anchors as well as the cells.

```rust
use ratatui::{buffer::Buffer, layout::Rect};
use urushi::{
    BlockStyle, Length, Projection, ProjectionBoundary, View, Viewport, resolve,
};
use urushi_adapter_ratatui::{anchor_placement, available, draw_resolved};

fn main() -> Result<(), urushi::LayoutError> {
    let view = View::viewport(
        Viewport::horizontal(Projection::new(2, ProjectionBoundary::Preserve)),
        View::anchor_block(
            "chart",
            BlockStyle::new()
                .width(Length::Cells(4))
                .height(Length::Cells(1)),
            View::empty(),
        ),
    );
    let area = Rect::new(10, 5, 3, 1);
    let resolved = resolve(&view, available(area))?;
    let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 10));
    draw_resolved(&resolved, area, &mut buffer);

    let anchor = resolved.anchors().first().unwrap();
    let placement = anchor_placement(anchor, area).unwrap();
    assert_eq!(placement.logical().x(), -2);
    assert_eq!(placement.logical().width(), 4);
    assert_eq!(placement.destination(), Rect::new(10, 5, 2, 1));
    assert_eq!(placement.source_column(), 2);
    assert_eq!(placement.source_row(), 0);
    Ok(())
}
```

```text title="Resolved anchor placement"
logical rectangle:    x=-2, width=4
visible destination:  x=10, y=5, width=2, height=1
source offset:        column=2, row=0
```

A sized anchor reserves a region whose placement only layout can know. An
application that owns the Ratatui buffer can translate that placement with
`anchor_placement` and draw a chart or another foreign widget into the returned
destination. For a partially clipped anchor, `source_column()` and
`source_row()` identify where the visible fragment begins inside the logical
region. `ViewWidget` only draws resolved cells, so resolve explicitly when
anchors are needed.

## Choose cell write behavior

Widgets merge their styles with earlier writes to the same cells by default.
Use `CellWriteMode::Replace` through the corresponding `*_with_mode` API when
the Urushi view must reset the cells it occupies.

```rust
use ratatui::{buffer::Buffer, layout::Rect, style::Color as RatatuiColor};
use urushi::{Color, TextStyle, View, resolve};
use urushi_adapter_ratatui::{
    CellWriteMode, available, draw_resolved_with_mode,
};

fn main() -> Result<(), urushi::LayoutError> {
    let area = Rect::new(0, 0, 1, 1);
    let view = View::text("x", TextStyle::new().foreground(Color::RED));
    let resolved = resolve(&view, available(area))?;

    let mut merge = Buffer::empty(area);
    merge.cell_mut((0, 0)).unwrap().set_bg(RatatuiColor::Blue);
    draw_resolved_with_mode(&resolved, area, &mut merge, CellWriteMode::Merge);

    let mut replace = Buffer::empty(area);
    replace.cell_mut((0, 0)).unwrap().set_bg(RatatuiColor::Blue);
    draw_resolved_with_mode(&resolved, area, &mut replace, CellWriteMode::Replace);

    assert_eq!(merge.cell((0, 0)).unwrap().bg, RatatuiColor::Blue);
    assert_eq!(replace.cell((0, 0)).unwrap().bg, RatatuiColor::Reset);
    Ok(())
}
```

| Mode | Existing blue background under `x` | Cells outside the view |
|---|---|---|
| `Merge` | Preserved because the Urushi style does not set a background | Untouched |
| `Replace` | Reset before the Urushi symbol and foreground are applied | Untouched |

The adapter does not interpret ANSI escape sequences stored in text. Build
styles as Urushi values and keep view content plain.

## What an existing application still owns

When using `ViewWidget` or `draw_resolved`, the application remains responsible
for terminal setup and restoration, event input, state, focus, scrolling,
cursor placement, frame scheduling, and errors. These adapters are a way to
place Urushi presentation inside an existing loop; they do not install a
second runtime.

For Urushi's application model and runtime entry points, see
[Application runtime](/docs/tui/runtime/).

The adapter depends on `urushi` and Ratatui, but not on `urushi-tui` or
`urushi-tui-app`. Ratatui version selection therefore remains isolated from
Urushi's native frame engine and application runtime.

Terminal images are another host-owned layer: the adapter writes only cells
and never creates or advances a Kitty or Sixel lifecycle. If an Urushi View
contains image anchors, the Ratatui application must resolve that same View,
draw its cell fallback, and present the image scene in its own frame order.
See [Rendering and lifecycle](/docs/graphics/rendering-and-lifecycle/#own-a-lifecycle-in-another-host)
for the required ownership, cleanup, resize, and failure ordering.

## Present terminal images after the Ratatui frame

Ratatui's `CrosstermBackend<Stdout>` and Urushi's
`urushi_terminal::backend::crossterm::CrosstermBackend<Stdout>` can be two
handles to the same physical terminal. Keep both in one host and use them
strictly in sequence: Ratatui draws and flushes the cell frame first, then the
Urushi command writer presents the image scene.

Add the graphics and terminal crates beside the adapter:

```toml
urushi-graphics = "0.1.0"
urushi-terminal = { version = "0.1.0", features = ["crossterm"] }
```

This host supports either retained Kitty images or immediate-mode Sixel. The
caller still owns terminal session setup, the event loop, and the decision that
a Sixel image scene changed.

```rust
use std::io;

use ratatui::{
    Terminal,
    backend::CrosstermBackend as RatatuiCrossterm,
    layout::Rect,
};
use urushi::{View, resolve};
use urushi_adapter_ratatui::{available, draw_resolved};
use urushi_graphics::{kitty::KittyLifecycle, sixel::SixelLifecycle};
use urushi_terminal::{
    PixelSize as CellPixelSize,
    backend::crossterm::CrosstermBackend as UrushiCrossterm,
};

enum Graphics {
    Kitty {
        lifecycle: KittyLifecycle,
        cell_pixels: Option<CellPixelSize>,
    },
    Sixel {
        lifecycle: SixelLifecycle,
        cell_pixels: CellPixelSize,
    },
}

impl Graphics {
    fn present(
        &mut self,
        view: &View,
        resolved: &urushi::ResolvedView,
        output: &mut UrushiCrossterm<io::Stdout>,
    ) -> io::Result<()> {
        match self {
            Self::Kitty { lifecycle, cell_pixels } => {
                lifecycle.present(view, resolved, *cell_pixels, output)
            }
            Self::Sixel { lifecycle, cell_pixels } => {
                lifecycle.present(view, resolved, *cell_pixels, output)
            }
        }
    }

    fn clear(
        &mut self,
        output: &mut UrushiCrossterm<io::Stdout>,
    ) -> io::Result<()> {
        match self {
            Self::Kitty { lifecycle, .. } => lifecycle.clear(output),
            Self::Sixel { lifecycle, .. } => lifecycle.clear(output),
        }
    }

    fn is_sixel(&self) -> bool {
        matches!(self, Self::Sixel { .. })
    }
}

struct RatatuiGraphicsHost {
    terminal: Terminal<RatatuiCrossterm<io::Stdout>>,
    graphics_output: UrushiCrossterm<io::Stdout>,
    graphics: Graphics,
}

impl RatatuiGraphicsHost {
    fn new(graphics: Graphics) -> io::Result<Self> {
        Ok(Self {
            terminal: Terminal::new(RatatuiCrossterm::new(io::stdout()))?,
            graphics_output: UrushiCrossterm::new(io::stdout()),
            graphics,
        })
    }

    fn draw(
        &mut self,
        view: &View,
        area: Rect,
        sixel_scene_changed: bool,
    ) -> io::Result<()> {
        let resolved = resolve(view, available(area)).map_err(io::Error::other)?;

        // Sixel has no retained placement deletion. Clear stale pixels before
        // redrawing the complete cell fallback for a changed image scene.
        if sixel_scene_changed && self.graphics.is_sixel() {
            self.graphics.clear(&mut self.graphics_output)?;
        }

        self.terminal.draw(|frame| {
            draw_resolved(&resolved, area, frame.buffer_mut());
        })?;
        self.graphics
            .present(view, &resolved, &mut self.graphics_output)
    }

    fn resize(&mut self, area: Rect) -> io::Result<()> {
        // Delete old Kitty placements, or clear Sixel pixels, before changing
        // the cell geometry used to resolve the next frame.
        self.graphics.clear(&mut self.graphics_output)?;
        self.terminal.resize(area)
    }

    fn clear_graphics(&mut self) -> io::Result<()> {
        self.graphics.clear(&mut self.graphics_output)
    }
}

fn kitty_host(
    cell_pixels: Option<CellPixelSize>,
) -> io::Result<RatatuiGraphicsHost> {
    RatatuiGraphicsHost::new(Graphics::Kitty {
        lifecycle: KittyLifecycle::new(),
        cell_pixels,
    })
}

fn sixel_host(cell_pixels: CellPixelSize) -> io::Result<RatatuiGraphicsHost> {
    RatatuiGraphicsHost::new(Graphics::Sixel {
        lifecycle: SixelLifecycle::new(),
        cell_pixels,
    })
}
```

For an ordinary Kitty frame, call `host.draw(&view, area, false)`. For Sixel,
pass `true` whenever image content, placement, or visibility changed; the host
then clears stale pixels before rebuilding cells and presenting the complete
scene. On `Event::Resize`, call `host.resize(new_area)` before the next draw.
Before restoring or releasing the terminal session, call
`host.clear_graphics()` so either protocol removes everything it owns.

If Ratatui drawing succeeds and graphics presentation fails, keep the same
lifecycle and retry `draw`. Both lifecycles recover conservatively on their
next operation; the application remains responsible for deciding whether to
retry, show the cell fallback, or exit.
