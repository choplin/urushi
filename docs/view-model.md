# View Model

Urushi represents composed terminal output as primitive style values and a
component-agnostic view tree that resolves to the single scene every renderer
consumes. Its built-in nodes form a closed common vocabulary, while Region and
Canvas admit presentation-owned, renderer-neutral work through bounded
contracts.

This document gives the shape of that model: the primitive styles and nodes,
what the layout pass produces, and the principles sizing rests on. Semantic
components reach this tree through the presentations defined by
[`component-model.md`](component-model.md); they are never exposed as View
variants. The precise rules — how a box's size is clamped, how siblings divide
an area, how overflow
is absorbed, the order a node resolves in, how rendered output is measured —
each have a file under [`design/`](design/), linked from the section that
summarizes them. Why the model has this shape at all is recorded in
[`design/view-block-model.md`](design/view-block-model.md). The value model
governing the two style types is defined in [`style-model.md`](style-model.md).

## Primitive styles and the view tree

The styles attached to the two fundamental primitives split into two values,
and geometry belongs to only one of them:

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

A view is a tree of seven primitive nodes, and a keyed form of one of them:

```rust
pub enum View {
    Text(String, TextStyle),
    Block(BlockStyle, Box<View>),
    Row(VerticalAlign, Vec<View>),
    Column(Align, Vec<View>),
    Grid(GridStyle, Vec<Vec<View>>),
    Region(RegionPlan),
    Canvas(Canvas),
    AnchorBlock(Key, BlockStyle, Box<View>),
}
```

- `Text` is a leaf: plain text and the style applied to it.
- `Block` applies one `BlockStyle` around exactly one child.
- `Row` places children side by side.
- `Column` stacks children.
- `Grid` lines child Views up in shared columns.
- `Region` lets an owned layout plan measure and resolve within its local area.
- `Canvas` draws ordered, freely positioned commands on a finite local surface.
- `AnchorBlock` is a `Block` that also reports where its content landed.

These primitives cover the current common mechanics: carry text, put a box
around something, place things beside each other, stack them, and line them up
in columns. They are not claimed to exhaust every renderer-neutral operation a
full-screen frame needs. Applications and concrete presentations construct
views through `View::text`, `View::block`, `View::row`, `View::column`,
`View::grid`, `View::region`, and `View::canvas`; there is one way to express
each node.

Those nodes state layout and drawing intent only. There is no `View::List`,
`View::Table`, `View::Tree`, or `View::Graph`: a concrete presentation either
lowers that meaning into built-in nodes or binds its component snapshot,
styles, and behavior into an opaque Region plan. The plan may retain and
interpret presentation-specific data, but its type is erased; `View` and the
resolver expose no component-specific variant or branch.

`GridStyle` is the grid node's own geometry: an optional `Length` per column
and the default padding its cells take. It holds no box geometry or line
network — a grid that needs a border, a margin, or a stated size is placed
inside a `Block`, while a presentation that draws internal rules owns them in
a Region plan. What Grid computes and why it is independent of Table are
defined in
[`design/grid.md`](design/grid.md).

`RegionPlan` is an opaque cloneable and comparable value holding one concrete
`RegionLayout`. The plan reports horizontal requirements, reports height after
a width is selected, and resolves to a `ResolvedView` under the local
`Available` passed by its parent. Its constructor requires the concrete plan to
provide value equality before erasing its type, so `View` retains structural
`Clone` and `PartialEq` without closing the plan implementations into an enum or
a layout DSL. Region has no box properties: border, padding, stated size,
alignment, and overflow remain the responsibility of a surrounding `Block`.
The exact contract and its purity and equality rules are defined in
[`design/presentation-region.md`](design/presentation-region.md).

Region implementations construct their result through checked row-oriented
output values. Those values derive grapheme widths and preserve the rectangular
and anchor invariants of `ResolvedView`; they do not expose backend cells or the
positioned Canvas contract.

Canvas is a finite, local drawing surface whose owned items see its final size
and record `View`, `Text`, `Path`, or sparse `Cells` commands for that resolve.
Commands compose in recording order at cell granularity. Canvas size is
independent of item bounds, and commands may use signed positions beyond any
edge; clipping occurs during Canvas assembly. The sizing, command,
composition, anchor, and equality contracts are defined in
[`design/canvas.md`](design/canvas.md).

`AnchorBlock` adds no eighth thing. It is a box in every respect sizing reasons
about — one child, one `BlockStyle`, the same rules — and the key adds only a
report, for a caller that draws in that rectangle content this crate does not
produce. `View::anchor_block` builds one; `View::anchor` is the boxless case,
an empty region that covers no cells and so changes no layout. The `Key` naming
it is the one [`design/tui-application.md`](design/tui-application.md) defines,
and the anchor's rule and the reasoning behind it are recorded in
[`design/tui-view.md`](design/tui-view.md).

