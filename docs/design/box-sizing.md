# Box Sizing

How large a box is: the `Length` vocabulary, the box every sizing property
measures, the clamp that decides a used size, the floors below it, and what
happens when even the floor does not fit. [`view-model.md`](../view-model.md)
summarizes this under "Sizing at a glance"; how siblings divide an area and
what happens to content that does not fit are separate topics —
[`area-sharing.md`](area-sharing.md) and [`overflow.md`](overflow.md) — and
the order these rules apply in per node is [`layout-resolution.md`](layout-resolution.md).

## The rule

Sizes are expressed in one vocabulary:

```rust
pub enum Length {
    Cells(u16),  // an absolute number of terminal cells
    Fill(u16),   // a weighted share of the remaining area
}
```

`width` and `height` take a `Length`; their absence means *auto* — the
intrinsic size. `u16` converts into `Length::Cells`, so `width(20)` stays
concise. `min_width`, `min_height`, `max_width`, and `max_height` are bounds
in cells. Every one of these measures the same box: content plus padding plus
enabled border edges. Margin lies outside. A bordered box "of width 6" is six
visible cells wide — `┌────┐`. Margin still occupies cells: what a container
hands a child, and what a `Fill` share or an `Available` bound is measured
against, is the box plus its margin — the extent the child occupies in its
parent — so a `width(20)` child with a margin of one on each side takes
twenty-two cells of its parent's area.

### Intrinsic sizes

With no bound on the area, a node takes its *intrinsic* size:

- A `Text` node's rectangle is its own lines, padded to the width of its widest
  line.
- `Row(align, children)`: width is the sum of child widths, height is the
  greatest child height. A shorter child is offset vertically by `align` and
  padded with blank rows of its own width.
- `Column(align, children)`: width is the greatest child width, height is the
  sum of child heights. A narrower child's rows are padded to the full width
  according to `align`.
- Padding introduced by composition carries the child's own fill style — a
  `Block`'s `BlockStyle::text`, so a background-colored block keeps its
  background across alignment rows — and an empty `TextStyle` for a `Text`
  child.

### The clamp

A box's used size is a clamp, per axis:

```
base = width if set (Cells directly; Fill resolved against the remaining area)
       else the intrinsic size
used = base
         capped by  min(max_width, available)
         floored by max(min_width, min-content)
```

*min-content* is the size below which the box cannot go without splitting a
grapheme: the widest unsplittable token for width. The two axes are not
symmetric here. A grapheme cannot be cut down the middle, so the width axis
has a content floor; a row can simply be absent, so the height axis has none,
and a box's height floor is its frame alone — a bordered `height(2)` box is
two border rows and no content. This is what makes `height` a size rather
than a minimum: content beyond it clips inside the frame. `measure` returns
the *max-content* size. When the floor exceeds the cap, the floor wins, and
the degenerate rules below apply.

The clamp only caps: an area wider than a node never widens it. A node
resolved below its available area keeps its own size and is placed by its
parent's alignment, which is what makes a resolved size mean what the content
needs rather than what it was offered. This holds for a text leaf as much as
for a box — a leaf takes its own lines, reflowed or cut where the area is
narrower than they are and unchanged where it is wider. `Fill` is the one
length that reads an area as a size to take, which is why a box containing one
spans its own extent.

A minimum states the size below which the application's layout stops making
sense; the implicit floor below it — the widest grapheme the box cannot
split — is not exposed as a query. Bounds are absent by default; use generic
`remove` to delete one, not a zero value.

There is no property that sizes the content box from inside a frame. A box
with no size takes its content's size plus frame; an exact content dimension
is written structurally, as a size on an unframed inner block, whose outer
size and content size coincide:

```rust
// "Wrap this text at 40 cells" inside a framed panel.
View::block(panel,                       // border and padding, width auto
    View::block(BlockStyle::new().width(40), text))
```

Conversions that leave the style system — matching another box's outer size or
computing an application's layout breakpoints — go through
`BlockStyle::frame_size`, defined in [`style-model.md`](../style-model.md).

