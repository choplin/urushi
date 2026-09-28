# Resolution Reuse

How Urushi evaluates the same view-projection semantics once or repeatedly
without charging retained-layout costs to callers that do not need them.
[`view-projection.md`](view-projection.md) owns what a viewport means; this file
owns only evaluation lifetime, reuse, and invalidation.

## The rule

Urushi provides three evaluation paths with one observable result:

```text
direct
    View ------------------------------------------> ResolvedView

one-shot projection
    View containing Viewport ----------------------> ResolvedView

retained evaluation
    View snapshot --reconcile--> retained subtree artifacts --> ResolvedView
    next snapshot --reconcile unchanged / rebuild changed--> ResolvedView
```

The free `resolve(view, available)` operation remains the ordinary path. A view
without a viewport uses the direct width, height, and assembly pipeline and
does not construct a retained scene, spatial index, or tile cache. A view with
a viewport may be resolved once by the same stateless entry point; that path
may eagerly assemble its child and project the result.

Repeated evaluation is opt-in. A stateful resolver retains one prior immutable
View snapshot for structural reconciliation and internal artifacts for
unchanged subtrees whose settled assembly inputs also remain equal. An artifact
contains a materialized rectangle and the exact settled assembly input that
produced it, including the complete content below a Viewport. It is distinct
from the semantic snapshot, application state, and renderer input. Dropping the
resolver drops both the snapshot and every artifact.

The resolver's state never decides or changes layout. The current `View` and
`Available` are the only layout inputs, and the ordinary width, fit, and height
phases settle them for each non-identical frame before retained assembly is
consulted. Structural layout changes therefore belong in the next immutable
View built by an application — in a TUI, in `Application::view`. A retained
rectangle is reusable only after the current layout pass proves that its local
assembly input is still equal; otherwise it is discarded. Retention cannot
preserve an old layout decision against the current View.

Viewport movement is the first important use, not the identity of the
capability. Two successive immutable View snapshots may differ in one animated
or live subtree while independent siblings remain equal. The resolver rebuilds
the changed branch's materialization and every composition result that depends
on it, while reusing an unchanged sibling whose own settled input is still
equal. The View itself remains a one-frame value; the resolver does not make it
stateful.

The application runtime or another output adapter may own that resolver because
it owns repeated evaluation. An application model is never required to retain
it, identify it, or react to its eviction.

The public shape keeps that choice at the evaluation boundary without fixing a
cache-budget vocabulary before its accounting can be measured:

```rust
pub struct Resolver { /* private retained content */ }

impl Resolver {
    pub fn new() -> Self;
    pub fn resolve(
        &mut self,
        view: &View,
        available: Available,
    ) -> Result<ResolvedView, LayoutError>;
    pub fn clear(&mut self);
}
```

The initial resolver retains one reconciled frame of subtree artifacts. An
entry reached unchanged by the next snapshot survives; an entry whose subtree
or settled input changed is released when that frame succeeds. `clear` and
drop release everything. Complete Viewport child rectangles are materialized
eagerly, so moving away and back while the child remains valid does not paint
it again. A bounded policy and its accounting unit remain an additive follow-up
for measured memory pressure rather than an initial public enum. The free
`resolve` function remains the zero-retention entry point rather than an
implicit default `Resolver`.

## Equivalence is the contract

For equal `View`, layout inputs, and viewport values, direct, one-shot, eager
retained, and lazy retained evaluation produce equal `ResolvedView` values.
Caching may change work and memory use only. It cannot change sizing, clipping,
composition order, anchor rectangles, or visibility.

This equivalence is the implementation oracle for laziness: querying and
materializing only the visible rectangle must match assembling the complete
content rectangle and projecting it afterward. A spatial query may return a
conservative superset of paint operations, but it must preserve their original
order and cannot omit an operation that contributes to the result.

One cell does not necessarily map to one View. Overlap and Canvas composition
can make several operations contribute to it. A lazy evaluator must therefore
recover one deterministic, traversal-ordered sequence containing every
contributor to a queried rectangle, plus enough neighboring information to
settle edge graphemes, and compose that sequence under the ordinary rules.
How it records bounds or locates contributors is private resolver machinery;
public coordinate lookup would expose an optimization representation and make
the caller responsible for evaluation.

## Reuse and invalidation

A retained subtree entry is reusable while both its structural View value and
every settled input that can change its output remain equal. The resolver
compares one linear structural snapshot in tree order, then matches each node's
settled assembly input at the same occurrence. Layout itself is recomputed from
the current View; the comparison authorizes reuse of materialized output rather
than supplying a layout decision. This avoids public identity and avoids
copying a complete descendant tree into every cache entry. A changed width
invalidates wrapped rows even when the child data is unchanged; a changed
sibling does not invalidate an independent subtree when its own inputs remain
equal.

