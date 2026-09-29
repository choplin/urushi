# Viewport Projection

How one `View` subtree is projected from a caller-selected cell origin into a
finite rectangle. [`../view-model.md`](../view-model.md) states where viewport
projection sits in the view model; this file owns the exact coordinate,
sizing, clipping, nesting, and anchor rules.

## The rule

A viewport is a renderer-neutral `View` node with exactly one child. It carries
an optional projection for each axis. Each projected axis states a signed cell
origin and one boundary behavior. The public values make at least one projected
axis and the boundary choice explicit:

```rust
pub enum ProjectionBoundary {
    Preserve,
    Clamp,
}

pub struct Projection { /* signed origin and boundary */ }
pub struct Viewport { /* one or both axis projections */ }

impl Projection {
    pub const fn new(origin: i64, boundary: ProjectionBoundary) -> Self;
    pub const fn origin(&self) -> i64;
    pub const fn boundary(&self) -> ProjectionBoundary;
}

impl Viewport {
    pub const fn horizontal(projection: Projection) -> Self;
    pub const fn vertical(projection: Projection) -> Self;
    pub const fn both(horizontal: Projection, vertical: Projection) -> Self;
}

impl View {
    pub fn viewport(viewport: Viewport, child: impl Into<View>) -> Self;
}
```

There is no boundary default and no constructor with both axes absent. A call
therefore states whether it wants an exact coordinate or a content-bounded one:

```rust
let view = View::viewport(
    Viewport::vertical(Projection::new(12, ProjectionBoundary::Clamp)),
    content,
);
```

An absent axis is not projected. It follows the ordinary layout rules of the
child and its ancestors. A present axis requires a finite allocation and makes
that allocation the viewport's extent on the axis. Missing finite allocation
is a layout error rather than an inferred size: a window has no meaning without
an edge at which content stops being visible.

For parent requirement calculation, a projected axis advertises the same
area-dependent claim as `Fill(1)`, with a zero floor: it consumes the finite
reference its parent assigns and may validly project to zero cells. This claim
propagates through auto Blocks, Rows, Columns, and Grid tracks like any
descendant `Fill`; a stated local box stops propagation and supplies its inner
extent. A Canvas View command must likewise supply an allocation when that
dependency has not already been stopped. Bottom-up requirement calculation
therefore remains possible before the extent is known. Validation happens
after top-down allocation: `try_measure` reports an error only when no finite
extent reaches the projected axis, and `measure` retains its existing
convenience contract and panics for that invalid unbounded request, just as it
does for a Canvas missing a required extent.

`Preserve` uses the requested origin exactly. Content before that origin is
translated out of the viewport; content that does not reach the opposite edge
leaves terminal-default blanks. Negative origins are valid and leave blanks
before content begins.

`Clamp` first constrains the requested origin to the interval in which content
can fill the viewport where its extent permits:

```text
maximum = max(content_extent - viewport_extent, 0)
effective_origin = min(max(requested_origin, 0), maximum)
```

The behavior is selected independently per projected axis. Urushi does not
infer it from the kind, provenance, completeness, or intended use of the
content.

The viewport then translates its child's complete local rectangle by the
negative effective origin and intersects it with the viewport rectangle. It
returns that finite projected rectangle to its parent. Projection changes
which cells are visible; it does not change the child's layout, revise a size,
or reflow content after an origin is applied.

## Layout before projection

The viewport extent and the child content extent are different facts. The
former is the finite rectangle returned to the parent; the latter is the
complete local rectangle over which an origin ranges.

On an unprojected axis, the child receives the ordinary area constraint and
resolves by the existing sizing rules. On a projected axis, the finite extent
populates the internal allocation reference used by `Fill` and area sharing,
while the content cap is absent. Auto and stated content claims may therefore
extend beyond the viewport and become reachable by another origin. The public
`Available` type is unchanged: a normal finite axis initializes both internal
fields to the same value. This distinction does not permit a decided size to
be renegotiated.

The distinction gives the two common axis combinations their expected result:

- a vertical-only viewport gives text the finite horizontal area, so text
  wraps at the viewport width, then projects rows from the vertical origin;
- a horizontal viewport preserves the child's horizontal content extent
  instead of rewrapping it at each origin.

A caller that wants a bound to change the child's layout states that bound in
the child tree, as it does without a viewport. Projection does not silently
turn a layout bound into a different overflow policy.

The viewport itself carries no style. Cells in its extent not covered by the
projected child are terminal-default blanks. A caller that needs another fill
wraps the viewport in an ordinary styled `Block`; background policy does not
become viewport state.

## Graphemes and four-edge clipping

Projection clips at the left and top as well as the right and bottom. A
grapheme remains atomic. If any positive-width grapheme would straddle a
viewport edge, none of its cells are copied and the cells it would have
occupied inside the viewport remain fill cells. Zero-width graphemes continue
to travel with the printable cluster to which text normalization attached
them.

