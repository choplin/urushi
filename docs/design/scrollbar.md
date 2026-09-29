# Scrollbar Presentation

How one finite viewport position becomes a selectable, area-dependent terminal
indicator. [`../component-model.md`](../component-model.md) places Scrollbar at
the semantic-data and presentation boundary; this document owns its exact data,
glyph, geometry, sizing, and edge-case rules.

## Semantic input

`Scrollbar` carries four facts in one caller-defined content unit:

```rust
pub enum ScrollbarOrientation {
    Vertical,
    Horizontal,
}

pub struct Scrollbar { /* orientation, content length, viewport length, position */ }

impl Scrollbar {
    pub const fn new(
        orientation: ScrollbarOrientation,
        content_length: usize,
        viewport_length: usize,
    ) -> Self;
    pub const fn position(self, position: usize) -> Self;
}
```

Rows, columns, fixed-height records, and another uniform content unit are all
valid. Mixing units is not. The value describes one frame; it does not update a
position, interpret input, navigate content, or synchronize a `Viewport`.
Orientation says which content axis the values describe. It does not say which
edge of a parent contains the bar: the parent places the returned `View` on its
left, right, top, or bottom like any other child.

Position is retained as requested and clamped for presentation to:

```text
maximum = content_length - min(viewport_length, content_length)
effective_position = min(position, maximum)
```

Retaining the request makes the semantic snapshot honest when content shrinks;
safe geometry does not require every caller to duplicate normalization.

## Selectable presentation

`ScrollbarPresentation` owns every visual and geometric choice. It stores a
`ScrollbarGlyphs` value for each orientation, a complete `TextStyle` for each
of the `Thumb`, `Track`, `Begin`, and `End` roles, and one thumb-sizing policy:

- `Proportional` represents both viewport size and position;
- `Marker` uses one cell and represents position only.

Each glyph is exactly one printable, one-cell grapheme. The thumb is required;
track and endpoint glyphs are optional. Distinct complete styles let the same
geometry appear as Unicode lines and blocks, as a blank glyph with contrasting
backgrounds, or as another terminal-cell repertoire without style inheritance
or backend-specific values.

The presentation deliberately supports both orientation glyph sets at once.
One Theme-owned value can therefore compose either orientation, while a caller
can clone it and replace only the vertical or horizontal set for a local use.
An omitted track records no cells in its range; the resulting cells retain the
Canvas's terminal-default blanks. A thumb-only `▐` display is consequently one
local configuration, not a separate semantic component or the canonical
Urushi appearance.

The canonical presentation uses a thin `│` or `─` track, a full-block thumb,
and arrow endpoints. `Theme` derives the thumb style from its accent token and
the other three styles from its border token. `Theme::scrollbar` delegates to
that stored presentation. Local variation clones the presentation or owns a
separate one; `ComponentTheme` does not become a presentation registry.

## Area-dependent lowering

Composition binds the semantic snapshot, selected orientation glyphs, four
styles, and sizing policy into one viewport-sized Canvas item. A default-style
Block fixes the cross axis and supplies the finite main-axis allocation:

- vertical: width is exactly one cell and height fills a finite allocation;
- horizontal: height is exactly one cell and width fills a finite allocation.

An unbounded main axis is a Canvas extent error. A scrollbar has no usable
track until layout supplies the edge where it ends, so inferring an intrinsic
main-axis size would manufacture geometry that neither component data nor
presentation policy contains. The one-cell cross axis means side placement is
ordinary Row or Column composition rather than an orientation variant such as
`VerticalRight`.

## Track and thumb geometry

Let `extent` be the final main-axis cell count. No cells are recorded for zero
extent or zero content length. Otherwise, configured begin and end glyphs are
kept only when all configured endpoints plus at least one position-indicator
cell fit. If they do not fit, all endpoints are omitted. Position information
therefore wins over decoration without biasing a two-cell bar toward either
end.

The remaining cells form a non-empty track of length `T`. For proportional
sizing:

```text
visible = min(viewport_length, content_length)
maximum = content_length - visible

maximum == 0:
    thumb_length = T
    thumb_start  = 0
otherwise:
    thumb_length = clamp(round(visible * T / content_length), 1, T)
    travel       = T - thumb_length
    thumb_start  = round(effective_position * travel / maximum)
```

For marker sizing:

```text
thumb_length = 1
travel       = T - 1
thumb_start  = 0                                      when maximum == 0
               round(effective_position * travel / maximum) otherwise
```

`round` is deterministic nearest-integer rounding with half values toward the
larger integer. Intermediates are widened before multiplication, so extreme
content values neither overflow nor change endpoint behavior. Both policies
map a non-degenerate range's first and last position exactly to its first and
last available thumb origin.

A zero-length viewport over non-empty content remains a valid position: a
proportional presentation uses its required one-cell minimum. When the viewport
covers or exceeds content, the range degenerates to its beginning; a
proportional thumb covers the complete track and a marker occupies its first
cell. Zero content draws nothing because no content position exists.

The presentation records the track first, endpoints in their reserved cells,
and the thumb last. Every contribution uses `Replace`, so each part's
`TextStyle` is complete and the thumb replaces rather than patches the track.

## Why this is not a Viewport feature

`Viewport` projects one already-composed child from a caller-selected origin.
It does not know whether its coordinates count semantic items or cells, and it
does not own the full content length needed for a position indicator. Coupling
the two would make projection produce application state and would prevent a
Scrollbar from representing semantic windowing that constructs only the
visible subset. The caller may feed both values from one model, but neither
primitive derives the other.

## Rejected alternatives

### One fixed terminal appearance

A full rail, a thumb-only edge, a background-color bar, and a one-cell marker
all express the same viewport state under different space and visual policies.
Fixing one glyph set or proportional geometry in `Scrollbar` would turn a
consumer's current appearance into semantic data. The concrete presentation
keeps those choices replaceable without adding a shared presentation trait.

### Edge-bearing orientations

`VerticalLeft`, `VerticalRight`, `HorizontalTop`, and `HorizontalBottom` are
useful when a backend widget paints into an existing rectangle and selects one
of its edges. Urushi returns a View that participates in layout. Encoding the
edge would duplicate Row and Column placement and make the component depend on
its eventual parent.

### Backend widget adaptation

Delegating Scrollbar to a Ratatui stateful widget would give one backend a
different component model and rounding behavior. The Canvas item produces the
same resolved cells for ANSI, Ratatui, and future output adapters.

### An open drawing callback

The two observed policies are proportional thumb and one-cell marker. A public
callback or trait for unknown algorithms would expose geometry internals before
a second implementation establishes a stable shared contract. A structurally
different future indicator can receive its own named presentation when it is
implemented.
