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
zero spacing, automatic width and height, no minimum or maximum bounds, wrap
overflow, left horizontal alignment, or top vertical alignment.

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
a text property directly and `BlockStyle::foreground` reads the same as
`TextStyle::foreground`. There is no conversion in the other direction:
geometry cannot reach a `TextStyle`.

The enums make the complete property vocabulary discoverable and give generic
code an exhaustive match. Both types may store the values in typed fields rather
than allocating an enum collection; all mutation still passes through the
closed `add` and `remove` operations.

## The underline is one value

An underline carries a shape and a color, and both live in a single property:

```rust
enum UnderlineStyle { Single, Double, Curly, Dotted, Dashed }

struct Underline {
    style: UnderlineStyle,
    color: Option<Color>,   // None: drawn in the foreground color
}
```

`TextStyle::underline` is `Option<Underline>`; `None` is no underline. There is
no `Modifier::UNDERLINED` flag and no free-standing underline color property,
because either would give one appearance two values — SGR `4` and SGR `4:1` are
the same single underline, and an underline color paints nothing on a run with
no underline. `design/style-value-model.md` records why that matters.

```rust
TextStyle::new().underline();                              // single, foreground color
TextStyle::new().underline_style(UnderlineStyle::Curly);   // keeps any color already set
TextStyle::new().underline_color(Color::RED);              // adds a single underline if absent
```

No builder produces a color nothing draws. `underline_value` returns the whole
`Option<Underline>` and `remove(TextStylePropertyKey::Underline)` removes the
underline and its color together. `BlockStyle` mirrors all of these for its fill
text.

SGR 6 (rapid blink) is deliberately absent: xterm-family terminals draw it
exactly as SGR 5, so it would be distinguishable as a value and
indistinguishable on screen.

`TextStyle::canonical` folds the one duplication the type cannot rule out — an
underline color equal to the foreground — under the rule
[`inline-prompt-rendering.md`](inline-prompt-rendering.md) defines. Call it once
the style is final: the builders are immutable, so an earlier fold is undone by
the next call that changes the foreground. `TerminalProfile` calls it at its
last step, after degradation, because degradation is what makes two logical
colors equal.

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

## Dimensions

The `Length` vocabulary, the clamp, the distribution across siblings, and the
degenerate cases are defined in [`view-model.md`](view-model.md)'s Sizing
section; this document states what the properties mean as style values.

All six sizing properties — `width`, `height`, `min_width`, `min_height`,
`max_width`, `max_height` — describe the box the terminal shows: content plus
padding plus enabled border edges. Margin lies outside. `height(Cells(3))`
with a top and bottom border is three rows tall, one of them content. The
absence of `width` or `height` means auto: the intrinsic size of the content.
`u16` converts into `Length::Cells`, so `width(20)` reads as before.

A dimension is a preferred size, resolved under the available area: shorter
content is padded out to it, longer content is absorbed by the overflow rule
below, and the frame closes at the resolved size either way.

Conversions between the outer box and the content area inside it go through
one query: `frame_size()` returns the per-axis overhead of enabled border
edges plus padding. (Margin lies outside the box and keeps its own getter.)
Outer to inner — how many rows fit a panel of a given height — is
subtraction; inner to outer is rarely arithmetic at all: an unsized box
already takes its content's size plus frame, and an exact content dimension
is expressed structurally with an inner unframed block, as
[`view-model.md`](view-model.md) shows. The query exists for the numbers that
leave the style system: matching another box's size, or computing an
application's layout breakpoints.

Shorter content is top-aligned by default. `align_vertical` places the padded
content block at the top, center, or bottom of the resolved content box. Like
Lip Gloss, centered content puts an odd extra row below the padded block: a
three-row gap is split as one row above and two below. Background color covers
both padding and every alignment row.

## Minimum and maximum dimensions

