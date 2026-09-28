# View Model

Urushi represents composed terminal output as primitive style values and a
component-agnostic view tree that resolves to the single scene every renderer
consumes. Its built-in nodes form a closed common vocabulary, while Canvas
admits presentation-owned, renderer-neutral measurement and drawing through a
bounded contract.

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
The styled plain-text invariant is defined in
[`design/styled-text.md`](design/styled-text.md).

## Primitive styles and the view tree

The styles attached to the two fundamental primitives split into two values,
and geometry belongs to only one of them:

```rust
/// Everything a terminal can express about a run of text.
pub struct TextStyle { foreground, background, underline, attributes }

/// A rectangle, and the style filling the geometry it creates.
pub struct BlockStyle {
    padding, margin,
    border, border_top, border_right, border_bottom, border_left,
    border_foreground, border_background,
    width, height, min_width, min_height, max_width, max_height,
    overflow,
    align, vertical_align,
    text_style: TextStyle,
}
```

`TextStyle` alone produces no rectangle. A `BlockStyle` contributes geometry
when it is attached to a `View::block`. The two types share named text
operations, while geometry operations exist only on `BlockStyle`, so no
geometry property can be applied to a `TextStyle`.

A view is a tree of seven primitive nodes, and a keyed form of one of them:

```rust
pub enum View {
    Text(StyledText),
    Block(BlockStyle, Option<BlockTitle>, Box<View>),
    Row(VerticalAlign, Vec<View>),
    Column(Align, Vec<View>),
    Grid(GridStyle, Vec<Vec<View>>),
    Canvas(Canvas),
    Viewport(Viewport, Box<View>),
    AnchorBlock(Key, BlockStyle, Option<BlockTitle>, Box<View>),
}
```

- `Text` is a leaf: one plain-text flow with zero or more styled segments.
- `Block` applies one `BlockStyle` and an optional border title around exactly
  one child.
- `Row` places children side by side.
- `Column` stacks children.
- `Grid` lines child Views up in shared columns.
- `Canvas` owns one sizing mode and draws ordered, freely positioned commands
  on the finite local surface that mode produces.
- `Viewport` projects one child's local content coordinates into a finite
  rectangle supplied by layout.
- `AnchorBlock` is a `Block` that also reports where its content landed.

These primitives cover the current common mechanics: carry text, put a box
around something, place things beside each other, stack them, line them up in
columns, draw into a finite surface, and select a finite window from content.
Applications and concrete presentations construct views through `View::text`,
`View::styled_text`, `View::block`, `View::titled_block`, `View::row`,
`View::column`, `View::grid`, `View::canvas`, and `View::viewport`; there is one
way to express each node.

`BlockTitle` is one styled line of content embedded in a Block's top border.
It is not a `BlockStyle` property: changing the text does not change the
reusable geometry and paint value. A title adds an automatic width demand but
never adds a row, and finite or stated widths clip it within the surviving top
edge. The precise sizing, alignment, padding, and clipping rules are defined
in [`design/block-title.md`](design/block-title.md).

`TextSpan` is caller input: a string fragment and one complete `TextStyle`.
`StyledText` joins those fragments into one source string, keeps private,
canonical style ranges on grapheme boundaries, and owns the policy that turns
source tabs into fixed-width printable cells before layout. The style ranges do
not create line, word, wrap, or clip boundaries. A `String` or string literal
converts to a default-style `TextSpan`, so only styled fragments need an
explicit constructor. Direct text rendering preserves source tabs instead of
applying the layout policy. The exact construction, tab, and normalization
rules are defined in [`design/styled-text.md`](design/styled-text.md).

Those nodes state layout and drawing intent only. There is no `View::List`,
`View::Table`, `View::Tree`, or `View::Graph`: a concrete presentation either
lowers that meaning into built-in nodes or binds its component snapshot,
styles, and behavior into a Canvas item. The item may retain and interpret
presentation-specific data, but `View` and the resolver expose no
component-specific variant or branch.

For example, the canonical Tree presentation binds its hierarchy into an
intrinsically sized Canvas item. The item records ordinary `LineNetwork` and
text commands at the final local width; neither Canvas nor `View` learns what a
tree node or connector role means.

`GridStyle` is the grid node's own geometry: an optional `Length` per column
and the default padding its cells take. It holds no box geometry or line
network — a grid that needs a border, a margin, or a stated size is placed
inside a `Block`, while a presentation that draws internal rules owns them in
an intrinsically sized Canvas. What Grid computes and why it is independent of
Table are defined in
[`design/grid.md`](design/grid.md).

