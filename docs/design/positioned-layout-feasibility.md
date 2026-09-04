# Positioned Layout Feasibility

Whether several existing box-shaped `View` children can be placed at cell
coordinates without replacing the box model or turning resolution into a
constraint solver. The one-frame capability this investigates is identified
in [`tui-view-expressiveness.md`](tui-view-expressiveness.md); this document
records the evidence and the constraints that a binding positioned-layout
design must take as inputs.

## Result

The hypothesis is **conditionally feasible**.

A minimal primitive can place ordinary child views at signed, local cell
coordinates, clip them to one resolved rectangle, and paint them in a stable
back-to-front order. It can retain the current width-then-height-then-assembly
pipeline: child extents are measured bottom-up, the container and child widths
are settled top-down, heights follow after wrapping, and each child rectangle
is assembled once. No child or sibling has to be re-resolved from another
child's result.

The condition is that positioned placement is a new assembly operation, not a
spelling of `Row`, `Column`, or `Grid`. Three current assumptions must change:

- placement offsets and reported anchor origins cannot remain unsigned;
- assembly must intersect a child with all four container edges, not only crop
  the finished result at its right and bottom edges; and
- overlap must have an explicit cell-ownership rule, including what a blank
  cell does to content behind it.

This result establishes feasibility, not the public type or its defaults. In
particular it does not decide whether a bounded positioned container fills its
area or remains at its intrinsic extent. That decision changes ordinary call
behavior and belongs with the complete coordinate, sizing, clipping, and
ordering contract.

## The minimal capability

The smallest useful scope is a rectangle containing an ordered sequence of
placements:

```text
placement = (signed local x, signed local y, child View)
```

The coordinate origin is the container's top-left content cell. Positive `x`
moves right and positive `y` moves down. A negative coordinate is valid and
puts the corresponding leading part of the child outside the container; it
does not move the origin or enlarge the container to the left or above.

Each child remains an ordinary `View`. A caller that needs a border, padding,
margin, an exact size, bounds, fill, or an anchor uses the existing `Block` or
`AnchorBlock` around that child. The positioned container adds no semantic
node, graph edge, camera, event, hit-test result, or backend rectangle.

The sequence itself is sufficient ordering for this scope: earlier children
paint first and later children paint over them. A caller changes the order by
changing the sequence. A `z_index`, hierarchy-aware stacking context, or
separate ordering key would add states and sorting policy without a current
non-graph use that needs them.

The minimal compositing rule is rectangular opacity. Every cell in a later
child's resolved rectangle, including a space inserted by its block or
alignment, replaces the cell behind it. Transparency is not inferred from a
space or from a default style: both can be intentional output. Per-cell
coverage, alpha-like behavior, and shape-aware hit testing are a larger scene
or compositor contract.

This scope is enough for overlapping native boxes such as a popup, floating
help, an annotation, or manually positioned cards. It is not enough by itself
for a zoomable graph editor.

## Fit with the existing resolution pass

The current pass has exactly the information the primitive needs, but the new
node must carry it through each phase.

### Intrinsic measurement

Under an unbounded axis, each child is measured by the existing rules. The
container's intrinsic extent on that axis is the farthest non-negative child
edge:

```text
width  = max(0, max(x + child_intrinsic_width))
height = max(0, max(y + child_intrinsic_height))
```

The arithmetic must be checked in a signed domain. A child wholly left of or
above the origin contributes zero on that axis. Negative coordinates never
cause every other placement to be translated, so adding an off-screen child
cannot move existing content.

As with every current node, `measure` asks the same resolver questions with no
available area. It need not assemble a rectangle, and a `Fill` below an
unbounded axis contributes its intrinsic demand rather than inventing an area.

### Bounded resolution

Once the container's used width is settled, each child can receive at most the
extent from its origin to the container's far edge:

```text
child_available_width = max(0, container_width - x)
```

