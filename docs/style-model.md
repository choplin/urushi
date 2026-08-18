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
    .add(Modifier::BOLD | Modifier::ITALIC)
    .remove(Modifier::ITALIC);
```

`TextStyle::new()` and `TextStyle::default()` are empty styles.

Removing a property restores its ordinary default in the resulting value;
the defaults are listed under [Property defaults](#property-defaults).

## Closed property vocabulary

The generic API uses one closed enum pair per type. The text vocabulary:

```rust
enum TextStyleProperty {
    Foreground(Color),
    Background(Color),
    Underline(Underline),
    Modifier(Modifier),
}

enum TextStylePropertyKey {
    Foreground,
    Background,
    Underline,
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
    Width(Length),
    Height(Length),
    MinWidth(u16),
    MinHeight(u16),
    MaxWidth(u16),
    MaxHeight(u16),
    Overflow(Overflow),
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
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Overflow,
    Align,
    VerticalAlign,
}
```

`TextStyleProperty` converts into `BlockStyleProperty`, so `BlockStyle::add` accepts
a text property directly, and `BlockStyle::foreground` reads the same as
`TextStyle::foreground`. There is no conversion in the other direction:
geometry cannot reach a `TextStyle`.

Both types may store the values in typed fields rather than allocating an enum
collection; all mutation still passes through the closed `add` and `remove`
operations.

Within that vocabulary, the Select Graphic Rendition (SGR) parameter 6 (rapid
blink) is deliberately absent. Why the vocabulary is closed, and why rapid
blink is not in it, is recorded in
[`design/style-value-model.md`](design/style-value-model.md).

## Underline shape and color

An underline carries a shape and a color, and both live in a single property:

```rust
enum UnderlineStyle { Single, Double, Curly, Dotted, Dashed }