Viewport origin and boundary behavior do not invalidate settled content. They
select another rectangle from it. Placement of the projected result in an
ancestor also does not invalidate the local content; it translates the finite
result afterward.

Logical render settings and backend capabilities remain after resolution and
therefore do not invalidate layout content. A Canvas item or sizing value that
compares unequal is changed content and invalidates the Canvas entry under the
same rule as any other child.

The first retained implementation derives reuse from the immutable values and
layout inputs already in the resolution call. Occurrence position is private
matching machinery, not public identity: duplicate equal subtrees remain
distinct occurrences, and their output is composed in original tree order. A
structural insertion may conservatively lose reuse for later siblings but can
never reuse output from the wrong occurrence. The implementation does not add
public revision counters, viewport keys, or application identities merely to
make lookup cheaper. Such a mechanism requires a separately demonstrated need
because a caller-supplied identity can otherwise make stale output appear
valid.

## Eager and lazy materialization

The initial resolver retains complete rectangles at reuse boundaries:
Viewport children, complete Canvas surfaces, and independent children of
multi-child composition nodes. Unary layout wrappers are recomposed around
those artifacts instead of retaining another copy at every depth. This is the
eager equivalence baseline: it preserves the ordinary assembly and paint order
without making a deeply wrapped tree retain one full rectangle per wrapper. A
later implementation may create cells lazily for requested rectangles. Lazy
storage may use rows, bands, or two-dimensional tiles; that choice is not
observable.

A positive-width grapheme has one owner at its leading cell even when a cache
boundary crosses it. Queries include enough neighboring information to apply
the atomic clipping rule. Ordered composition likewise remains ordered when
operations come from more than one cache entry.

Retention lifetime is explicit in the choice to use and retain a stateful
resolver rather than in `Viewport`. The stateless path need not allocate cache
metadata at all.

## Composition with other retained capabilities

`Resolver` owns only core View evaluation. It does not own an animation clock,
wakeup or coalescing policy, application state, terminal capabilities, or a
graphics protocol transaction. A host composes those responsibilities through
the existing values:

```text
Model + clock -> immutable View snapshot
                         |
                         v
                      Resolver -> ResolvedView
                                      |\
                                      | +-> graphics reconciliation
                                      +----> cell presentation and commit
```

For animation, the host advances semantic state and constructs the next View;
the resolver reuses unaffected subtrees. For terminal graphics,
`urushi-graphics` combines its immutable assets from the View with resolved
anchors, while its host-owned protocol state reconciles upload, placement,
deletion, and failure recovery. These states have different invalidation and
commit rules, so neither is registered inside the resolver and no universal
capability registry is part of the core API.

## Canvas reuse

The complete settled Canvas surface is one valid retained unit. Existing
Canvas items may record and rasterize that whole surface once, after which
viewport movement reuses its cells. A later optimization may retain commands
or spatially indexed output and materialize only an intersection, but it must
remain equivalent to complete Canvas composition, including custom composition
and paint order.

Consequently viewport support does not by itself change `CanvasItem` or require
command bounds. Partial Canvas rasterization is separate work justified only by
measured cost or a concrete surface too large for complete evaluation.

## External data and memory

Retained resolution applies only to the `View` value supplied to it. It does
not promise that an external document, log, data set, or image collection is
represented by one View or held in memory. A caller may lower any finite data
selection into a View and replace that value as its own data policy requires.
Urushi then applies the same direct or retained evaluation rules to that value.

This boundary lets a library user combine semantic virtualization with cell
projection without introducing document positions, loading, prefetch, or
application cache policy into Urushi.

## Why reuse is not mandatory

Making every resolve produce a retained scene would turn an optimization for
changing origins into permanent allocation and indexing work for ordinary CLI
output, static widgets, and one-frame values. Putting cache state in `Viewport`
would also make a pure description own evaluation lifetime.

Separate entry points preserve the simple call, allow a one-shot projection,
and give repeated renderers a place to retain work. They share semantics rather
than forcing one lifetime onto every caller.

## Rejected designs

- **Replace `resolve` with prepare-then-project.** It charges retained-content
  construction to views that have no projection or reuse.
- **Make `ResolvedContent` part of the application model.** It mixes evaluation
  state with semantic state and makes eviction observable to `update`.
- **Require an identity or revision on every viewport.** One-frame projection
  needs neither, and an unchecked identity can preserve stale output.
- **Require lazy Canvas commands in the first viewport implementation.** A
  complete finite Canvas surface already has correct semantics and can be
  cached as one unit.
- **Put retention policy on `Viewport`.** Memory lifetime follows the explicit
  choice to keep a `Resolver`, not the fact that a viewport exists. A bounded
  resolver policy remains additive once its accounting has concrete evidence.