Canvas is a finite, local drawing surface whose owned items see its final size
and record `View`, `Text`, cell-space primitives, `LineNetwork`, or sparse
`Cells` commands for that resolve. A future sampled `Drawing` model remains a
separate capability rather than a kind of line network.
It carries exactly one sizing mode. The default viewport mode consumes finite
allocation and requires explicit extents on unbounded axes; an intrinsic mode
may instead be supplied by a built-in presentation to report width
requirements and height at the selected width. Neither mode derives size from
item bounds. After sizing, Canvas assembly rasterizes one command at a time
through the same internal contract without access to the Canvas surface,
immediately applies its output through one compositor, and releases that output
before rasterizing the next command. Commands may use signed positions beyond
any edge; the compositor clips them to Canvas. `LineNetwork` derives corners,
tees, and crossings from the incident directions of segments within that one
network, then emits ordinary cells through the same command composition path.
Neither `CanvasCell` nor the final `ResolvedView` owns line topology.
The sizing, command, composition, anchor, and equality contracts are defined in
[`design/canvas.md`](design/canvas.md).

`Viewport` is a pure presentation operation. Its caller supplies an optional
signed origin and boundary behavior independently for each axis; layout
supplies the finite extent. A projected axis participates in parent sizing as
an area-dependent, zero-floor claim, so the requirement can propagate through
auto ancestors until a finite allocation or stated box satisfies it.
Resolution lays out the complete child coordinate
space, then translates and clips cells and anchors into that extent. The node
does not own scrolling, navigation, a data window, or cache identity. Those are
caller concerns that may produce its current inputs. The coordinate, sizing,
clipping, nesting, and anchor rules are defined in
[`design/view-projection.md`](design/view-projection.md).

`AnchorBlock` adds no eighth thing. It is a box in every respect sizing reasons
about — one child, one `BlockStyle`, the same optional `BlockTitle`, the same
rules — and the key adds only a report, for a caller that draws in that
rectangle content this crate does not produce. `View::anchor_block` and
`View::titled_anchor_block` build the two forms; `View::anchor` is the boxless
case, an empty region that covers no cells and so changes no layout. The `Key`
naming it is the one [`design/tui-application.md`](design/tui-application.md)
defines, and the anchor's rule and the reasoning behind it are recorded in
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
    visible: Option<VisibleRect>,
}

pub struct VisibleRect {
    x: signed cell coordinate, y: signed cell coordinate,
    width: usize, height: usize,
}

pub struct ResolvedView {
    size: Size,
    rows: Vec<Vec<StyledGrapheme>>,
    anchors: Vec<AnchoredRect>,
}

