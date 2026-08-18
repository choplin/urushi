# Styles as Effective Values

Urushi's style contract, defined in [`style-model.md`](../style-model.md),
shapes a style as an immutable collection of effective presentation values with
a closed property vocabulary, rather than a patch, an instruction list, or a
completed ANSI state transition. This file holds the full vocabulary and the
reasons for that shape. Two members of the vocabulary have files of their own:
the underline value ([`underline.md`](underline.md)) and the canonical fold that
closes the one duplication the vocabulary cannot
([`style-canonical-form.md`](style-canonical-form.md)).

## The vocabulary

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

Both types may store the values in typed fields rather than allocating an enum
collection; all mutation still passes through the closed `add` and `remove`
operations.

## Why effective values, not instructions

A style whose entries are effective values means one thing wherever it is read:
a renderer consumes the values present and emits whatever its backend needs to
realize them. A style whose entries are instructions — "turn bold off" — only
means something relative to a prior state, so every reader must agree on what
that state is.

Removing a modifier from an immutable value therefore removes the value; it
does not preserve an ANSI off-code instruction. A renderer that maintains prior
terminal state diffs the previous and next effective styles itself.
`TextStyle::paint` and `BlockStyle::render` surround emitted styling with a
final ANSI reset. Between the two, no stored removal instruction is needed
anywhere.

## Why a closed vocabulary and one generic `remove`

Urushi exposes a closed enum pair per type and one generic `remove`, while
retaining named builders for common construction. Lip Gloss, which likewise
treats a style as an immutable value containing a set of rules, instead tracks
property presence separately and exposes many property-specific `Unset*`
methods. The enums make the complete property vocabulary discoverable and give
generic code an exhaustive match, and one
`remove` cannot drift from a parallel family of `Unset*` methods. The named
builders are thin wrappers over `add` for the same reason: Urushi avoids two
entry points that define two behaviors.

## Why rapid blink is not in the vocabulary

The property vocabulary is not merely a list of what a terminal can express; it
decides whether two styles with the same appearance are the same value. A run's
style is the unit a redraw compares, so a vocabulary admitting two spellings of
one appearance makes every frame redraw rows that did not change.

SGR parameter 6, rapid blink, is left out for that reason. Xterm-family
terminals draw it exactly as SGR 5, so it would be distinguishable as a value
and indistinguishable on screen — one appearance with two values, admitted at
the vocabulary level. The underline is where the same test bites twice, and
[`underline.md`](underline.md) records how it is closed; what no single field
can close is left to the canonical fold of
[`style-canonical-form.md`](style-canonical-form.md).

## Why no patch operation

Urushi has no patch operation, and modifiers are one set rather than an
add set and a subtract set. Reusable changes are ordinary
`fn(TextStyle) -> TextStyle` transforms, and no Urushi boundary requires a
separately inspectable patch value. A patch data type would be introduced only
for a concrete need to serialize or inspect such changes.

The absence of a patch representation is also what keeps parent-to-child style
inheritance out of the view model, as
[`view-block-model.md`](view-block-model.md) records.