The subtraction is signed before it is converted to a non-negative available
extent. Thus a child at `x = -3` may resolve up to three cells wider than the
container: the first three cells are outside, while the remainder can reach
the right edge. The height phase applies the corresponding rule after wrapping
has fixed the child's rows:

```text
child_available_height = max(0, container_height - y)
```

This preserves the existing dependency direction. A child receives one area
and resolves once; its size is never revised after another child resolves.
Numeric freeze loops used to divide `Row`, `Column`, and `Grid` space are not
needed because positioned siblings share no track.

The binding design still has to choose how the container obtains
`container_width` and `container_height` when an axis is bounded. Two coherent
choices survive the feasibility check:

- **intrinsic extent capped by the area** behaves like an auto container and
  may leave unused space; or
- **take the bounded area, intrinsic when unbounded** behaves like a viewport
  surface and gives positioned descendants the full allocation.

An enclosing `Block` can state exact, bounded, and `Fill` box geometry, but it
does not erase this choice: the child container must still decide whether its
own rectangle consumes all the content area the block offered. Adding a second
width/height vocabulary directly to the positioned node would duplicate the
box model and is therefore not required for feasibility.

### Child overflow versus container clipping

The two operations have different owners and must stay distinct:

1. The child resolves its box and absorbs its own overflow under the
   `BlockStyle` policy and the `Available` it received.
2. Assembly translates the already-resolved child rectangle and copies only
   its intersection with the positioned container.

The second operation may remove the left or top of a child. It is projection
at a container boundary, not another layout decision, and must not cause the
child to wrap or lay itself out again. A child with a partially hidden border
is therefore assembled at its full resolved local size and then intersected,
the same reason rataflow renders a partially off-screen node in a complete
local scratch buffer before copying its visible subrectangle.

The top-level right/bottom crop in `resolve` remains a degenerate safety net.
Ordinary positioned clipping happens while assembling the positioned node,
after that node's size has already been settled. If the safety crop routinely
clips positioned children, the sizing phase is wrong.

### Grapheme ownership

Intersection and overwrite must be grapheme-atomic. A wide grapheme crossing
any clip edge is dropped rather than split. Painting a later grapheme over any
cell owned by an earlier wide grapheme must clear the earlier grapheme's whole
span before writing the new one; otherwise a continuation cell or half of a
glyph survives.

Urushi's assembly rows currently retain grapheme tokens and their widths but
are optimized for concatenation. Positioned overwrite therefore needs a
cell-addressable internal assembly representation or equivalent span-aware
row operations. This is an implementation change inside resolution, not a
second renderer input: the output remains one rectangular `ResolvedView`.

## Anchors and signed placement

An anchor reports where layout put a region, even when the final resolved view
does not contain it. Clipping a negative-position child and reporting only the
visible intersection would violate that rule and could falsely move an
off-screen cursor or foreign region onto the boundary.

The positioned design must therefore represent an anchor's logical origin in
a signed coordinate domain through assembly. It should translate the full
logical rectangle first and determine containment against the final resolved
size afterward, as today. The current `AnchoredRect` fields and `offset`
operation use `usize`, so they cannot express this case. A binding design must
choose:

- the signed public coordinate type;
- checked behavior when an offset plus an extent is not representable; and
- whether the existing full-containment boolean is sufficient or callers also
  need to distinguish partial intersection from complete exclusion.

Keeping unsigned origins by clamping or discarding negative anchors is not a
viable option: it destroys layout information that the anchor contract
deliberately preserves.

## Evidence from rataflow

Rataflow separates three spaces because its graph problem includes more than
positioned terminal boxes:

```text
world f64 -- pan/zoom --> canvas f64 -- area offset/floor --> terminal i32
                                                   -- clip --> buffer u16
```

- World coordinates preserve layout output, fractional zoom, smooth dragging,
  and zoom-independent hit testing.
- Canvas coordinates are world coordinates after the camera transform and are
  relative to the drawing surface.