/// The intrinsic size; panics when an unbounded node requires a finite extent.
pub fn measure(view: &View) -> Size;
/// The fallible intrinsic-size operation.
pub fn try_measure(view: &View) -> Result<Size, LayoutError>;
pub fn resolve(view: &View, available: Available) -> Result<ResolvedView, LayoutError>;
```

`Available` — a terminal width or a Ratatui `Rect` — participates in sizing
from the start. At the root, each finite axis supplies both the allocation
reference and the content cap. Most nodes pass those facts together. A
`Viewport` separates them only on a projected axis: its finite extent remains
the reference for `Fill` and area sharing, while its child content is not
capped until projection clips the settled result. This is an internal
constraint distinction, not a second public area type. A raw crop survives
only as the degenerate-case safety net.

Every row's widths sum to `size.width`, and the row count equals `size.height`.
Styles in a `ResolvedView` are logical: `RenderSettings` are applied when
`render` serializes it, so output feature selection stays at the output
boundary. Rows hold per-grapheme tokens rather than styled text runs, so a
renderer never sees text below grapheme granularity and receives every width
from the layout pass instead of re-deriving it.

`ResolvedView` carries no semantic data and no primitive nodes. It is the
resolved scene: styled graphemes plus placements such as anchors. A Canvas or
Graph presentation does not create another renderer input; after
lowering and resolution it produces the same `ResolvedView` as every other
tree.

Full-screen validation requires the same tree to express inline style changes
within one text flow, viewport projection, and positioned overlap without
application-computed final rectangles. These are generic capability
requirements rather than semantic component nodes; their boundary and the
responsibilities that remain with the caller are recorded in
[`design/tui-view-expressiveness.md`](design/tui-view-expressiveness.md).

Anchored rectangles come from the same resolution as the rows: `anchors`
returns them all in tree order, a box before what it encloses, and
`anchor(key)` returns the one named. One key names one region; two anchors
carrying one key are a contract violation, asserted in debug builds and left
unresolved in release ones, as escape sequences in a `Text` node are.

A rectangle states where layout put the complete logical region, not only what
survived into the rows, and is never moved onto a clipping edge. Its visible
intersection is reported separately: equal to the logical rectangle when fully
visible, smaller when clipped, and absent when outside. An empty cursor region
is either visible at its exact point or outside. Every clipping stage updates
this intersection; exact edge semantics are defined in
[`design/tui-view.md`](design/tui-view.md).

Bounding a region inwards would report a cursor projected ten rows out of sight
as sitting on the last row. The separate visible rectangle avoids that loss: a
full-screen renderer hides an outside cursor, while a caller drawing into a
foreign region uses the intersection and the logical rectangle together to
derive its destination and source offset.

A single styled block is represented as a `View::block` containing a text view.
It goes through the same `resolve` function as every other tree, so there is one
implementation of the box model in the workspace.

## Sizing at a glance

Three questions are kept separate: how large a box is, how siblings share an
area, and what happens to content that does not fit.

**How large a box is.** Sizes are expressed in one vocabulary — `Length::Cells`
for an absolute size, `Length::fill(weight)` for a positive weighted share of
the remaining area, and *auto* (an absent `width` or `height`) for the intrinsic
size. These sizes combine with `min_*` and `max_*` bounds in cells. Every one of
them measures the same box:
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
children need more than the content cap, they shrink in a fixed order down to
their floors. A projected axis has a finite `Fill` reference but no child cap,
so claims beyond the viewport extent remain available to projection.
Distribution, the remainder rule, the cross axis, shrinking, and how a `Fill`
reaches an area through auto ancestors are defined in
[`design/area-sharing.md`](design/area-sharing.md).

**How cells line up across rows.** A `Grid` decides one width per column and
resolves every cell of that column under it, forming each column's claim from
the cells beneath it — the kind from an optional `Length` on the column, the
demand and the floor from the cells — and dividing its width by the same rule.
The column claim, the meaning of a `Length` on a column, cell padding, and the
absence of spans and line drawing are defined in
[`design/grid.md`](design/grid.md).

**How a drawing surface participates.** A `Canvas` consumes a finite parent
allocation under its default viewport sizing, or requires an explicit extent
on an unbounded axis. A built-in presentation may explicitly replace that mode
with intrinsic sizing, which reports a width demand and floor and then height
requirements at the width selected by the parent. A surrounding `Block`
supplies stated size, fill, frame, padding, alignment, and overflow. Once both
Canvas axes are final, its items record commands using local signed
coordinates; those commands rasterize, compose in order, clip at the Canvas
edges, and return the same cells and anchors as every other node. Items never
determine the Canvas size. The full contract is
[`design/canvas.md`](design/canvas.md).

**How a finite window selects content.** A `Viewport` keeps layout and
projection distinct. Its child first settles in local content coordinates;
the caller-supplied origin then selects the cells visible in the finite extent
allocated to the viewport. `Preserve` uses that origin exactly, while `Clamp`
limits it to the range that can fill the viewport when the content permits.
Neither choice is inferred from the content or its application. The complete
rule is [`design/view-projection.md`](design/view-projection.md).

**What happens to content that does not fit.** The frame always closes at the
used size; excess is absorbed by the content under a policy the caller chooses
per block — `Overflow::Wrap` (the default) or `Overflow::Clip` with a
caller-chosen marker. The policy governs the width axis; height always clips
inside the frame. Selecting a nonzero origin is the separate projection
operation above. Which text a policy reaches, what a marker costs, and why the
choice is the caller's are defined in
[`design/overflow.md`](design/overflow.md).

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
on the subtree alone. `try_measure` runs the same rules with no area at all, so
an intrinsic size is the same computation as a bounded one rather than a
second rule. It reports a missing finite extent for a viewport or
viewport-sized Canvas that cannot be measured unbounded; `measure` is the
convenience form that panics on that invalid request.

A `Viewport` does not revise those decisions. It resolves the child's content
coordinates and then selects a rectangle from them. The ordinary `resolve`
function remains a direct, stateless operation: trees without projection do
not acquire a retained intermediate representation, and one-shot projection
may assemble eagerly. Reusing settled content across changing origins, or
unchanged subtrees across successive immutable frame snapshots, is an optional
stateful evaluation path with the same observable result, as defined in
[`design/resolution-reuse.md`](design/resolution-reuse.md).

That state does not own layout policy or preserve an old layout. Conditional
structure and every geometry-bearing value still come from the current View;
the resolver reruns layout and may reuse materialized output only when the
resulting local assembly input remains equal.

Conditional structure — hiding a sidebar below a width, stacking instead of
placing side by side — is not a sizing property. It is a decision about which
tree to build, made by the application's view function, which holds the size
`resolve` will be given; the model owes it computable breakpoints, not trees
that rewrite themselves.

The full procedure, step by step per node, and the reasoning behind the order
are recorded in [`design/layout-resolution.md`](design/layout-resolution.md).

## Plain text and rendered output

The layout pass accepts model values, not rendered strings. A `Text` node holds
one `StyledText`; tabs are replaced under its fixed-width policy before its
graphemes are measured, and segment boundaries do not affect the result.
`PrintableLines` and `PrintableText` carry the printable-text contract after
that replacement.

Passing escape sequences to a `Text` node is a contract violation. Content that
participates in layout is composed as a `View` before `resolve`; `render`
returns a final `String`, and that string does not re-enter the view tree.

## Backends

- `render` serializes an already-resolved view under explicit `RenderSettings`,
  coalescing adjacent graphemes of equal effective style into one SGR scope.
- `render_text` serializes a `StyledText` directly under explicit
  `RenderSettings`; it performs no layout and preserves source tabs and line
  boundaries.
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

The crate exposes no function that reparses a rendered `String` to recover
geometry. An application asks the model instead: `measure`, the `ResolvedView`
returned by `resolve`, and `BlockStyle::frame_size` are the computable
breakpoints the application is owed.
