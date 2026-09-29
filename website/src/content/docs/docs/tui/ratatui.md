---
title: Use the Ratatui integration
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
use ratatui::layout::Rect;
use urushi::{BlockStyle, Border, Color, TextStyle, VerticalAlign, View};
use urushi_adapter_ratatui::ViewWidget;

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

terminal.draw(|frame| {
    frame.render_widget(ViewWidget::new(&view), Rect::new(0, 0, 12, 3));
})?;
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
use ratatui::layout::Rect;
use urushi::{Align, PanelRole};
use urushi_adapter_ratatui::RatatuiStyleExt as _;

let panel = theme
    .block_style(PanelRole::PanelFocused)
    .align(Align::Center);

terminal.draw(|frame| {
    let area = Rect::new(0, 0, 18, 3);
    frame.render_widget(panel.widget("Saved"), area);
})?;
```

In that 18-by-3 target rectangle, the resolved block is:

```text title="Rendered block (18 × 3)"
╭────────────────╮
│     Saved      │
╰────────────────╯
```

`TextStyle` conversion carries foreground, background, and supported text
attributes. Geometry remains on `BlockStyle` and is resolved by Urushi.

## Resolve once when you need placements

Resolve manually when the application needs the resolved dimensions or
anchors as well as the cells.

```rust
use urushi::{Key, resolve};
use urushi_adapter_ratatui::{anchor_placement, available, draw_resolved};

let resolved = resolve(&view, available(area))?;
draw_resolved(&resolved, area, frame.buffer_mut());

let chart_key = Key::from("chart");
if let Some(anchor) = resolved
    .anchors()
    .iter()
    .find(|anchor| anchor.key() == chart_key)
{
    if let Some(placement) = anchor_placement(anchor, area) {
        frame.render_widget(chart, placement.destination());
    }
}
# Ok::<(), urushi::LayoutError>(())
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

The adapter does not interpret ANSI escape sequences stored in text. Build
styles as Urushi values and keep view content plain.

## What an existing application still owns

When using `ViewWidget` or `draw_resolved`, the application remains responsible
for terminal setup and restoration, event input, state, focus, scrolling,
cursor placement, frame scheduling, and errors. These adapters are a way to
place Urushi presentation inside an existing loop; they do not install a
second runtime.

For Urushi's application model and runtime entry points, see
[TUI runtime](/docs/tui/runtime/).

The adapter depends on `urushi` and Ratatui, but not on `urushi-tui` or
`urushi-tui-app`. Ratatui version selection therefore remains isolated from
Urushi's native frame engine and application runtime.