- Signed terminal coordinates retain off-screen left/top positions until
  clipping proves that conversion to Ratatui's unsigned buffer coordinates is
  safe.

Visible world bounds cull nodes and edges before rendering. A partially visible
node is still rendered into a full, origin-zero scratch buffer, then the
visible source subrectangle is copied to the canvas. Edges use a separate
buffer so crossing symbols can merge before that buffer is composited. Nodes
then paint in `(effective_z, insertion_order)`, children are forced above their
parents, selected nodes may be elevated, and hit testing reads the reverse of
the same order.

These are reasons not to copy rataflow's `Flow` into core. The three-space
transform, graph hierarchy, edge merge policy, selection elevation, culling,
and hit testing serve a scene with camera and interaction state. The reusable
lesson for the minimal primitive is narrower: keep logical signed positions
until clipping, render a child in its complete local geometry, and make paint
order deterministic.

## Evidence from Zoetrope

Zoetrope demonstrates the boundary in an application rather than a widget
demo. Its `SessionModel` is semantic session data. `state/graph.rs` projects
that data into a rataflow `Flow`, while `App` separately owns the flow,
timeline and playhead, camera mode and glide, selection-derived panel state,
scroll offsets, overlay flags, and hit-test rectangles.

One frame uses ordinary Ratatui row/column splitting for the graph, detail
panel, timeline, and status bar. Within the graph region it explicitly paints
`Background`, the mutable `Flow`, frame-exact chips, and `MiniMap` in that
order. Help and information popups use `Clear` and paint after the base frame.
The timeline uses `Line` and `Span` for styled runs, then writes markers and a
playhead directly into the buffer so later marks overwrite earlier ones.

That separation shows both sides of the boundary:

- a minimal positioned primitive can express native opaque boxes and overlays
  without owning pan, zoom, selection, dragging, or timeline state; but
- graph edges, transparent parent nodes, minimap projection, styled inline
  flow, and cell-level timeline marks require other primitives or a dedicated
  scene/compositor escape hatch.

Treating direct `Rect` and buffer use in Zoetrope as proof that every one of
those operations belongs in a generic Canvas would collapse application,
graph-presentation, and renderer responsibilities back together.

## Noctui parity

The checked-out Noctui source currently implements the older
`Text`/`Block`/`Row`/`Column` intrinsic layout and a final `Limits` crop; it does
not yet implement Urushi's area-driven pipeline, anchors, or Grid. Its current
shape is migration scope, not evidence against this result.

Noctui already has two useful lower-level properties. `Bounds` and `Position`
use signed `Int`, and its `Buffer` represents wide graphemes with explicit
start/continuation ownership and clears a complete old span before overwrite.
However, public buffer placement requires exact containment and rejects a
partial intersection atomically. Positioned clipping must therefore be added
to view resolution or to a new internal compositor; reusing that public
placement operation unchanged would not implement the same model.

Urushi and Noctui can expose equivalent coordinates, size rules, ordering,
opacity, clipping, and anchor results. The language/runtime-specific parts are
only their spelling and implementation:

- Rust needs a deliberate signed type beside `usize` sizes and careful
  conversion at backend boundaries;
- MoonBit's `Int` already represents negative positions, but arithmetic and
  allocation representability still need checks; and
- each implementation may choose a different private cell/span structure as
  long as wide-grapheme overwrite and the observable resolved scene agree.

## Options evaluated

### Keep: an ordered positioned list of opaque child views

This is the conditionally feasible minimum. It reuses every child's box model,
keeps one `measure`/`resolve` operation, introduces no semantic or interaction
state, and has a testable ordering and clipping rule.

### Reject: encode positions with rows, columns, margins, or blank text

Sequential containers cannot overlap, cannot preserve an independent draw
order, and make negative positions inexpressible. Blank text also bakes area
dependent geometry into content before resolution.

### Reject: use anchors as native positioned content

An anchor reserves and reports a rectangle whose content core does not draw.
Using one for ordinary native content would bypass the expressiveness
criterion rather than satisfy it, and the application would still have to
compute backend rectangles and paint cells itself.

