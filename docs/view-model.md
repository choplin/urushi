# View Model

Urushi represents composed terminal output as two style types and a view tree
that resolves to the single rectangle every renderer consumes.

The reasoning behind this shape — why presentation splits into two types, why
styles do not inherit, and which alternatives were rejected — is recorded in
[`design/view-block-model.md`](design/view-block-model.md). The value model
governing the two style types is defined in [`style-model.md`](style-model.md).

## The two style types and the view tree

Presentation splits into two values, and geometry belongs to only one of them:

```rust
/// Everything a terminal can express about a run of text.
pub struct TextStyle { fg, bg, underline, modifiers }

/// A rectangle, and the style filling the geometry it creates.
pub struct BlockStyle {
    padding, margin,
    border, border_top, border_right, border_bottom, border_left,
    border_foreground, border_background,
    width, height, min_width, min_height, max_width, max_height,
    overflow,
    align, vertical_align,
    text: TextStyle,
}
```

`TextStyle` alone produces no rectangle. `TextStyle::paint` wraps text in its
Select Graphic Rendition (SGR) scope; `BlockStyle::render` produces a
rectangle. `TextStyleProperty` converts into `BlockStyleProperty`; there is no
conversion in the other direction, so no geometry property can be applied to a
`TextStyle`.

A view is a tree of four nodes:

```rust
pub enum View {
    Text(String, TextStyle),
    Block(BlockStyle, Box<View>),
    Row(VerticalAlign, Vec<View>),
    Column(Align, Vec<View>),
}
```

- `Text` is a leaf: plain text and the style applied to it.
- `Block` applies one `BlockStyle` around exactly one child.
- `Row` places children side by side.
- `Column` stacks children.

These are the four things terminal output does: carry text, put a box around
something, place things beside each other, stack them. Components construct
views through `View::text`, `View::block`, `View::row`, and `View::column`;
there is one way to express each node.

## Intrinsic sizes of each node

Every node resolves to a rectangle. With no bound on the area, a node takes its
*intrinsic* size:

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
  background across alignment rows — and an empty `TextStyle` for a `Text` child.

Alignment belongs to the `Row` or `Column`, not to its children: a child cannot
align itself inside a height that is only known once its siblings are measured.

Two alignment biases coexist and are not unified. `Row` and `Column` place the
odd extra row of a `Center` alignment *above* the shorter child;
`BlockStyle`'s `vertical_align` places it *below* the padded content block, as
[`style-model.md`](style-model.md) specifies. Why they are separate rules is
recorded in [`design/view-block-model.md`](design/view-block-model.md).

A block's style does not flow into its child. There is no inheritance and no
patch operation; a `Block`'s style applies to the geometry that block creates —
border glyphs, padding, alignment fill — and each child carries its own
complete value. The reasoning is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

## The layout pass and what it produces

One layout pass turns a tree into a rectangle, and every renderer consumes that
rectangle:

```rust
pub struct Size { width: usize, height: usize }

/// The area a view may occupy: an input to layout, not an afterthought.
pub struct Available { width: Option<usize>, height: Option<usize> }

/// One grapheme, the width it occupies, and its logical style.
pub struct StyledGrapheme { symbol: String, width: usize, style: TextStyle }

pub struct ResolvedView { size: Size, rows: Vec<Vec<StyledGrapheme>> }

/// The intrinsic size: what the view asks for when nothing bounds it.
pub fn measure(view: &View) -> Size;
pub fn resolve(view: &View, available: Available) -> ResolvedView;
```

`Available` — a terminal width or a Ratatui `Rect` — participates in sizing
from the start. `Available` is not a clip applied to a finished rectangle; a
raw crop survives only as the degenerate-case safety net defined below.

Every row's widths sum to `size.width`, and the row count equals `size.height`.
Styles in a `ResolvedView` are logical: a `TerminalProfile` is applied when a
renderer serializes it, so terminal capability resolution stays at the output
boundary.

Rows hold per-grapheme tokens rather than styled text runs, so a renderer never
sees text below grapheme granularity and receives every width from the layout
pass instead of re-deriving it.

