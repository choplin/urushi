---
title: Show viewport position with a scrollbar
description: Present finite viewport state and compose the indicator beside content.
---

`Scrollbar` describes position using caller-defined units. All lengths may be
rows, columns, items, or another uniform unit, as long as they use the same
unit.

```rust
use urushi::{
    Available, Scrollbar, ScrollbarOrientation, ThemePreset, resolve,
};

let theme = ThemePreset::get("Catppuccin Mocha")
    .expect("built-in theme")
    .theme();
let scrollbar = Scrollbar::new(
    ScrollbarOrientation::Vertical,
    100,
    25,
)
.position(50);

let view = theme.scrollbar(&scrollbar);
let resolved = resolve(&view, Available::size(1, 8))?;
assert_eq!(resolved.size().height(), 8);
# Ok::<(), urushi::LayoutError>(())
```

```text title="Rendered output"
↑
│
│
│
█
█
│
↓
```

The presentation clamps an out-of-range position to the final origin where the
viewport remains inside content. Orientation selects the main axis, not the
edge; place the returned one-cell-cross-axis View beside content with a row or
column.

Clone `theme.components().scrollbar()` to choose proportional or one-cell
marker thumb sizing, replace vertical or horizontal glyph sets, or style the
thumb, track, and endpoints independently. Present glyphs must be one printable,
one-cell grapheme.

## Use a marker or replace glyphs

```rust
use urushi::{
    Available, Scrollbar, ScrollbarGlyphs, ScrollbarOrientation,
    ScrollbarThumbSizing, ThemePreset, resolve,
};

let theme = ThemePreset::get("Catppuccin Mocha")
    .expect("built-in theme")
    .theme();
let scrollbar = Scrollbar::new(ScrollbarOrientation::Horizontal, 100, 25)
    .position(50);
let presentation = theme
    .components()
    .scrollbar()
    .clone()
    .thumb_sizing(ScrollbarThumbSizing::Marker)
    .glyphs(
        ScrollbarOrientation::Horizontal,
        ScrollbarGlyphs::new("◆")
            .track(Some("·"))
            .begin(Some("["))
            .end(Some("]")),
    );
let view = presentation.compose(&scrollbar);
let resolved = resolve(&view, Available::size(12, 1))?;
assert_eq!(resolved.size().width(), 12);
# Ok::<(), urushi::LayoutError>(())
```

```text title="Rendered output"
[······◆···]
```

`Proportional` uses the viewport-to-content ratio for the thumb length;
`Marker` always uses one cell. Endpoints are shown only when the allocation can
hold both endpoints and at least one track cell. An out-of-range position is
clamped to `content_length - viewport_length`.

```text title="Allocation and position comparison"
proportional, 12 cells  [···███····]
marker, 12 cells        [······◆···]
marker, 2 cells         ·◆          (endpoints omitted)
position 500            same as position 75: the last valid origin
```

```rust
# use urushi::{Available, Scrollbar, ScrollbarOrientation, ThemePreset, resolve};
# let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let clamped = Scrollbar::new(ScrollbarOrientation::Horizontal, 100, 25)
    .position(500);
let wide = resolve(&theme.scrollbar(&clamped), Available::size(12, 1))?;
let narrow = resolve(&theme.scrollbar(&clamped), Available::size(2, 1))?;
assert_eq!(wide.size().width(), 12);
assert_eq!(narrow.size().width(), 2);
# Ok::<(), urushi::LayoutError>(())
```

```text title="Verification"
position 500 is clamped to 75; both finite allocations resolve successfully.
wide width = 12 cells; narrow width = 2 cells
```

Styles apply independently to the four logical parts:

```rust
# use urushi::{Color, TextStyle, ThemePreset};
# let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let presentation = theme.components().scrollbar().clone()
    .begin_style(TextStyle::new().foreground(Color::CYAN))
    .track_style(TextStyle::new().dim())
    .thumb_style(TextStyle::new().foreground(Color::GREEN).bold())
    .end_style(TextStyle::new().foreground(Color::CYAN));
# let _ = presentation;
```

<pre class="terminal-preview" aria-label="Independently styled scrollbar endpoints, track, and thumb"><code><span class="ansi-cyan">[</span><span class="ansi-dim">····</span><span class="ansi-green ansi-bold">◆</span><span class="ansi-dim">····</span><span class="ansi-cyan">]</span></code></pre>
