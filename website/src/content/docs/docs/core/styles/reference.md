---
title: Styles reference
description: Complete reference for terminal-cell color, attributes, underlines, and hyperlinks.
---

This page is the task-oriented index to the style API. The generated Rust API
reference supplies every signature, generic bound, trait implementation, and
source link:

- [Complete `urushi` API](https://docs.rs/urushi/latest/urushi/index.html)
- [`TextStyle`](https://docs.rs/urushi/latest/urushi/struct.TextStyle.html),
  [`Color`](https://docs.rs/urushi/latest/urushi/enum.Color.html), and
  [`Hyperlink`](https://docs.rs/urushi/latest/urushi/struct.Hyperlink.html)

Box geometry, dimensions, alignment, borders, and overflow are indexed in the
[Layout reference](/docs/core/views/reference/).

## TextStyle

Appearance builders: `foreground`, `background`, `add_attribute`,
`add_attributes`, `remove_attribute`, `remove_attributes`, `bold`, `dim`,
`italic`, `underlined`, `underline_style`, `underline_color`, `underline`,
`hyperlink`, `blink`, `reverse`, `hide`, and `strikethrough`.

Explicit reset builders: `reset_foreground`, `reset_background`,
`reset_attributes`, `reset_underline`, and `reset_hyperlink`.

Accessors: `get_foreground`, `get_background`, `get_underline`,
`get_hyperlink`, and `get_attributes`.

### Color and attributes

`Color` is `Ansi(0..=15)`, `Ansi256(u8)`, or `Rgb(r, g, b)`. Constants cover
the eight standard and eight bright ANSI colors. `Color::parse` accepts
`#rgb`, `#rrggbb`, or a decimal palette index; `u8` and RGB tuples convert into
Color.

`TextAttribute` variants are bold, dim, italic, slow/rapid blink, reversed,
hidden, crossed out, Fraktur, framed, encircled, and overlined.
`TextAttributes` supports `empty`, `all`, `from_attribute`, set union,
difference, intersection, containment checks, `is_empty`, and iteration.

`UnderlineStyle` is single, double, curly, dotted, or dashed. `Underline`
supports `new`, style/color builders and resets, plus style/color getters.

`UnderlineStyleSet` is the capability set used by `RenderSettings`. Constants
are `SINGLE`, `DOUBLE`, `CURLY`, `DOTTED`, and `DASHED`; construct and inspect
sets with `empty`, `all`, `union`, `contains`, or the `|` operator. It describes
which underline shapes an output can emit, not the one shape requested by a
`TextStyle`.

## BlockStyle cell appearance

`BlockStyle` uses a complete `TextStyle` for styled cells created inside the
block, including padding and alignment fill. Margin remains unstyled, border
cells have their own style, and appearance does not cascade into the child
View.

Construction and inspection: `BlockStyle::new`, `from_text_style`, and
`text_style`.

Appearance builders shared with `TextStyle`: `foreground`, `background`,
`add_attribute`, `add_attributes`, `remove_attribute`, `remove_attributes`,
`bold`, `dim`, `italic`, `underlined`, `underline_style`, `underline_color`,
`underline`, `hyperlink`, `blink`, `reverse`, `hide`, and `strikethrough`.

Appearance resets: `reset_foreground`, `reset_background`,
`reset_attributes`, `reset_underline`, and `reset_hyperlink`.

Appearance accessors: `get_foreground`, `get_background`, `get_attributes`,
and `get_underline`. Use `text_style().get_hyperlink()` to inspect the link.

Border cells have a separate complete style through `border_text_style`,
`border_foreground`, and `border_background`, with matching reset and getter
methods: `reset_border_text_style`, `reset_border_foreground`,
`reset_border_background`, `get_border_text_style`, `get_border_foreground`,
and `get_border_background`. See the
[View and layout reference](/docs/core/views/reference/#block-geometry) for
margin, padding, border edges, dimensions, alignment, overflow, and
`frame_size`.

## Hyperlink

| API | Result |
|---|---|
| `Hyperlink::new(uri)` | A link without parameters |
| `parameter(name, value)` | The link with one appended parameter |
| `uri()` | The encoded URI emitted by the ANSI renderer |
| `parameters()` | Encoded parameters in insertion order |

`TextStyle::hyperlink` accepts `impl Into<Hyperlink>`; `&str` and `String`
construct a link without parameters. `BlockStyle` exposes the same appearance
builder for cells created by a block. See
[Configure text appearance](/docs/core/styles/text/#add-an-osc-8-hyperlink) for
terminal behavior, parameters, and the input-safety contract.