The same pass covers the single-block case: `BlockStyle::render` resolves
`Block(style, Text(content, style.text))` with unbounded `Available`. There is
one implementation of the box model in the workspace.

How a bound reshapes a box is defined under "Bounds, sharing, and overflow",
and the order those rules apply in under "How a node resolves".

## Bounds, sharing, and overflow

Three questions are kept separate: how large a box is, how siblings share an
area, and what happens to content that does not fit. Why they separate this
way, and which alternatives were rejected, is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

### How large a box is

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
`BlockStyle::frame_size`, defined in [`style-model.md`](style-model.md).

### How siblings share an area

`Row` hands its available width to its children; `Column`, its height. On that
main axis:

- `Cells` children take their stated size.
- Auto children take their intrinsic size.
- `Fill` children divide what remains, in proportion to their weights.

```text
Row: [ sidebar: width 20 ] [ main: fill 1 ]            -- a fixed sidebar; main takes the rest
Row: [ a: fill 1 ] [ b: fill 1 ]                       -- a 50/50 split
Row: [ a: fill 1 ] [ b: fill 2 ]                       -- a 1:2 split
Row: [ mode ] [ path: fill 1, clip "…" ] [ pos ]        -- a status bar: ends sized to content
```

(Each bracket is a `Block` child whose `BlockStyle` carries the stated length
and overflow; `Row` itself takes only its alignment and its children.)

A remainder that does not divide evenly is distributed in full: shares are cut
from a running prefix of it, so the odd cells fall to the last children, and
the shares always sum to the remainder — three equal weights over ten cells
are 3, 3, 4.

On the cross axis — height in a `Row`, width in a `Column` — there is nothing
to divide: the container passes its available extent to every child
unchanged, and a `Fill` length there stretches to it. A fixed-width sidebar
spanning the terminal's height is `width(20).height(Fill(1))` inside a `Row`,
and panels stacked inside it divide that height with their own `Length`s.

Each child is resolved once at its assigned size. There is no renegotiation: a
`Fill` child whose clamp caps it below its share leaves the remainder unused,
and the container resolves smaller than its area. Capping a *group* therefore
belongs on an enclosing block:

```text
Block: fill 1, align center                            -- spans the terminal, centers its child
  Block: max_width 120                                 -- caps the group
    Row: [ sidebar: width 20 ] [ main: fill 1 ]
```

A box resolved below its available area is *placed* by its parent's existing
alignment — `align`, `vertical_align`, and the `Row`/`Column` parameters — as
the outer `Fill(1)` block above places the capped group.

When the children's assigned sizes — stated `Cells`, intrinsic auto, `Fill`
shares — add up to more than the area, children shrink below those sizes:
`Fill` children first, then auto children, then `Cells` children, each
proportionally to size and floored at its own `max(min_width, min-content)`. A
floor that binds freezes that child, and the shortfall falls on the rest — an
iteration over numbers only, settled before any child is assembled. When even
the floors exceed the area, the container resolves larger than its area and the
degenerate safety net below is what finally bounds it.

A `Fill` length resolves against an area, so it needs one: an auto box with a
`Fill` anywhere among its descendants spans its own available extent (through
its own clamp), and so does each auto ancestor above it, until an ancestor with
a stated `Cells` size stops the propagation and keeps that size. Under
`measure`, where no area exists, `Fill` contributes the intrinsic size, and
weights have no effect.

### What happens to content that does not fit

The frame always closes at the used size. Overflow is absorbed by the
content, under a policy the application chooses per block:

```rust
pub enum Overflow {
    Wrap,                     // the default: reflow to the content width
    Clip(Cow<'static, str>),  // cut inside the frame, ending the line with a marker
}

Overflow::clip()             // cut silently
Overflow::ellipsis()         // Clip("…")
Overflow::clip_with("...")   // where an ASCII border would be chosen too
```

The marker occupies cells of its own, so the content keeps the content width
less the marker's display width — a three-cell `...` costs three. A marker the
box cannot hold beside any content is dropped, leaving a silent cut rather than
a box filled with the marker.

`overflow` governs the width axis. Height always clips inside the frame;
clipping inside a closed frame is a viewport's behavior, so scrolling composes
on top of this rule.

