# Style Value Model

This document defines Urushi's logical style contract: the value model
governing [`TextStyle`](../urushi/src/style/text.rs) and
[`BlockStyle`](../urushi/src/style/block.rs).

The two style types themselves, and the view tree they style, are defined in
[`view-model.md`](view-model.md); why presentation splits between them is
settled in [`design/view-block-model.md`](design/view-block-model.md); why the
value model has this shape is recorded in
[`design/style-value-model.md`](design/style-value-model.md).

The value model governs both types identically: the same immutable builders,
the same closed vocabulary, the same generic `add` and `remove`. Statements
below written about `TextStyle` hold for `BlockStyle` too; sections that
concern geometry apply to `BlockStyle` alone, since `TextStyle` has no
geometry to describe.

## Value model

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
    .add(Modifier::BOLD | Modifier::ITALIC)
    .remove(Modifier::ITALIC);
```

`TextStyle::new()` and `TextStyle::default()` are empty styles. Removing a property
restores its ordinary default in the resulting value: no color, no border,
zero spacing, automatic width and height, left horizontal alignment, or top
vertical alignment.

## Closed property vocabulary

The generic API uses one closed enum pair per type. The text vocabulary:

```rust
enum TextStyleProperty {
    Foreground(Color),
    Background(Color),
    Modifier(Modifier),
}

enum TextStylePropertyKey {
    Foreground,
    Background,
    Modifier(Modifier),
}
```

The block vocabulary is the geometry plus, through its `Text` variant, every
text property of the style filling it:

```rust
enum BlockStyleProperty {
    Text(TextStyleProperty),
    Padding(Sides),
    Margin(Sides),
    Border(Border),
    BorderTop(bool),
    BorderRight(bool),
    BorderBottom(bool),
    BorderLeft(bool),
    BorderForeground(Color),
    BorderBackground(Color),
    Width(u16),
    Height(u16),
    MaxWidth(u16),
    MaxHeight(u16),
    Align(Align),
    VerticalAlign(VerticalAlign),
}

enum BlockStylePropertyKey {
    Text(TextStylePropertyKey),
    Padding,
    Margin,
    Border,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    BorderForeground,
    BorderBackground,
    Width,
    Height,
    MaxWidth,
    MaxHeight,
    Align,
    VerticalAlign,
}
```

`TextStyleProperty` converts into `BlockStyleProperty`, so `BlockStyle::add` accepts
a text property directly and `BlockStyle::foreground` reads the same as
`TextStyle::foreground`. There is no conversion in the other direction:
geometry cannot reach a `TextStyle`.

The enums make the complete property vocabulary discoverable and give generic
code an exhaustive match. Both types may store the values in typed fields rather
than allocating an enum collection; all mutation still passes through the
closed `add` and `remove` operations.

## Public API

`add` and `remove` are the complete generic operations:

```rust
let style = TextStyle::new()
    .add(TextStyleProperty::Foreground(Color::CYAN))
    .add(Modifier::BOLD)
    .remove(TextStylePropertyKey::Foreground)
    .remove(Modifier::BOLD);
```

Common properties also have concise named builders for completion and
readability:

```rust
let style = BlockStyle::new()
    .foreground(Color::CYAN)
    .bold()
    .padding((0, 1))
    .border(Border::ROUNDED)
    .width(20)
    .height(5)
    .max_width(18)
    .max_height(6)
    .align(Align::Center)
    .align_vertical(VerticalAlign::Center);
```

Border edge visibility has named builders and getters as well as generic
properties:

```rust
let separator = BlockStyle::new()
    .border(Border::NORMAL)
    .border_top(false)
    .border_right(false)
    .border_bottom(true)
    .border_left(false);

assert!(separator.is_border_bottom_enabled());
```

`border(Border)` enables the familiar four-sided rendering by default. Each
side setting is an independent effective value. Removing a side property, such
as `TextStylePropertyKey::BorderLeft`, restores its default value of `true`.
Removing `Border` removes only the glyph set; it does not rewrite the four side
values, which remain inactive until a border is added again.

These builders are thin wrappers over `add`; they do not define a second
behavior. There are no `unset_*` methods. Generic `remove` is the single way to
delete a property.

## Composition

`TextStyle` has no `patch` or `inherit` operation. Deriving one style from another
uses the immutable builders directly:

```rust
let focused = base
    .clone()
    .foreground(accent)
    .bold()
    .remove(Modifier::DIM);
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

## Fixed dimensions

`width` and `height` describe the padded content box. Padding is inside the
requested dimensions; enabled border columns and rows, followed by margin, are
outside them. For example, `height(3)` with a top and bottom border produces
five rows before margin.

