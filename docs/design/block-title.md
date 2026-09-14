# Block Border Titles

A Block may carry one optional `BlockTitle`: a single `StyledText` line,
horizontal alignment, and a preferred symmetric horizontal padding. The title
occupies cells of the Block's top border. This document defines how that
content participates in layout and how it degrades when the edge is narrow.

## Ownership and construction

The title belongs to the Block node beside its `BlockStyle` and child. It is
content, not a style property: the text and its complete per-span styles vary
with the view snapshot, while `BlockStyle` remains a reusable geometry and
paint value. It does not introduce a semantic component or another `View`
variant.

`BlockTitle::new` accepts plain or styled text, rejects line breaks, and
defaults to left alignment with one preferred blank cell on each side.
`View::titled_block` is the ordinary constructor and accepts either a value
convertible to `BlockTitle` or a configured `BlockTitle`. Its anchor equivalent
is `View::titled_anchor_block`. The untitled constructors remain
`View::block` and `View::anchor_block`.

A titled Block requires a border with its top edge enabled. This is checked at
the titled construction boundary. A directly constructed invalid enum value is
a contract violation; debug layout asserts the same invariant, while release
layout ignores its title.

## Intrinsic size

An empty title has no effect. For a non-empty title, its outer width demand is:

```text
enabled left border + title text + 2 * preferred title padding + enabled right border
```

Text width is the sum of grapheme display widths. The Block's automatic width
uses the greater of its child-and-frame demand and its title demand, then adds
margin in the ordinary way. A title contributes no height: it replaces top
border cells, never creates another row, and never wraps.

The title demand is intrinsic, not a minimum. Explicit width, maximum width,
finite `Available`, and a finite allocation selected for `Fill` determine the
same outer box they do for an untitled Block. The title then adapts to that box;
it never forces the selected width wider. Block margin and content padding are
independent of title padding.

## Top-edge placement

The title's placement band is the top edge after excluding an enabled left
corner and enabled right corner. A disabled side border contributes no corner,
so the band extends to that outer edge. Border rows are assembled first, then
the title replaces cells in this band.

When the complete preferred slot fits, it consists of left padding, title
text, and right padding. The slot is aligned as one unit within the band:

- left alignment leaves all spare border cells after the slot;
- right alignment leaves them before the slot;
- center alignment puts an odd spare cell after the slot.

Title graphemes retain their complete `TextStyle`. Padding cells retain the
Block border style; they are not styled by neighboring title spans.

## Narrow-edge degradation

The two corner cells, when enabled, have priority over every title cell. The
remaining band degrades in this order:

1. keep as much title text as fits, clipping only between grapheme clusters;
2. allocate remaining cells to preferred padding;
3. align the surviving text-and-padding slot within the band.

Padding allocation preserves the aligned edge first: left alignment gives a
cell to left padding before right padding, right alignment does the reverse,
and center alignment divides available padding with an odd cell on the right.
If no complete title grapheme fits, the original border remains unchanged.
The ordinary final safety crop may still cut the Block itself when even its
frame cannot fit the external area.

## Rejected designs

- **A `BlockStyle` title property.** The changing text snapshot and its styled
  spans would become part of a reusable style value and theme role.
- **A `View::TitledBlock` variant.** Titled and untitled boxes have identical
  layout identity; a second variant would duplicate every Block branch and
  make title presence look like another primitive operation.
- **A title presentation or component.** A border annotation is generic Block
  content. Treating it as semantic composition would make the common border
  intersection unavailable without presentation-specific drawing.
- **Drawing the title through Canvas or `LineNetwork`.** That would require a
  caller to reproduce Block sizing, border clipping, and styles outside the
  Block that owns those cells.
- **Making title demand a minimum.** A long label would defeat explicit panel
  widths and finite application layouts instead of clipping within them.
- **Wrapping or adding a title row.** Both change the box's height and cease to
  describe text embedded in its top border.
