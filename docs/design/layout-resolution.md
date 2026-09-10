# Layout Resolution

The procedure that applies the sizing rules, node by node: what each node does
with the area it receives, what it hands its children, and the order the rules
apply in. [`view-model.md`](../view-model.md) states the shape under "How a
node resolves"; the rules themselves are [`box-sizing.md`](box-sizing.md),
[`area-sharing.md`](area-sharing.md), and [`overflow.md`](overflow.md).

## The procedure

Every node receives an area and returns the size it resolved to: the area
flows down, and the size flows back up. What each node does with the area it
receives — and what it hands its own children — is fixed.

The extents the clamp needs — a subtree's max-content and min-content widths —
depend on the subtree alone, so they are read off the tree once, before any
area is handed down. One question does have to be asked of a subtree again: the
height a `Column` divides among its children, because a height is what fitting
the content produced. That question is pure — its answer depends only on the
subtree and the area it is asked about, never on what a sibling resolved to —
so asking it is not constraint solving.

What never happens is renegotiation: a size, once decided, is not revised in
the light of what a child or a sibling resolved to, and no node is assembled
twice. That is the boundary against a constraint solver, drawn in
[`area-sharing.md`](area-sharing.md) under "Why `Fill`, and why nothing is
renegotiated".

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

**`Canvas`** carries exactly one sizing mode. Its default viewport mode settles
each axis from a finite parent allocation or, when that axis is unbounded, its
explicit extent. An intrinsic mode supplied explicitly by a built-in
presentation reports width demand and floor; after the parent selects that
width, it reports height demand and floor. Neither mode inspects Canvas items
to determine size. After both axes are final, Canvas assembly creates a local
context and asks items to record commands in order. It then rasterizes one
command through the common contract, applies that command's cell contributions
through its composition, and releases the output before proceeding to the next
command. The compositor clips at all four edges and returns the cells and
translated anchors. A placed View resolves once in its complete local
geometry before projection; Canvas clipping never revises its size or reflows
it. The detailed contract and walkthrough are in [`canvas.md`](canvas.md).

The order settles two questions that would otherwise be ambiguous. A
`max_height` bounds the box *before* `vertical_align` places content inside
it, not after. And no bound ever reaches the frame, because steps 2 and 4
closed it at the used size before there was anything to cut.

## Why the axes resolve in different orders

The axes are asymmetric on purpose. Width is decided before the content
because wrapping needs a width to wrap to; height is decided after it because
wrapping is what determines the row count. This is why a narrower box can be a
taller one, and why a `height` cannot be met by reflowing: the rows already
exist when the height applies, so the excess clips.

It also means a box does not shrink to the longest line its own wrapping
produced. A `max_width(9)` box whose content reflows to seven cells stays nine
wide: narrowing it to seven would be a second width decision derived from the
content the first one produced, and sizes flow down only once. CSS's
shrink-to-fit resolves the same way, for the same reason.

## Why the no-solver boundary is drawn at revision, not repetition

An earlier statement of the boundary was "no node is laid out twice". That
forbade the pure re-measurement a `Column` needs to divide its height, which
does not depend on any sibling and so is not what a solver does. The boundary
is therefore stated as "a decided size is never revised, and no node is
assembled twice": repeating a measurement of a subtree is inside the model,
revising a decision in the light of a sibling's outcome is not.

## What stays outside layout

The rules above size a box; conditional structure is not a sizing property.
"Hide the sidebar when the terminal is narrow" and "stack vertically below 80
cells" are decisions about which tree to build, made by the application's view
function, which holds the size that `resolve` will be given. The model's
obligation is that breakpoints are computable — `measure` is public, the
minimums an application declares are its own, and `BlockStyle::frame_size`
exposes a box's frame overhead — not that trees rewrite themselves.

CSS cannot express these decisions in layout properties either — they live in
media queries, a layer outside layout — and a view function in the Elm
Architecture (TEA) style already holds the size `resolve` will be given, so
the branch costs nothing. A declarative priority-collapse vocabulary would
re-open negotiation for a case the branch already covers.