The operation is defined by equivalence: projecting a rectangle must produce
the same cells as assembling the complete child rectangle, translating it by
the effective origin, and copying its grapheme-atomic intersection. An
implementation may avoid materializing invisible cells, but that optimization
cannot change the result. The evaluation choices are recorded separately in
[`resolution-reuse.md`](resolution-reuse.md).

## Nested viewports

Every viewport establishes a local content coordinate system and a local clip.
Its origin is measured from its child's top-left cell before that viewport is
translated or placed by an ancestor. A descendant viewport resolves in that
local system first; the ancestor then treats the descendant's finite result as
ordinary child content.

Visible cells and placements must survive every enclosing clip. A descendant
cannot become visible merely because its translated coordinates happen to fall
inside the root rectangle after it was outside an intermediate viewport. No
viewport carries pane identity, focus, commands, or mutable state; independent
regions are independent nodes because the tree places a projection around each
relevant subtree.

## Anchors

Cells and anchors take the same translation and the same stack of viewport
clips. An anchor retains its complete logical rectangle in the final resolved
coordinate system; projection never rounds that rectangle onto an edge. It
also reports the intersection that remains visible after every local and
ancestor clip, distinguishing three states:

- **visible** — the complete logical rectangle is visible;
- **clipped** — a non-empty part is visible;
- **outside** — no part is visible.

The full rectangle preserves the position needed to derive a foreign region's
source offset. The visible intersection tells a caller where it may draw. A
zero-sized cursor anchor has no partially visible state: its point is either
visible under every enclosing viewport or outside, and the TUI renderer hides
an outside cursor. Clip rectangles use half-open cell ranges. A zero-sized
cursor point is visible only when `left <= x < right` and
`top <= y < bottom`; a point exactly on the right or bottom edge is outside.
A presentation that needs to show an insertion point after its last grapheme
must allocate the cell in which that cursor will appear. These rules refine the anchor contract in
[`tui-view.md`](tui-view.md); they do not turn an anchor into content or a
scroll target.

## Canvas

A Canvas and a viewport answer different questions. Canvas is a finite local
drawing surface. Its viewport sizing mode means that the surface consumes a
finite parent allocation; it does not give Canvas a scroll origin or a larger
retained content space. Canvas therefore continues to rasterize its complete
settled surface under the existing contract.

When that finite Canvas is inside a viewport, its cells and anchors are
projected like those of any other child. If a caller instead gives Canvas an
explicit surface larger than the viewport, eager rasterization of that surface
is a valid one-shot implementation. Partial Canvas evaluation is an optional
reuse optimization only when it can preserve the same complete-surface result;
viewport semantics do not require every Canvas command to become spatially
queryable.

## What remains with the caller

The viewport accepts projection values; it does not produce or update them.
Navigation commands, selection or cursor following, tail following, source
positions, data loading, semantic windowing, prefetch, and application cache
policy remain outside Urushi. A caller may construct a `View` from all of its
data or from any finite subset and then apply either boundary behavior. The
projection API neither knows nor records which choice was made.

Large and independently scrolling applications are validation cases for this
mechanic, not categories in its type system. The contract is complete when the
same inputs have the same projected result regardless of why a caller selected
the content or origin.

A tree containing no `Viewport` follows the existing rules unchanged. In
particular, ordinary CLI output may remain vertically unbounded, and
`urushi-prompt` may continue choosing a semantic visible field window before
building its View and applying its prompt-specific Frame stage afterward. A
finite projection extent is required only where a caller actually inserts this
node.

## Why projection is a View node

A renderer-only root offset cannot express independently projected descendants
and would let cells and anchors take different paths. Pre-slicing text in a
presentation loses the common layout coordinate system needed by Canvas,
foreign regions, and nested boxes. A block style property would mix a dynamic
one-frame coordinate with a reusable appearance and geometry value.

A one-child View node instead composes wherever other layout nodes compose,
keeps the operation renderer-neutral, and lets one resolution transform cells
and placements together. It adds a presentation operation, not an application
model.

## Rejected designs

- **One implicit root viewport in the TUI framework.** It cannot express independent
  nested regions and is unavailable to callers using the Ratatui widget or
  another renderer.
- **Always clamp.** It assigns application intent to a coordinate operation and
  makes an exact projection of caller-selected content impossible.
- **Always preserve.** It makes the common fill-the-final-page behavior require
  the caller to reproduce resolved cell extents before resolution.
- **Application-computed final rows or rectangles.** It separates content from
  the layout and anchor pass that must transform it.
- **Viewport state or identity in the node.** Commands, lifetime, and identity
  are not required to compute one frame. Optional evaluation reuse is a
  separate concern.