Resolving a dimension can leave spare rows. Shorter content is top-aligned by
default; `align_vertical` places the padded content block at the top, center,
or bottom of the resolved content box. As in the Lip Gloss layout library,
centered content puts an odd extra row below the padded block: a three-row gap
is split as one row above and two below. Background color covers both padding
and every alignment row.

### Degenerate cases

When a box cannot reach even its floor, it degrades in order: margin
collapses first, then padding, then content. Only when the area cannot hold
the frame itself — two border columns in a width of one — does the final
safety net crop the assembled rectangle, grapheme-atomically, blank-filling a
dropped wide character's cells. This crop is the single way a frame is ever
cut. It is unreachable while the frame fits and the children's floors add up
to no more than the area; a container whose floors exceed its area resolves
larger than the area and meets this crop.

## Why the area is an input to layout

The previous model applied bounds after assembly: `max_width` and the outer
`Limits` truncated the finished rectangle row by row — Lip Gloss's
`applyBorder`-then-`Truncate` order, inherited with its artifacts. Two boxes
occupying the same six columns then meant different things:

```
width(4) + border      -> 6x4        max_width(6) + border  -> 6x3
  ┌────┐                               ┌─────
  │abcd│                               │abcde
  │ef  │                               └─────
  └────┘
```

The first assembles a content box and derives the frame from it; the second
cuts the assembled rectangle, severing the derivation that placed the border,
so the right edge vanishes. A border derives from the content box it
surrounds: any bound that arrives after the box is assembled can only cut the
frame open. Making `Available` an input — sizes flow down once, resolved sizes
flow up once — is what lets the frame close at whatever size the bound forces,
and it is the shape a full-screen surface needs anyway, where the terminal's
`Rect` is the primary fact of layout.

## Why every sizing property measures the outer box

Distribution and `Available` necessarily measure the box that sits in the
area — frame included — so a content-basis `width` would leave the vocabulary
measuring two different boxes, which is precisely the asymmetry the old
`width`/`max_width` pair suffered from, generalized. In a terminal the outer
box is also the visible one: a border occupies cells a reader counts, and
`┌────┐` is a six-cell box to anyone looking at it.

Even under that rule, the intent "size my content" does not need a property at
all: it is a size on an unframed inner block, where outer and content coincide,
so the tree disambiguates the two intents structurally and no precedence rule
between two width properties ever arises.

## Why `height` is a size and not a minimum

The previous rule let content taller than `height(n)` grow the box. It made
`height` unable to state "this tall, period", which a viewport needs, and it
treated the axes asymmetrically for no reason wrapping does not already cover.
Overflow absorbs the excess inside the frame, and growth-on-content is what
auto sizing is for.

## What this model borrows from CSS, and what it does not

CSS reached the same separation — sizing, track distribution, and overflow as
three vocabularies — and this model borrows those concepts, not their
implementation:

| Urushi | CSS | Note |
|---|---|---|
| `width(Cells)` / `min_` / `max_` | `width`, `min-width`, `max-width` (border-box) | margin outside, as in CSS |
| auto (absent `width`) | `max-content` sizing | |
| `Fill(n)` | Grid's `fr` track | named after Ratatui's `Constraint::Fill` |
| `Available` | the containing block | |
| `Overflow::Wrap`/`Clip(marker)` | `white-space`, `overflow`, `text-overflow` | one enum: a terminal has no paint layer to make `overflow` its own axis |
| min-content floor | `min-content` | grapheme atomicity, not word atomicity |
| — | `overflow: visible` | impossible: no paint layer; a resolved rectangle is the output |
| — | percentages | ratios are `Fill` weights; a percentage length is a possible `Length` extension |
| view-function branching | media queries | conditional structure stays outside layout |

## Rejected designs

- **Sizing by post-hoc crop** — the previous model, where `max_width` and
  `Limits` truncated the assembled rectangle. Argued under "Why the area is an
  input to layout"; the crop survives only as the degenerate safety net.
- **Content-box sizing, or a second `content_width` property.** Argued under
  "Why every sizing property measures the outer box".
- **Fixed height as a minimum.** Argued under "Why `height` is a size and not
  a minimum".
