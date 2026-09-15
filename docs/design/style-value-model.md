# Styles as Effective Values

Urushi's style contract, defined in [`style-model.md`](../style-model.md),
shapes a style as an immutable collection of typed effective presentation
values, rather than a patch, an instruction list, or a completed ANSI state
transition. This file holds the value shape and the reasons for its public API.
Two members of the vocabulary have files of their own:
the underline value ([`underline.md`](underline.md)) and the canonical fold that
closes the one duplication the vocabulary cannot
([`style-canonical-form.md`](style-canonical-form.md)).

## The value shape

`TextStyle` stores foreground, background, underline, and hyperlink as
independent optional typed values and stores modifiers as one set. `BlockStyle`
stores one `TextStyle` for its fill plus the geometry of a rectangle:

```rust
struct BlockStyle {
    text_style: TextStyle,
    padding: Sides,
    margin: Sides,
    border: Option<Border>,
    border_top: bool,
    border_right: bool,
    border_bottom: bool,
    border_left: bool,
    border_text_style: TextStyle,
    border_foreground: Option<Color>,
    border_background: Option<Color>,
    width: Option<Length>,
    height: Option<Length>,
    min_width: Option<u16>,
    min_height: Option<u16>,
    max_width: Option<u16>,
    max_height: Option<u16>,
    overflow: Overflow,
    align: Align,
    vertical_align: VerticalAlign,
}
```

`GridStyle` stores only the optional length claimed by each column and the
default cell padding. A grid carries no box geometry, fill style, or line
network — a grid that needs a border, a margin, or a stated size is placed
inside a block, while a presentation that draws internal rules uses Canvas.

Each immutable builder updates the corresponding typed field in the returned
value. There is no separate property value that exists only long enough to be
matched and discarded.

## Why effective values, not instructions

A style whose entries are effective values means one thing wherever it is read:
a renderer consumes the values present and emits whatever its backend needs to
realize them. A style whose entries are instructions — "turn bold off" — only
means something relative to a prior state, so every reader must agree on what
that state is.

Removing a modifier from an immutable value therefore removes the value; it
does not preserve an ANSI off-code instruction. A renderer that maintains prior
terminal state diffs the previous and next effective styles itself.
ANSI rendering surrounds emitted styling with a final reset. No stored removal
instruction is needed in an immutable style value.

## Why only named operations are public

A public property enum duplicated the builder vocabulary without representing
stored state. A caller constructing an ordinary style had to choose between
the named `foreground(Color::CYAN)` operation and wrapping the same value for
generic `add`, even though both immediately wrote the same field. The enum also
invited exhaustive matching, so adding a property to a pre-alpha style
unnecessarily broke generic consumer code.

The public API therefore exposes named operations rather than a second generic
property vocabulary. Their setter, reset, getter, set-operation, and convenience
names follow the single rule recorded in
[`style-api-naming.md`](style-api-naming.md).

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