Alignment belongs to the `Row` or `Column`, not to its children: a child cannot
align itself inside a height that is only known once its siblings are measured.
A block's style does not flow into its child: there is no inheritance and no
patch operation, so each child carries its own complete value, and a `Block`'s
style applies only to the geometry that block creates — border glyphs, padding,
alignment fill. `Row`/`Column` centering and `BlockStyle`'s `vertical_align`
place their odd extra row on opposite sides; both are deliberate, and
[`design/view-block-model.md`](design/view-block-model.md) records why the
biases are not unified.

## The layout pass and the resolved scene

One layout pass turns a tree into a resolved scene, and every renderer consumes
that scene:

```text
pub struct Size { width: usize, height: usize }

/// The area a view may occupy: an input to layout, not an afterthought.
pub struct Available { width: Option<usize>, height: Option<usize> }

/// One grapheme, the width it occupies, and its logical style.
pub struct StyledGrapheme { symbol: String, width: usize, style: TextStyle }

/// One anchor's rectangle, stated from the resolved view's top-left cell.
pub struct AnchoredRect {
    key: Key,
    x: signed cell coordinate, y: signed cell coordinate,
    width: usize, height: usize,
    within_resolved_view: bool,
}

pub struct ResolvedView {
    size: Size,
    rows: Vec<Vec<StyledGrapheme>>,
    anchors: Vec<AnchoredRect>,
}

/// The intrinsic size: what the view asks for when nothing bounds it.
pub fn measure(view: &View) -> Size;
pub fn resolve(view: &View, available: Available) -> ResolvedView;
```

`Available` — a terminal width or a Ratatui `Rect` — participates in sizing
from the start. It is not a clip applied to a finished rectangle; a raw crop
survives only as the degenerate-case safety net.

Every row's widths sum to `size.width`, and the row count equals `size.height`.
Styles in a `ResolvedView` are logical: a `TerminalProfile` is applied when a
renderer serializes it, so terminal capability resolution stays at the output
boundary. Rows hold per-grapheme tokens rather than styled text runs, so a
renderer never sees text below grapheme granularity and receives every width
from the layout pass instead of re-deriving it.

`ResolvedView` carries no semantic data and no primitive nodes. It is the
resolved scene: styled graphemes plus placements such as anchors. A Region,
Canvas, or Graph presentation does not create another renderer input; after
lowering and resolution it produces the same `ResolvedView` as every other
tree.

Full-screen validation requires the same tree to express inline style changes
within one text flow, viewport projection, and positioned overlap without
application-computed final rectangles. These are generic capability
requirements rather than semantic component nodes; their boundary and the
responsibilities that remain in `urushi-tui` are recorded in
[`design/tui-view-expressiveness.md`](design/tui-view-expressiveness.md).

Anchored rectangles come from the same resolution as the rows: `anchors`
returns them all in tree order, a box before what it encloses, and
`anchor(key)` returns the one named. One key names one region; two anchors
carrying one key are a contract violation, asserted in debug builds and left
unresolved in release ones, as escape sequences in a `Text` node are.

A rectangle states where layout put the region, not what survived into the
rows, and is never bounded by the resolved size. Whether the rectangle contains
it is reported instead, by `is_within_resolved_view`: false when layout put the
region where the rectangle does not reach, as a cursor below content taller
than the area is. An empty region one cell past the content is still within it,
because that is where a cursor belongs when it follows the last grapheme.

Bounding a region inwards would report a cursor scrolled ten rows out of sight
as sitting on the last row. What being outside means is the caller's: a
full-screen runtime hides a cursor it cannot show or scrolls to it, and a
caller drawing into a region intersects it with the resolved size first.

The same pass covers the single-block case: `BlockStyle::render` resolves
`Block(style, Text(content, style.text))` with unbounded `Available`. There is
one implementation of the box model in the workspace.

## Sizing at a glance

Three questions are kept separate: how large a box is, how siblings share an
area, and what happens to content that does not fit.

**How large a box is.** Sizes are expressed in one vocabulary — `Length::Cells`
for an absolute size, `Length::Fill(weight)` for a share of the remaining
area, and *auto* (an absent `width` or `height`) for the intrinsic size — with
`min_*` and `max_*` bounds in cells. Every one of these measures the same box:
content plus padding plus enabled border edges, with margin outside. A box's
used size is a clamp: the stated or intrinsic size, capped by its maximum and
the available area, floored by its minimum and — on the width axis only — the
widest grapheme it cannot split. Where the floor exceeds the cap the floor
wins, and the box degrades in a fixed order before, as a last resort, a
grapheme-atomic crop bounds it. The clamp, the floors, the structural way to
size a content box, and the degenerate rules are defined in
[`design/box-sizing.md`](design/box-sizing.md).

**How siblings share an area.** `Row` hands its width to its children and
`Column` its height: `Cells` children take their stated size, auto children
their intrinsic size, and `Fill` children divide what remains by weight. Each
child is resolved once at its assigned size; nothing is renegotiated. When the
children need more than the area, they shrink in a fixed order down to their
floors. Distribution, the remainder rule, the cross axis, shrinking, and how a
`Fill` reaches an area through auto ancestors are defined in
[`design/area-sharing.md`](design/area-sharing.md).