The policy fits the text a block directly contains. A child that is itself a
view absorbs its own overflow when it resolves under the area the containing
block leaves it, so the policy does not reach past one node. A bare `Text`
resolved in a narrow area wraps, the same default a block's content gets.
Choosing another policy requires a block, because the policy is a box property.

### Degenerate cases

When a box cannot reach even its floor, it degrades in order: margin
collapses first, then padding, then content. Only when the area cannot hold
the frame itself — two border columns in a width of one — does the final
safety net crop the assembled rectangle, grapheme-atomically, blank-filling a
dropped wide character's cells. This crop is the single way a frame is ever
cut. It is unreachable while the frame fits and the children's floors add up
to no more than the area; a container whose floors exceed its area resolves
larger than the area and meets this crop.

### What stays outside layout

The rules above size a box; conditional structure is not a sizing property.
"Hide the sidebar when the terminal is narrow" and "stack vertically below 80
cells" are decisions about which tree to build, made by the application's view
function, which holds the size that `resolve` will be given. The model's
obligation is that breakpoints are computable — `measure` is public, the
minimums an application declares are its own, and `BlockStyle::frame_size`
exposes a box's frame overhead — not that trees rewrite themselves. The
implicit floor a box has below its declared minimum, the widest grapheme it
cannot split, is not exposed as a query.

## How a node resolves

The sections above define the rules; this one is the procedure that applies
them, and it is the whole of resolution. Every node receives an area and
returns the size it resolved to: the area flows down, and the size flows back
up. What each node does with the area it receives — and what it hands its own
children — is fixed.

The extents the clamp needs — a subtree's max-content and min-content widths —
depend on the subtree alone, so they are read off the tree once, before any
area is handed down. One question does have to be asked of a subtree again: the
height a `Column` divides among its children, because a height is what fitting
the content produced. That question is pure — its answer depends only on the
subtree and the area it is asked about, never on what a sibling resolved to —
so asking it is not constraint solving.

What never happens is renegotiation: a size, once decided, is not revised in
the light of what a child or a sibling resolved to, and no node is assembled
twice. That is the boundary against a constraint solver, and
[`design/view-block-model.md`](design/view-block-model.md) records why it is
drawn there.

**`Text`** fits its lines to the width it was given, under the overflow policy
of the block containing it, and returns however many rows that produced. Its
width is the width it was given, except where a grapheme it cannot split is
wider than that.

**`Block`** applies the rules in this order:

1. **Degrade the frame to the area** — margin collapses first, then padding,
   and only by what the area cannot hold.
2. **Resolve the width** by the clamp. The content has not been laid out yet:
   the width comes from the intrinsic width, the bounds, and the area, never
   from what wrapping is about to do.
3. **Resolve the child** under an area of the used width less the frame, which
   is where a directly contained `Text` meets `overflow`.
4. **Resolve the height** by the same clamp — now the content's rows are
   known, because step 3 is what decided how many there are.
5. **Clip the content** to that height, from the bottom. `vertical_align`
   places slack; it has nothing to say when there is none.
6. **Draw the frame** at the used size, and the margin outside it.

**`Row`** gives each child its width by the distribution rule and passes its
own height to every child unchanged. **`Column`** does the same with the axes
swapped.

`resolve` applies the degenerate safety net once, to the finished rectangle;
`measure` runs these same rules with no area at all and stops at the size they
settle, which is what makes an intrinsic size the same computation as a bounded
one rather than a second rule.

Width is decided before the content because wrapping needs a width to wrap to;
height is decided after it because wrapping is what determines the row count.
This is why a narrower box can be a taller one, and why a `height` cannot be
met by reflowing: the rows already exist when the height applies, so the excess
clips. It also means a box does not shrink to the longest line its own wrapping
produced: a `max_width(9)` box whose content reflows to seven cells stays nine
wide. Why the axes resolve in this order is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

The order settles two more questions that would otherwise be ambiguous. A
`max_height` bounds the box *before* `vertical_align` places content inside
it, not after. And no bound ever reaches the frame, because steps 2 and 4
closed it at the used size before there was anything to cut.

