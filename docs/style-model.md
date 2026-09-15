# Style Value Model

This document defines Urushi's logical style contract: the value model
governing [`TextStyle`](../urushi/src/style/text.rs) and
[`BlockStyle`](../urushi/src/style/block.rs).

The two style types themselves, and the view tree they style, are defined in
[`view-model.md`](view-model.md); why primitive styling splits between them is
settled in [`design/view-block-model.md`](design/view-block-model.md); why the
value model has this shape is recorded in
[`design/style-value-model.md`](design/style-value-model.md). Three topics
inside the model have files of their own — the canonical form
([`design/style-canonical-form.md`](design/style-canonical-form.md)), the
underline ([`design/underline.md`](design/underline.md)), and border edges
([`design/border-edges.md`](design/border-edges.md)) — linked from the sections
that summarize them.

The value model governs both types identically: immutable named builders over
typed effective values. Statements below written about `TextStyle` hold for
`BlockStyle` too; sections that concern geometry apply to `BlockStyle` alone,
since `TextStyle` has no geometry to describe.

## Immutability and replacement

A style is an immutable collection of effective presentation values. It is not
a patch, an instruction list, or a completed ANSI state transition.

Calling a builder consumes a style and returns a new value. The original
style remains unchanged:

```rust
let base = TextStyle::new().foreground(Color::CYAN);
let focused = base.clone().bold();
```

Each singleton property has at most one value. Adding another value of the
same kind replaces it. Modifiers are a set, so they are unioned and subtracted
individually.

```rust
let style = TextStyle::new()
    .foreground(Color::CYAN)
    .add_modifier(Modifier::BOLD | Modifier::ITALIC)
    .remove_modifier(Modifier::ITALIC);
```

`TextStyle::new()` and `TextStyle::default()` are empty styles.

Named operations restore ordinary defaults in the resulting value: no color,
no border, zero spacing, automatic width and height, no minimum or maximum
bounds, wrap overflow, left horizontal alignment, or top vertical alignment.

## Typed property vocabulary

The text vocabulary is foreground, background, underline, hyperlink, and
modifier. A hyperlink is one URI plus zero or more OSC 8 parameters;
construction percent-encodes control and delimiter characters so caller input
cannot escape its field. The block vocabulary is the geometry — padding,
margin, the border glyph set, its four edge switches, complete logical text
style, and color overrides, the six sizing properties, overflow, and the two
alignments — together with named access to the text properties of the style
filling it. The complete border style lets a semantic rail retain modifiers as
well as colors; explicit border foreground and background values override
those two properties.

Geometry methods exist only on `BlockStyle`, so a geometry property cannot be
applied to a `TextStyle`. Shared text operations use the same names on both
types: `foreground`, `add_modifier`, `without_underline`, and their peers.

The values are chosen so that one appearance has one value: an underline is
one optional value carrying its shape and its color, not a modifier flag beside
a color property, and Select Graphic Rendition (SGR) parameter 6 (rapid blink)
is deliberately absent. The full value shape and the precise rules for its
public operations are recorded in
[`design/style-value-model.md`](design/style-value-model.md); the underline
value and its builders in [`design/underline.md`](design/underline.md).

## Canonical form

One duplication survives the vocabulary: an underline color equal to the
foreground. It is a duplication *between* fields, so no signature makes it
unrepresentable, and it is closed by normalization once the style is final —
`RenderSettings::resolve_text_style` applies the fold as its last step, so an
effective style selected for output is canonical.
The rule for admitting any fold is narrow: **fold only what is inert** — a
value may be dropped only when doing so cannot change the output, whatever the
terminal does. The precise fold, its timing, its one unclosable residue, and
the reasoning are recorded in
[`design/style-canonical-form.md`](design/style-canonical-form.md).

## Public API

The public API names the operation a consumer intends:

```rust
let style = TextStyle::new()
    .foreground(Color::CYAN)
    .add_modifier(Modifier::BOLD)
    .without_foreground()
    .remove_modifier(Modifier::BOLD);

let documentation = TextStyle::new().hyperlink("https://example.com/docs");
```

Construction uses concise named builders:

```rust
let style = BlockStyle::new()
    .foreground(Color::CYAN)
    .bold()
    .padding((0, 1))
    .border(Border::ROUNDED)
    .width(20)                     // Length::Cells(20) through From<u16>
    .height(Length::fill(1))
    .min_width(12)
    .max_width(60)
    .overflow(Overflow::ellipsis())
    .align(Align::Center)
    .align_vertical(VerticalAlign::Center);
```

An operation that removes an optional value names the resulting domain state:
`without_border` removes the glyph set, `auto_width` restores automatic width,
and `without_max_width` removes that bound. Values whose defaults are ordinary
arguments use their existing builder: zero padding is
`padding(Sides::default())`, the default left alignment is
`align(Align::default())`, and a disabled border edge is re-enabled with
`border_left(true)`. `GridStyle` similarly restores its defaults with
`columns([])` and `cell_padding(Sides::default())`.

