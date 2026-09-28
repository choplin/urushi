# Area Sharing

How the children of a `Row` or `Column` divide the area their container hands
them: distribution by `Length`, the remainder rule, the cross axis, shrinking
when the children need more than the content cap, and how a `Fill` reaches an
allocation reference through auto ancestors.
[`view-model.md`](../view-model.md) summarizes this under "Sizing at a glance";
the clamp each child applies to its own share is
[`box-sizing.md`](box-sizing.md). A `Grid` divides its width by this same rule,
over columns rather than over children — [`grid.md`](grid.md).

## The rule

`Row` hands its width constraint to its children; `Column`, its height
constraint. The constraint distinguishes an optional **allocation reference**
from an optional **content cap**. An ordinary finite `Available` supplies the
same number for both. A projected axis keeps the viewport extent as its
reference but has no child-content cap. On the main axis:

- `Cells` children take their stated size.
- Auto children take their intrinsic size.
- `Fill` children divide what remains of the allocation reference, in
  proportion to their weights. Without a reference they contribute their
  intrinsic size.

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
to divide: the container passes its constraint to every child unchanged, and a
`Fill` length there stretches to the allocation reference. A fixed-width sidebar
spanning the terminal's height is `width(20).height(Length::fill(1))` inside a `Row`,
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
the outer `Length::fill(1)` block above places the capped group.

When the children's assigned sizes — stated `Cells`, intrinsic auto, `Fill`
shares — add up to more than the content cap, children shrink below those sizes:
`Fill` children first, then auto children, then `Cells` children, each
proportionally to size and floored at its own `max(min_width, min-content)`. A
floor that binds freezes that child, and the shortfall falls on the rest — an
iteration over numbers only, settled before any child is assembled. When even
the floors exceed the area, the container resolves larger than its area and the
degenerate safety net of [`box-sizing.md`](box-sizing.md) is what finally
bounds it.

With no content cap, that shrink step does not run merely because the claims
exceed the allocation reference. `Cells` and auto children keep their claims;
`Fill` still divides only the nonnegative remainder of the finite reference.
The container may therefore resolve larger than the reference. This is how a
`Column` inside a vertical `Viewport`, or a `Row` inside a horizontal one,
retains later content for another origin while still giving `Fill` a finite
meaning. Projection, not area sharing, clips the settled result; the exact
composition is [`view-projection.md`](view-projection.md).

A `Grid` column makes the same claim from a different place: its kind comes
from an optional `Length` on the column, its demand and its floor from the
cells beneath it. What the division and the shrink then do with that claim is
unchanged — a column stating `Cells` shrinks last, and still shrinks when
nothing else is left to give.

A `Fill` length resolves against an allocation reference, so it needs one: an
auto box with a `Fill` anywhere among its descendants spans its own available
extent (through its own clamp), and so does each auto ancestor above it, until
an ancestor with a stated `Cells` size stops the propagation and keeps that
size. Under `measure`, where no area exists, `Fill` contributes the intrinsic
size, and weights have no effect.

Only the need for a finite reference propagates through an auto ancestor, not
the descendant's weight. When an otherwise-auto container or Grid track must
turn that dependency into its own claim, it uses `Fill(1)`. Weights compare
siblings stated in the same sharing container; carrying one through arbitrary
ancestors would compare unrelated levels.

## Why `Fill`, and why nothing is renegotiated

Without `Fill`, "a fixed sidebar and main takes the rest" — the most ordinary
full-screen layout — is inexpressible under any non-negotiating rule, which is
why this much is reclaimed from the rejected constraint-solving design.

The boundary against that design stays sharp: a size is decided from an area
and pure measurements of the subtree below it, results flow up once, and the
only iteration is the numeric freeze loop over one axis's floors when an area
is too small. No cross-axis coupling, no size revised once decided, no
propagation of one sibling's resolution into another's content. Measuring a
subtree more than once does not cross that line — a measurement is a question
about that subtree alone, so its answer cannot depend on what a sibling
resolved to. Depending on a sibling's resolution is exactly what a solver's
iteration does.

The remainder is deliberately not redistributed either: a `Fill` child capped
by its own `max_width` leaves the remainder unused rather than triggering
redistribution. Capping a group is an enclosing block's `max_width`, and the
container that resolves below its area is placed by ordinary alignment — the
same structure as CSS's `max-width` with auto margins. The distribution rule
stays one sentence, and the cost is an addition at the call site.

The rule that stays this simple is the division itself: `Fill` weights divide
the remainder directly, so equal weights are an equal split. Per-child grow
factors — CSS flex rather than CSS Grid — were rejected for exactly this: a
grow factor distributes slack *on top of* intrinsic sizes, so two `grow(1)`
children of unequal content do not split an area 50/50, and the flex
`basis: 0` trick exists to cancel what the factor did.

## Rejected designs

- **A constraint-solving layout tree with flex-like grow and shrink.** The
  boundary is drawn under "Why `Fill`, and why nothing is renegotiated".
- **Per-child `grow` factors.** Argued there as well.
- **Redistributing the remainder a capped `Fill` child leaves.** Argued there:
  capping a group is an enclosing block's job.