## Plain text and rendered output

The layout pass never inspects text for escape sequences. Whether a string is
plain text or already-rendered ANSI is carried by the type and measured once,
where it is declared:

```rust
pub struct RenderedBlock { text: String, size: Size }

impl RenderedBlock {
    /// Adopts a string produced elsewhere. The caller asserts it is rendered
    /// output, and this is the one place ANSI-aware measurement happens.
    pub fn from_ansi(text: impl Into<String>) -> Self;
    pub fn size(&self) -> Size;
    pub fn as_str(&self) -> &str;
}
```

- A `Text` node holds plain text. Its width is grapheme display width.
- `BlockStyle::render` and `AnsiRenderer::render` return a `RenderedBlock`,
  which implements `Display`.
- `join_horizontal` and `join_vertical` take and return `RenderedBlock`. They
  compose rendered output — text this crate did not lay out, or output destined
  straight for a writer — and each input carries the size it was measured at, so
  they never re-measure ANSI text.
- A `RenderedBlock` does not re-enter the view tree. Content that participates
  in layout is expressed as a tree. Cutting an already-rendered string at a
  column is a text-layer utility, not part of the box model.

The plain side of the boundary is carried by types too, public alongside
`RenderedBlock`: `PrintableLines` for text that spans rows and `PrintableText`
for one row. Display width belongs to the second of these, because a width is a
property of a row of cells; measuring across a line break would sum cells that
never share one. Both types take the caller's declaration on trust, as
`from_ansi` does: the domain is declared, never detected.

Passing escape sequences to a `Text` node is therefore a contract violation,
not a supported call with a degraded result. Debug builds assert at the
boundary where the domain is declared; release builds do not check and instead
measure the escapes as ordinary characters, and wrapping or truncation may
split them. Detection is a development aid, never a runtime behavior the model
promises.

`from_ansi` measures what a terminal would show rather than what the byte
stream contains, because a row of rendered output may be a sequence of
operations rather than a sequence of cells. It is resolved once, at that
boundary:

- `\n` and `\r\n` end a row; the `\r` of a `\r\n` pair is not part of the row
  it ends, a lone `\r` does not end one, a trailing newline leaves one empty
  row, and empty text has no rows at all.
- `\r` returns to column 0, backspace steps back one column and stops there,
  and a tab advances to the next tab stop; text written afterwards lands on top
  of text written earlier, and overwriting either half of a wide character
  erases all of it.
- Each cell keeps the escape scope that was open when it was written, and the
  row re-emits only the transitions between them, so a scope closed before a
  carriage return still covers the cells it wrapped.

What comes out is a rectangle of cells that contains no cursor movement. That
invariant is what lets a block be placed at any column of a join without its
content sliding.

This is what lets the two backends agree: both consume a `ResolvedView` whose
contents are graphemes with known widths and logical styles, containing no
escape sequences at all.

## Backends

- `AnsiRenderer` resolves a view and serializes each row, coalescing adjacent
  graphemes of equal effective style into one SGR scope.
- `urushi-tui`'s `ViewWidget` derives `Available` from the target `Rect`,
  resolves the view, converts each grapheme's logical `TextStyle` through
  `RatatuiStyle`, and writes cells. `RatatuiWidget` draws a single `BlockStyle`
  through the same path.
- The core crate holds no Ratatui dependency: `ResolvedView` and `Available`
  are Urushi values.

A backend does not compute geometry; the reasoning is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

## Display width

Display width is decided once, in the layout pass, using the shared `text`
implementation, and carried per grapheme in the `ResolvedView`. No component and
no renderer defines its own notion of display width, and none re-measures one
the layout pass already decided.

There are exactly two measurement paths, and which one applies is decided by a
type rather than by inspecting a string: `PrintableText::width` for plain text,
and `RenderedBlock::from_ansi` for rendered output. The crate exposes no free
function taking a `&str` and returning a width; the reasoning is recorded in
[`design/view-block-model.md`](design/view-block-model.md). An application asks
the model instead — `measure`, `resolve(…).size()`, `RenderedBlock::size`, and
`BlockStyle::frame_size` are the computable breakpoints the application is
owed.