**How cells line up across rows.** A `Grid` decides one width per column and
resolves every cell of that column under it, forming each column's claim from
the cells beneath it — the kind from an optional `Length` on the column, the
demand and the floor from the cells — and dividing its width by the same rule.
The column claim, the meaning of a `Length` on a column, cell padding, and the
absence of spans and line drawing are defined in
[`design/grid.md`](design/grid.md).

**How a specialized presentation participates.** A `Region` asks its bound
plan for a width demand and floor, then for its height at the width selected by
the parent. The plan resolves only after sibling sharing has derived its local
area. A `Block` around the Region supplies any stated size, fill claim, frame,
padding, alignment, or overflow; Region itself adds none. The phase agreement
and output invariants are defined in
[`design/presentation-region.md`](design/presentation-region.md).

**How a drawing surface participates.** A `Canvas` consumes a finite parent
allocation, or requires an explicit extent on an unbounded axis. Once both axes
are final, its items record commands using local signed coordinates; those
commands rasterize, compose in order, clip at the Canvas edges, and return the
same cells and anchors as every other node. Item contents never determine the
Canvas size. The full contract is [`design/canvas.md`](design/canvas.md).

**What happens to content that does not fit.** The frame always closes at the
used size; excess is absorbed by the content under a policy the application
chooses per block — `Overflow::Wrap` (the default) or `Overflow::Clip` with an
application-chosen marker. The policy governs the width axis; height always
clips inside the frame, and scrolling composes on top of that. Which text a
policy reaches, what a marker costs, and why the choice is the application's
are defined in [`design/overflow.md`](design/overflow.md).

## How a node resolves

Every node receives an area and returns the size it resolved to: the area
flows down, and the size flows back up, once. A `Block` decides its width
before its content is laid out — wrapping needs a width to wrap to — and its
height after, because wrapping is what determines the row count. That is why a
narrower box can be a taller one, and why a `height` cannot be met by
reflowing: the rows already exist when the height applies, so the excess
clips.

A size, once decided, is never revised in the light of what a child or sibling
resolved to, and no node is assembled twice. That is the boundary against a
constraint solver. Repeating a pure measurement of a subtree — which a `Column`
needs to divide its height — is inside the model, because its answer depends
on the subtree alone. `measure` runs the same rules with no area at all, so an
intrinsic size is the same computation as a bounded one rather than a second
rule.

Conditional structure — hiding a sidebar below a width, stacking instead of
placing side by side — is not a sizing property. It is a decision about which
tree to build, made by the application's view function, which holds the size
`resolve` will be given; the model owes it computable breakpoints, not trees
that rewrite themselves.

The full procedure, step by step per node, and the reasoning behind the order
are recorded in [`design/layout-resolution.md`](design/layout-resolution.md).

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
  in layout is expressed as a tree.

The plain side of the boundary is carried by types too: `PrintableLines` for
text that spans rows and `PrintableText` for one row, which owns display width.
Both take the caller's declaration on trust, as `from_ansi` does: the domain is
declared, never detected. Passing escape sequences to a `Text` node is a
contract violation, not a supported call with a degraded result.

`from_ansi` measures what a terminal would show rather than what the byte
stream contains — it resolves cursor movement to cells — and yields a
rectangle that contains none, which is what lets a block be placed at any
column of a join without its content sliding. The measurement rules, the
domain boundary, and why there is exactly one width measure are recorded in
[`design/rendered-output-measurement.md`](design/rendered-output-measurement.md).

## Backends

- `AnsiRenderer` resolves a view and serializes each row, coalescing adjacent
  graphemes of equal effective style into one SGR scope.
- `urushi-tui`'s `ViewWidget` derives `Available` from the target `Rect`,
  resolves the view, converts each grapheme's logical `TextStyle` through
  `RatatuiStyle`, and writes cells. `RatatuiWidget` draws a single `BlockStyle`
  through the same path.
- The core crate holds no Ratatui dependency: `ResolvedView` and `Available`
  are Urushi values.

A backend does not compute geometry; both consume a `ResolvedView` whose
contents are graphemes with known widths and logical styles, containing no
escape sequences at all. The reasoning is recorded in
[`design/view-block-model.md`](design/view-block-model.md).

## Display width

Display width is decided once, in the layout pass, using the shared `text`
implementation, and carried per grapheme in the `ResolvedView`. No component and
no renderer defines its own notion of display width, and none re-measures one
the layout pass already decided.

There are exactly two measurement paths, and which one applies is decided by a
type rather than by inspecting a string: `PrintableText::width` for plain text,
and `RenderedBlock::from_ansi` for rendered output. The crate exposes no free
function taking a `&str` and returning a width. An application asks the model
instead — `measure`, `resolve(…).size()`, `RenderedBlock::size`, and
`BlockStyle::frame_size` are the computable breakpoints the application is
owed.