Fixed height is a minimum, not a clipping limit. Content taller than the fixed
height expands the box so no row is discarded. An output boundary with a finite
area, such as a Ratatui `Rect`, may still clip the resolved box to that area.
Dedicated maximum-dimension properties own content truncation when present;
fixed height does not. Fixed width is a minimum in the same sense: a grapheme
wider than the requested width expands the content box rather than being
dropped, because the layout pass never splits a wide character. This happens in
the layout pass, so every backend sees the expanded box; it is not a Ratatui
adjustment made against the available `Rect`.

Shorter content is top-aligned by default. `align_vertical` places the padded
content block at the top, center, or bottom of the fixed content box. Like Lip
Gloss, centered content puts an odd extra row below the padded block: a
three-row gap is split as one row above and two below. Background color covers
both padding and every alignment row in the fixed content box.

## Maximum dimensions

`max_width` and `max_height` are hard limits on the final rendered block. In
contrast to fixed dimensions, they include padding, enabled border edges, and
margin. Like Lip Gloss, a zero maximum disables that constraint; use generic
`remove` when the property itself should be absent from the `TextStyle` value.

Rendering first wraps content only when `width` is present, then resolves fixed
width and height, alignment, padding, border, and margin. Maximum dimensions
are applied last: `max_width` crops every row without rewrapping, and
`max_height` keeps rows from the top. Consequently, a maximum wins when it is
smaller than a fixed dimension. Cropping never splits a grapheme cluster or a
wide character: a grapheme that would straddle the bound is dropped, and the
freed cells become blanks so the block stays rectangular at the cropped width.

Ratatui follows the same order. Its `Rect` remains an additional external clip;
the smaller of the explicit maximum and the available area is visible.

## Theme contract

Themes store complete logical values of both kinds. `Theme::style` and
`TextThemeRole::resolve` return an owned `TextStyle` for text roles; `Theme::style`
and `BlockThemeRole::resolve` return an owned `BlockStyle` for roles whose value
is a rectangle, such as `PanelRole` and the table's cell roles. An application
role of either kind builds its value with the ordinary consuming builders.
`ComponentStyles::with_style` replaces a text role's style,
`ComponentStyles::with_panel` and `with_panel_focused` replace the panel blocks,
and `Theme::components` exposes the stored built-in values as borrows for
consumers that want to avoid the copy.

Styles do not implicitly flow from a parent view to a child: as
[`view-model.md`](view-model.md) specifies, each child carries its own
complete value, so a theme role resolves to the whole style its position uses.

## Output-boundary contract

Renderers consume the values present in one `TextStyle`.

- ANSI rendering emits the active colors and modifiers.
- The `urushi-tui` adapter maps the active modifier set to Ratatui's
  `add_modifier`; it does not populate `sub_modifier`.
- Removing a modifier from an immutable `TextStyle` removes the value. It does not
  preserve an ANSI off-code instruction.
- A renderer that maintains prior terminal state is responsible for diffing
  previous and next effective styles and emitting any required reset codes.
- `TerminalProfile::resolve_text_style` maps or removes the effective text values;
  `TerminalProfile::resolve_block_style` does the same for a block's fill and
  border colors while preserving its geometry.

`TextStyle::paint` and `BlockStyle::render` surround emitted styling with a final
ANSI reset, so neither requires a stored removal instruction. Both take plain
text: the layout pass never inspects text for escape sequences, so
already-rendered output is adopted as a `RenderedBlock` instead of being fed
back in.

## Border edge geometry

An enabled top or bottom edge contributes one row. An enabled left or right
edge contributes one column. A corner glyph represents the intersection of
two enabled incident edges, so it is drawn only when both those edges are
enabled. For example, a top edge without a left edge starts with the top
horizontal glyph rather than the top-left corner. The horizontal glyph repeats
across the padded content width, and every emitted row has the same outer width:
the padded content width plus the enabled vertical-edge columns.

A border with all four sides disabled contributes no rows or columns and is
layout-equivalent to no border. Border foreground and background colors apply
uniformly to every enabled edge. The direct ANSI renderer and the Ratatui
widget use this same geometry, because both consume the same resolved
rectangle. A `Rect` smaller than the block clips it at the area's right and
bottom boundaries; it does not lay the box out again inside the smaller area,
so a trailing border edge outside the area is cropped rather than pulled
inwards.

## Required verification

Changes to this model must test:

- generic `add` and named builders produce the same value on both types;
- a geometry property cannot be attached to a `TextStyle`, checked at compile time;
- singleton properties replace their prior value;
- modifier sets union and subtract correctly;
- removing every property restores the documented default;
- every property enum variant is handled by ANSI and Ratatui boundaries where
  applicable;
- theme replacement remains unchanged; and
- truecolor, ANSI-256, ANSI-16, monochrome, disabled ANSI, CJK width, and box
  rendering regressions remain covered.