### Defer: a generic transparent scene or compositor

Transparent blanks, masks, arbitrary cell blending, edge-junction merging,
stable z-indices, transforms, and hit testing form a coherent larger contract.
No current non-graph use requires that whole surface, and `ResolvedView` has no
coverage channel from which it could be inferred safely.

### Reject: port rataflow's graph model into the view tree

Rataflow intentionally combines persistent graph content, hierarchy, camera,
selection, event handling, layout, hit testing, edge routing, and Ratatui
rendering. Those responsibilities cross Urushi's semantic-data,
presentation, primitive-view, runtime, and backend boundaries. It is prior art
for their interaction, not a candidate core primitive.

## Inputs the binding design must settle

The follow-up design is not self-contained until it answers all of these:

- the signed coordinate type, checked arithmetic, and top-left local origin;
- whether coordinates address a child's outer rectangle, including margin;
- the container's used size for bounded and unbounded `Available` on each axis;
- how descendant `Fill` affects a positioned container's own area claim;
- the exact `Available` derived for a child at positive and negative offsets;
- the separation between child overflow and four-edge container clipping;
- grapheme-atomic clipping and overwrite at every edge;
- back-to-front order and whether rectangular opacity is the only initial mode;
- signed anchor translation and the meaning of fully, partially, and not
  contained regions; and
- the boundary at which world coordinates, camera transforms, transparent
  coverage, connectors, and hit testing require a separate viewport or scene
  design.

## Reproducible sources

The repository sources were inspected at Urushi commit
`49c91cc45ffc4045d302645c6e3b5c5c0d496416` and Noctui commit
`859108e63a89568c907d9099b25ff104c40fb7e9`. The relevant Urushi entry points
are `urushi/src/view/{model,width,height,assemble,resolve}.rs` and
`urushi/tests/anchor.rs`; the relevant Noctui entry points are
`src/view/{view,resolve,resolved_view}.mbt`, `src/buffer/{buffer,placement}.mbt`,
and `src/render/buffer/renderer.mbt`.

External behavior and implementation were inspected at:

- [rataflow `68c175e`](https://github.com/furkankly/rataflow/tree/68c175e9d3d56028bbeb5cb44047db5781d8da10), especially
  [`docs/ARCHITECTURE.md`](https://github.com/furkankly/rataflow/blob/68c175e9d3d56028bbeb5cb44047db5781d8da10/docs/ARCHITECTURE.md),
  [`docs/INTERNALS.md`](https://github.com/furkankly/rataflow/blob/68c175e9d3d56028bbeb5cb44047db5781d8da10/docs/INTERNALS.md),
  [`src/state/render_context.rs`](https://github.com/furkankly/rataflow/blob/68c175e9d3d56028bbeb5cb44047db5781d8da10/src/state/render_context.rs), and
  [`src/ui/canvas.rs`](https://github.com/furkankly/rataflow/blob/68c175e9d3d56028bbeb5cb44047db5781d8da10/src/ui/canvas.rs); and
- [Zoetrope `077707d`](https://github.com/furkankly/zoetrope/tree/077707da679955c0402c39ca992bf56cdc6b0264), especially
  [`src/state/graph.rs`](https://github.com/furkankly/zoetrope/blob/077707da679955c0402c39ca992bf56cdc6b0264/src/state/graph.rs),
  [`src/ui/mod.rs`](https://github.com/furkankly/zoetrope/blob/077707da679955c0402c39ca992bf56cdc6b0264/src/ui/mod.rs),
  [`src/ui/nodes.rs`](https://github.com/furkankly/zoetrope/blob/077707da679955c0402c39ca992bf56cdc6b0264/src/ui/nodes.rs), and
  [`src/handler.rs`](https://github.com/furkankly/zoetrope/blob/077707da679955c0402c39ca992bf56cdc6b0264/src/handler.rs).
