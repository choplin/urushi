# Layout Resolution

The procedure that applies the sizing rules, node by node: what each node does
with the area it receives, what it hands its children, and the order the rules
apply in. [`view-model.md`](../view-model.md) states the shape under "How a
node resolves"; the rules themselves are [`box-sizing.md`](box-sizing.md),
[`area-sharing.md`](area-sharing.md), and [`overflow.md`](overflow.md).
Viewport projection after those sizing decisions is defined in
[`view-projection.md`](view-projection.md).

## The procedure

Every node receives an area and returns the size it resolved to: the area
flows down, and the size flows back up. What each node does with the area it
receives — and what it hands its own children — is fixed. Internally each axis
distinguishes the finite reference used by `Fill` and area sharing from the cap
applied to content claims. A normal `Available` initializes both fields to
the same value; only a projected axis deliberately keeps the reference while
removing the child cap.

### Constraint propagation

The two fields remain distinct through every node. A finite reference does not
silently become a cap merely because a node forwards or subtracts it.

| Node and axis | Rule |
| --- | --- |
| Root | A finite `Available` initializes both reference and cap to its extent; an unbounded axis initializes neither. |
| `Text` width | The cap controls fitting and wrapping. A reference without a cap does not wrap an intrinsic line. |
| `Text` height | The cap limits surviving rows. A reference without a cap leaves all fitted rows in the content extent. |
| `Block` width | `Fill` reads the reference; stated and auto claims are limited only by the cap and the box bounds. Once width settles, the child receives the inner used width as both reference and cap because the Block has established that content box. |
| `Block` height | Before height settles, border, padding, and margin are subtracted independently from the inherited reference and cap and the child receives both remaining fields. The child's rows then determine the auto claim; `Fill`, stated size, bounds, and the inherited cap settle the Block height. The final inner height clips cells and anchor visibility without reflow. |
| `Row` main axis / `Column` main axis | Claims and `Fill` shares are computed from the reference and shrink only against the cap. Each resulting child assignment is an established local extent and reaches that child as both reference and cap. With no cap, auto and stated claims beyond the reference are preserved. |
| `Row` / `Column` cross axis | The incoming reference and cap pass through independently. |
| `Grid` width | An explicit column `Length` supplies its kind. An unstated column becomes `Fill(1)` when any cell carries an unresolved horizontal area dependency, and auto otherwise. Column claims use the reference and shrink only against the cap; each selected width reaches its cells as both fields. Explicit `Cells` stops the dependency. |
| `Grid` height | A row becomes `Fill(1)` when any cell carries an unresolved vertical area dependency, and auto otherwise. Fill rows divide the reference after auto-row demands; each selected row height reaches its cells as both fields. |
| `Canvas` | Viewport sizing consumes a finite reference, or its explicit extent on an unbounded axis, and establishes the resulting finite surface as a local clip. Intrinsic sizing contributes claims that the surrounding constraint resolves normally. |
| `Viewport` projected axis | Its bottom-up requirement is a `Fill(1)`-equivalent, zero-floor claim. A finite incoming reference becomes the viewport extent; the child receives that reference with no cap, and the settled result is then projected and clipped to the extent. Missing reference after top-down allocation is an error. |
| `Viewport` unprojected axis | Both fields pass through unchanged. |

A node's own stated size or bound can therefore establish a smaller local box,
but a transparent auto container cannot recreate a cap that a surrounding
Viewport deliberately removed. This is what preserves later rows and columns
through nested Blocks while retaining finite `Fill` behavior.

The extents the clamp needs — a subtree's max-content and min-content widths —
depend on the subtree alone, so they are read off the tree once, before any
area is handed down. One question does have to be asked of a subtree again: the
height a `Column` divides among its children, because a height is what fitting
the content produced. That question is pure — its answer depends only on the
subtree and the area it is asked about, never on what a sibling resolved to —
so asking it is not constraint solving.

This bottom-up requirement pass does not call public `try_measure` on each
subtree. An area-dependent node records that dependency in its claim so an auto
ancestor can propagate it and a finite ancestor can satisfy it during the
top-down pass. A Viewport projected axis records a `Fill(1)`-equivalent claim
with zero floor. Only after constraints have been routed does validation reject
a projected axis that received no finite reference. The same distinction lets
Canvas viewport sizing participate in a parent before its final extent exists.

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
5. **Clip the content** to that height, from the bottom, intersecting each
   anchor's visible region with the surviving content rectangle while leaving
   its logical rectangle complete. `vertical_align` places slack; it has
   nothing to say when there is none.
6. **Draw the frame** at the used size, and the margin outside it.

**`Row`** gives each child its width by the distribution rule and passes its
own height constraint to every child unchanged. **`Column`** does the same with
the axes swapped. `Fill` shares use the allocation reference; shrinking runs
only against a content cap. A projected axis therefore keeps automatic and
stated claims beyond its reference instead of shrinking them to the viewport.

`resolve` applies the degenerate safety net once, to the finished rectangle,
and applies that final crop to accumulated anchor visibility as well;
`try_measure` runs these same rules with no area at all and stops at the size
they settle, which is what makes an intrinsic size the same computation as a
bounded one rather than a second rule. It reports a Viewport or viewport-sized
Canvas whose required finite extent is missing; `measure` is the convenience
form that panics on that invalid request.

**`Canvas`** carries exactly one sizing mode. Its default viewport mode settles
each axis from a finite parent allocation or, when that axis is unbounded, its
explicit extent. An intrinsic mode supplied explicitly by a built-in
presentation reports width demand and floor; after the parent selects that
width, it reports height demand and floor. Neither mode inspects Canvas items
to determine size. After both axes are final, Canvas assembly creates a local
context and asks items to record commands in order. It then rasterizes one
command through the common contract, applies that command's cell contributions
through its composition, and releases the output before proceeding to the next
command. The compositor clips cells at all four edges; anchors retain their
complete logical rectangles while their visible intersections are clipped to
the local surface, then both coordinates are translated. A placed View resolves
once in its complete local geometry before projection; Canvas clipping never
revises its size or reflows it. The detailed contract and walkthrough are in
[`canvas.md`](canvas.md).

**`Viewport`** receives a finite extent on each projected axis. It supplies the
extent as the reference for area-dependent child sizing without using it as a
child-content cap, resolves the child's complete local geometry, and only
then translates and intersects cells and anchors using the caller-selected
origin and boundary behavior. An unprojected axis follows the ordinary child
rules. This adds no renegotiation: projection selects from settled content and
never asks the child to reflow for a changed origin. The exact per-axis and
nested rules are in [`view-projection.md`](view-projection.md).

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
