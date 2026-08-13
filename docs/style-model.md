# Style Value Model

This document defines Urushi's logical `Style` contract.

## Decision

`Style` is an immutable collection of effective presentation values. It is not
a patch, an instruction list, or a completed ANSI state transition.

Calling a builder consumes a `Style` and returns a new value. The original
style remains unchanged:

```rust
let base = Style::new().foreground(Color::CYAN);
let focused = base.clone().bold();
```

Each singleton property has at most one value. Adding another value of the
same kind replaces it. Modifiers are a set, so they are unioned and subtracted
individually.

```rust
let style = Style::new()
    .foreground(Color::CYAN)
    .add(Modifier::BOLD | Modifier::ITALIC)
    .remove(Modifier::ITALIC);
```

`Style::new()` and `Style::default()` are empty styles. Removing a property
restores its ordinary default in the resulting value: no color, no border,
zero spacing, automatic width, or left alignment.

## Closed property vocabulary

The generic API uses two closed enums:

```rust
enum StyleProperty {
    Foreground(Color),
    Background(Color),
    Modifier(Modifier),
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
    Align(Align),
}

enum StylePropertyKey {
    Foreground,
    Background,
    Modifier(Modifier),
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
    Align,
}
```

The enums make the complete property vocabulary discoverable and give generic
code an exhaustive match. `Style` may store the values in typed fields rather
than allocating an enum collection; all mutation still passes through the
closed `add` and `remove` operations.

## Public API

`add` and `remove` are the complete generic operations:

```rust
let style = Style::new()
    .add(StyleProperty::Foreground(Color::CYAN))
    .add(Modifier::BOLD)
    .remove(StylePropertyKey::Foreground)
    .remove(Modifier::BOLD);
```

Common properties also have concise named builders for completion and
readability:

```rust
let style = Style::new()
    .foreground(Color::CYAN)
    .bold()
    .padding((0, 1))
    .border(Border::ROUNDED)
    .width(20)
    .align(Align::Center);
```

Border edge visibility has named builders and getters as well as generic
properties:

```rust
let separator = Style::new()
    .border(Border::NORMAL)
    .border_top(false)
    .border_right(false)
    .border_bottom(true)
    .border_left(false);

assert!(separator.is_border_bottom_enabled());
```

`border(Border)` enables the familiar four-sided rendering by default. Each
side setting is an independent effective value. Removing a side property, such
as `StylePropertyKey::BorderLeft`, restores its default value of `true`.
Removing `Border` removes only the glyph set; it does not rewrite the four side
values, which remain inactive until a border is added again.

These builders are thin wrappers over `add`; they do not define a second
behavior. There are no `unset_*` methods. Generic `remove` is the single way to
delete a property.

## Composition

`Style` has no `patch` or `inherit` operation. Deriving one style from another
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
fn focused(style: Style, accent: Color) -> Style {
    style.foreground(accent).bold()
}
```

If an application needs several transforms, it can fold functions over a base
style. Urushi does not introduce a separate patch data type without a concrete
need to serialize or inspect such changes.

## Theme contract

Themes store complete logical `Style` values. `Theme::style` and
`ThemeRole::resolve` return `&Style`. `ComponentStyles::with_style` replaces a
role's style. Applications derive a value by cloning it and applying builders
or a normal transform function.

Box properties do not implicitly flow from a parent view to a child. If a
future hierarchical view needs CSS-like inheritance, that resolver will need
an explicit contract for which effective values flow. It must not silently
turn the general `Style` value into a patch.

## Output-boundary contract

Renderers consume the values present in one `Style`.

- ANSI rendering emits the active colors and modifiers.
- The `urushi-tui` adapter maps the active modifier set to Ratatui's
  `add_modifier`; it does not populate `sub_modifier`.
- Removing a modifier from an immutable `Style` removes the value. It does not
  preserve an ANSI off-code instruction.
- A renderer that maintains prior terminal state is responsible for diffing
  previous and next effective styles and emitting any required reset codes.
- `TerminalProfile::resolve_style` maps or removes effective values while
  preserving box properties.

Direct `Style::render` surrounds emitted styling with a final ANSI reset, so it
does not require a stored removal instruction.

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
widget use this same geometry; constrained Ratatui areas clip at the area's
right and bottom boundaries after reserving enabled edge cells.

## Alignment with Lip Gloss and noctui

Like Lip Gloss, Urushi treats `Style` as an immutable value containing a set of
rules. Lip Gloss tracks property presence separately and exposes many
property-specific `Unset*` methods. Urushi instead exposes a closed enum and
one generic `remove`, while retaining named builders for common construction.

This deliberately differs from noctui's incremental add/sub modifier sets and
`patch` operation. Urushi does not adopt that representation because reusable
changes can be ordinary `fn(Style) -> Style` transforms and no current Urushi
boundary requires a separately inspectable patch value.

## Required verification

Changes to this model must test:

- generic `add` and named builders produce the same value;
- singleton properties replace their prior value;
- modifier sets union and subtract correctly;
- removing every property restores the documented default;
- every property enum variant is handled by ANSI and Ratatui boundaries where
  applicable;
- theme replacement remains unchanged; and
- truecolor, ANSI-256, ANSI-16, monochrome, disabled ANSI, CJK width, and box
  rendering regressions remain covered.