Complete underline values use `with_underline`; the shorter `underline`,
`underline_style`, and `underline_color` builders cover ordinary construction.
Runtime-selected modifier sets use `add_modifier` and `remove_modifier`, which
accept any `Modifier` set; `bold`, `italic`, and the other common single flags
are the short construction path.

## Composition

`TextStyle` has no `patch` or `inherit` operation. Deriving one style from another
uses the immutable builders directly:

```rust
let focused = base
    .clone()
    .foreground(accent)
    .bold()
    .remove_modifier(Modifier::DIM);
```

Reusable sequences of changes are ordinary functions:

```rust
fn focused(style: TextStyle, accent: Color) -> TextStyle {
    style.foreground(accent).bold()
}
```

If an application needs several transforms, it can fold functions over a base
style. There is no separate patch data type; the reasoning is recorded in
[`design/style-value-model.md`](design/style-value-model.md).

## Geometry properties

`BlockStyle` carries the geometry of the box it fills. As style values:

- `width` and `height` take a `Length` — `Cells` or `Fill` — and their absence
  means auto; `min_width`, `min_height`, `max_width`, and `max_height` are
  bounds in cells, absent by default and removed with `without_min_width`,
  `without_max_width`, and their height peers, not a zero value. `auto_width`
  and `auto_height` remove a stated dimension. A dimension is a preferred
  size, resolved under the available
  area: shorter content is padded out to it, longer content is absorbed by the
  overflow rule, and the frame closes at the resolved size either way.
- `overflow` is the policy for content that does not fit — `Overflow::Wrap`
  (the default) or `Overflow::Clip` with an application-chosen marker.
- `align` and `align_vertical` place shorter content inside the resolved box.
- `border` sets a glyph set; the four `border_*` sides, `true` by default, say
  which edges are drawn; `border_foreground` and `border_background` color
  every enabled edge uniformly.

What these values mean under layout — the box they measure, the clamp, the
distribution across siblings, what a marker costs, and how each edge contributes
rows and columns — is defined by the view model:
[`design/box-sizing.md`](design/box-sizing.md),
[`design/area-sharing.md`](design/area-sharing.md),
[`design/overflow.md`](design/overflow.md), and
[`design/border-edges.md`](design/border-edges.md).

Conversions between the outer box and the content area inside it go through
one query: `frame_size()` returns the per-axis overhead of enabled border
edges plus padding. Margin lies outside the box and keeps its own getter.

## Theme contract

Themes store complete logical values of both kinds. `Theme::text_style` and
`TextThemeRole::resolve` return an owned `TextStyle` for text roles;
`Theme::block_style` and `BlockThemeRole::resolve` return an owned `BlockStyle`
for roles whose value is a rectangle, such as `PanelRole` and the cell roles of
the built-in table component. An application role of either kind builds its
value with the ordinary consuming builders.

`ComponentTheme::with_text_style` replaces a text role's style,
`ComponentTheme::with_panel` and `with_panel_focused` replace the panel blocks,
and `Theme::components` exposes the stored built-in styles and canonical
presentations as borrows for consumers that want to avoid the copy. Component
presentations are not style values: their separate role and naming are defined
in [`component-model.md`](component-model.md).

`urushi-cli::CliTheme` derives its own roles and presentations from a core
`Theme`; core does not reserve the CLI-specific Warning role or store its
presentations.

Styles do not implicitly flow from a parent view to a child: as
[`view-model.md`](view-model.md) specifies, each child carries its own
complete value, so a theme role resolves to the whole style its position uses.

## Output-boundary contract

Renderers consume the values present in one `TextStyle`.

- ANSI rendering emits the active colors, modifiers, and underline, in SGR
  parameter order so that one style always spells one sequence. It emits a
  hyperlink as an OSC 8 scope around the styled run and closes the scope before
  a line boundary.
- The `urushi-tui` adapter maps the active modifier set to Ratatui's
  `add_modifier`; it does not populate `sub_modifier`. Ratatui's cell model has
  no hyperlink target or parameter field, so the adapter intentionally discards
  hyperlinks while retaining every representable text property.
- `RenderSettings::resolve_text_style` independently selects color fidelity,
  modifiers, underline shapes, underline color, and hyperlinks, then returns a
  canonical effective style.

A style value does not serialize an arbitrary string itself. `render_text`
serializes the styled spans of a `StyledText` without layout, while `render`
serializes the styled graphemes of a `ResolvedView`. This keeps output feature
selection at the rendering boundary instead of making it a natural-looking
escape hatch on `TextStyle`.

How each backend spells and degrades the underline is recorded in
[`design/underline.md`](design/underline.md).

Removing a modifier from an immutable `TextStyle` removes the value; a renderer
that maintains prior terminal state is responsible for diffing previous and
next effective styles and emitting any required reset codes.
[`design/style-value-model.md`](design/style-value-model.md) records why a
style holds no removal instruction.

`render` surrounds emitted SGR styling with a final reset and closes hyperlink
scopes at line boundaries. The layout pass accepts plain model text, never a
rendered string; its `String` output is terminal data rather than a layout input.