struct Underline {
    style: UnderlineStyle,
    color: Option<Color>,   // None: drawn in the foreground color
}
```

`TextStyle::underline` is `Option<Underline>`; `None` is no underline. There is
no `Modifier::UNDERLINED` flag and no free-standing underline color property;
[`design/style-value-model.md`](design/style-value-model.md) records why one
appearance must not have two values.

```rust
TextStyle::new().underline();                              // single, foreground color
TextStyle::new().underline_style(UnderlineStyle::Curly);   // keeps any color already set
TextStyle::new().underline_color(Color::RED);              // adds a single underline if absent
```

No builder produces a color nothing draws. `underline_value` returns the whole
`Option<Underline>`, and `remove(TextStylePropertyKey::Underline)` removes the
underline and its color together. `BlockStyle` mirrors all of these for its fill
text.

## Canonical form

The vocabulary above is chosen so that one appearance has one value, but one
duplication survives it: an underline color equal to the foreground. It is a
duplication *between* fields, so no signature makes it unrepresentable, and it
is closed by normalization, applied to both style types alike:

> Fold an underline color to *absent* when the foreground is a concrete color
> and the underline color is **the same value**. The values are compared as
> values: a palette red and a true-color red look different on screen and must
> not be folded together.

An underline is drawn in the foreground color unless one is set, so stating the
color a run already has changes nothing but the bytes.

The fold applies once the style is final. `TerminalProfile::resolve_text_style`
and `resolve_block_style` apply it as their last step, after degradation,
because degradation is what makes two logical colors equal; a style that has
passed through a profile is therefore canonical, and no separate normalizing
call is part of the public API.

The rule for admitting any future fold is narrow:

> **Fold only what is inert.** A value may be dropped when doing so cannot
> change the output, whatever the terminal does. An equivalence that holds only
> because a terminal is assumed to implement an attribute a particular way is
> not a fold.

One case is not closable: when the foreground is absent, its concrete color is
the terminal's default and unknown here, so an underline color equal to it
cannot be recognized. A canonical form owes determinism, not minimality.

Why the fold happens only once the style is final, why the admission rule is
phrased around inertness rather than appearance, and what the residue costs are
recorded in [`design/style-value-model.md`](design/style-value-model.md).
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) defines the
canonical form of the styled runs a prompt frame emits, which rests on this
rule.

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
    .width(20)                     // Length::Cells(20) through From<u16>
    .height(Length::Fill(1))
    .min_width(12)
    .max_width(60)
    .overflow(Overflow::ellipsis())
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

`border(Border)` sets the glyph set and nothing else. Which sides are drawn is
four independent effective values, all `true` by default, so a style that has
never touched them draws all four sides once a glyph set is present; calling
`border` does not rewrite them, and `border_left(false).border(Border::ROUNDED)`
still has no left edge. Removing a side property, such as
`BlockStylePropertyKey::BorderLeft`, restores its default value of `true`.
Removing `Border` removes only the glyph set; it does not rewrite the four side
values, which remain inactive until a border is added again.

These builders are thin wrappers over `add`; they do not define a second
behavior. Generic `remove` is the single way to delete a property; there are no
`unset_*` methods.

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

## Dimensions

There are six sizing properties: `width` and `height` take a `Length`;
`min_width`, `min_height`, `max_width`, and `max_height` take cells. This
document states what they mean as style values. The `Length` vocabulary, the
box they measure, what their absence means, the clamp between them, the
distribution across siblings, and the degenerate rules are defined in
[`view-model.md`](view-model.md), under "Bounds, sharing, and overflow" and
"How a node resolves".

A dimension is a preferred size, resolved under the available area: shorter
content is padded out to it, longer content is absorbed by the overflow rule
below, and the frame closes at the resolved size either way.

Conversions between the outer box and the content area inside it go through
one query: `frame_size()` returns the per-axis overhead of enabled border
edges plus padding. (Margin lies outside the box and keeps its own getter.)
[`view-model.md`](view-model.md) shows what it converts and how an exact
content dimension is expressed instead.

Resolving a dimension can also leave spare rows. Shorter content is
top-aligned by default. `align_vertical` places the padded content block at the
top, center, or bottom of the resolved content box. As in the Lip Gloss layout
library, centered content puts an odd extra row below the padded block: a
three-row gap is split as one row above and two below.

Background color covers both padding and every alignment row.

## Minimum and maximum bounds

A minimum states the size below which the application's layout stops making
sense. How the bounds participate in the clamp, and the implicit floors below
an explicit minimum, are defined in [`view-model.md`](view-model.md), under
"Bounds, sharing, and overflow".

Bounds are absent by default; use generic `remove` to delete one, not a zero
value.

## Overflow

The overflow policy absorbs content that cannot fit the resolved box; the
application chooses that policy per block:

```rust
pub enum Overflow {
    Wrap,                     // reflow to the content width — the default
    Clip(Cow<'static, str>),  // cut inside the frame, ending the line with a marker
}
```

There are three constructors: `Overflow::clip()` cuts silently,
`Overflow::ellipsis()` is `Clip("…")`, and `Overflow::clip_with("...")` states
the marker an ASCII-only terminal can show — the same choice `Border::ASCII`
answers for box glyphs. Which glyphs a terminal can render is the
application's knowledge, so the library fixes no marker.

Which axis the policy governs, what the marker costs, and how a cut relates to
the frame are defined in [`view-model.md`](view-model.md), under "Bounds,
sharing, and overflow" and "How a node resolves".

Cutting an already-rendered string at a column is a text-layer utility, not a
style property.

## Property defaults

Removing a property restores its ordinary default in the resulting value: no
color, no border, zero spacing, automatic width and height, no minimum or
maximum bounds, wrap overflow, left horizontal alignment, or top vertical
alignment.

## Theme contract

Themes store complete logical values of both kinds. `Theme::text_style` and
`TextThemeRole::resolve` return an owned `TextStyle` for text roles;
`Theme::block_style` and `BlockThemeRole::resolve` return an owned `BlockStyle`
for roles whose value is a rectangle, such as `PanelRole` and the cell roles of
the built-in table component. An application role of either kind builds its
value with the ordinary consuming builders.

`ComponentStyles::with_text_style` replaces a text role's style,
`ComponentStyles::with_panel` and `with_panel_focused` replace the panel
blocks, and `Theme::components` exposes the stored built-in values as borrows
for consumers that want to avoid the copy.

Styles do not implicitly flow from a parent view to a child: as
[`view-model.md`](view-model.md) specifies, each child carries its own
complete value, so a theme role resolves to the whole style its position uses.

## Output-boundary contract

Renderers consume the values present in one `TextStyle`.

- ANSI rendering emits the active colors, modifiers, and underline, in SGR
  parameter order so that one style always spells one sequence. A single
  underline is spelled `4` rather than the equivalent `4:1`, which a terminal
  that does not parse subparameters still understands; the other shapes are
  spelled `4:2` to `4:5`, and a terminal that does not parse subparameters
  draws them as a single underline or not at all. No profile degrades a shape:
  which shapes a terminal renders is the application's knowledge, like which
  border glyphs and clip markers it renders. An absent underline
  color emits nothing rather than SGR 59: `Color` has no reset spelling, and
  the reset closing every painted scope already restores the default.
- The `urushi-tui` adapter maps the active modifier set to Ratatui's
  `add_modifier`; it does not populate `sub_modifier`. Ratatui has no underline
  shape, so every underline degrades to Ratatui's `Modifier::UNDERLINED` there,
  and the underline color is dropped — reaching it would require the
  `underline-color` feature, which pulls in a backend the adapter does not
  depend on.
- `TerminalProfile::resolve_text_style` maps or removes the effective text values;
  `TerminalProfile::resolve_block_style` does the same for a block's fill and
  border colors while preserving its geometry. An underline survives a
  colorless profile — it is a shape — while its color degrades with the
  foreground and background. Both return a canonical style.

Removing a modifier from an immutable `TextStyle` removes the value; a renderer
that maintains prior terminal state is responsible for diffing previous and
next effective styles and emitting any required reset codes.
[`design/style-value-model.md`](design/style-value-model.md) records why a
style holds no removal instruction.

`TextStyle::paint` and `BlockStyle::render` surround emitted styling with a
final ANSI reset. Both take plain text: the layout pass never inspects text for escape sequences, so
already-rendered output is adopted as a `RenderedBlock` instead of being fed
back in.

## Border edge geometry

Each enabled edge contributes to the box as follows:

- An enabled top or bottom edge contributes one row.
- An enabled left or right edge contributes one column.
- A corner glyph represents the intersection of two enabled incident edges, so
  it is drawn only when both those edges are enabled.
- The horizontal glyph repeats across the padded content width, and every
  emitted row has the same outer width: the padded content width plus the
  enabled vertical-edge columns.

For example, a top edge without a left edge starts with the top horizontal
glyph rather than the top-left corner.

A border with all four sides disabled contributes no rows or columns and is
layout-equivalent to no border. Border foreground and background colors apply
uniformly to every enabled edge. The direct ANSI renderer and the Ratatui
widget use this same geometry, because both consume the same resolved
rectangle. A Ratatui `Rect` smaller than the block is an `Available` bound (see
[`view-model.md`](view-model.md)) the box resolves under, so the frame closes
inside the area; only the degenerate rules ever crop an edge.