`min_width`, `min_height`, `max_width`, and `max_height` are bounds in cells
on the same box the dimensions measure. They participate in layout: a box
shrinks to fit its content down to its minimum and never exceeds its maximum.
A maximum without a dimension gives shrink-to-fit sizing with a cap and no
padding-out; a minimum states the size below which the application's layout
stops making sense, which also fixes the box's floor when siblings compete
for a too-small area.

Below every explicit minimum lies the implicit one, on the width axis: the
layout pass never splits a grapheme cluster or a wide character, so a box
never resolves narrower than its widest unsplittable token without entering
the degenerate rules. The height axis has no counterpart — a row is either
drawn or not — so a box's implicit height floor is its frame, and a height
that leaves no content row is a closed frame around nothing. Bounds are
absent by default; use generic `remove` to delete one, not a zero value.

## Overflow

Content that cannot fit the resolved box is absorbed by the content, under a
policy the application chooses per block:

```rust
pub enum Overflow {
    Wrap,                     // reflow to the content width — the default
    Clip(Cow<'static, str>),  // cut inside the frame, ending the line with a marker
}
```

Marking a cut is a property of the cut, so there is one clipping policy
carrying the marker it ends with: `Overflow::clip()` cuts silently,
`Overflow::ellipsis()` is `Clip("…")`, and `Overflow::clip_with("...")` states
the marker an ASCII-only terminal can show — the same choice
`Border::ASCII` answers for box glyphs. Which glyphs a terminal can
render is the application's knowledge, so the library fixes no marker.

The marker is measured in cells like any other content: the text keeps the
content width less the marker's display width, and a marker that leaves no
room for content at all is dropped rather than shown alone.

`overflow` governs the width axis; height always clips inside the frame. The
order the two axes resolve in — width before the content is laid out, height
after — is defined in [`view-model.md`](view-model.md). No policy opens the
frame: a border is never cut by sizing, only by the
degenerate safety net. Cutting an already-rendered string at a column is a
text-layer utility, not a style property.

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

- ANSI rendering emits the active colors, modifiers, and underline, in SGR
  parameter order so that one style always spells one sequence. A single
  underline is spelled `4` rather than the equivalent `4:1`, which a terminal
  that does not parse subparameters still understands. An absent underline
  color emits nothing rather than SGR 59: `Color` has no reset spelling, and
  the reset closing every painted scope already restores the default.
- The `urushi-tui` adapter maps the active modifier set to Ratatui's
  `add_modifier`; it does not populate `sub_modifier`. Ratatui has no underline
  shape, so every underline degrades to `Modifier::UNDERLINED` there and the
  underline color is dropped — reaching it would require the `underline-color`
  feature, which pulls in a backend the adapter does not depend on.
- Removing a modifier from an immutable `TextStyle` removes the value. It does not
  preserve an ANSI off-code instruction.
- A renderer that maintains prior terminal state is responsible for diffing
  previous and next effective styles and emitting any required reset codes.
- `TerminalProfile::resolve_text_style` maps or removes the effective text values;
  `TerminalProfile::resolve_block_style` does the same for a block's fill and
  border colors while preserving its geometry. An underline survives a
  colorless profile — it is a shape — while its color degrades with the
  foreground and background. Both return a canonical style.

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
rectangle. A `Rect` smaller than the block is an `Available` bound the box
resolves under, so the frame closes inside the area; only the degenerate
safety net — an area the frame itself cannot fit — ever crops an edge.

## Required verification

Changes to this model must test:

- generic `add` and named builders produce the same value on both types;
- a geometry property cannot be attached to a `TextStyle`, checked at compile time;
- singleton properties replace their prior value;
- modifier sets union and subtract correctly;
- each underline shape and color emits its own SGR parameters, degradation maps
  and drops the underline color with the others, and `canonical` folds an
  underline color equal to the foreground and nothing else;
- removing every property restores the documented default;
- every property enum variant is handled by ANSI and Ratatui boundaries where
  applicable;
- theme replacement remains unchanged; and
- truecolor, ANSI-256, ANSI-16, monochrome, disabled ANSI, CJK width, and box
  rendering regressions remain covered.
