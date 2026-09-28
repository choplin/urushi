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

retained projection
    View --prepare--> retained resolved content
                             --project Viewport-----> ResolvedView
                             --project Viewport-----> ResolvedView
```

The free `resolve(view, available)` operation remains the ordinary path. A view
without a viewport uses the direct width, height, and assembly pipeline and
does not construct a retained scene, spatial index, or tile cache. A view with
a viewport may be resolved once by the same stateless entry point; that path
may eagerly assemble its child and project the result.

Repeated evaluation is opt-in. A stateful resolver may retain an internal
resolved-content artifact for structurally unchanged viewport children. The
artifact contains settled layout geometry and whatever owned paint recipes or
materialized cells are needed to reproduce projection. It is evaluation state,
not semantic data, a `View` node, application state, or another renderer input.
Dropping the resolver drops the artifact.

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

The initial resolver does not evict a materialized region while its content
remains valid; replacing, invalidating, clearing, or dropping the resolver
releases it. This is the guarantee that returning to a previously evaluated
region does not paint it again. A bounded policy and its accounting unit remain
an additive follow-up for measured memory pressure rather than an initial
public enum. The free `resolve` function remains the zero-retention entry point
rather than an implicit default `Resolver`.

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

A retained content entry is reusable while both the content value and every
input that can change its layout remain equal. At minimum these include the
child `View`, relevant finite allocations, text fitting and overflow policy,
and final Canvas surface sizes. A changed width invalidates wrapped rows even
when the child data is unchanged.

Viewport origin and boundary behavior do not invalidate settled content. They
select another rectangle from it. Placement of the projected result in an
ancestor also does not invalidate the local content; it translates the finite
result afterward.

Logical render settings and backend capabilities remain after resolution and
therefore do not invalidate layout content. A Canvas item or sizing value that
compares unequal is changed content and invalidates the Canvas entry under the
same rule as any other child.

The first retained implementation derives reuse from the immutable values and
layout inputs already in the resolution call. It does not add public revision
counters, viewport keys, or application identities merely to make lookup
cheaper. Such a mechanism requires a separately demonstrated need because a
caller-supplied identity can otherwise make stale output appear valid.

## Eager and lazy materialization

Settled geometry does not require every cell to be retained. A resolver may
materialize all content eagerly or create cells lazily for requested
rectangles. Lazy storage may use rows, bands, or two-dimensional tiles; that
choice is not observable.

A positive-width grapheme has one owner at its leading cell even when a cache
boundary crosses it. Queries include enough neighboring information to apply
the atomic clipping rule. Ordered composition likewise remains ordered when
operations come from more than one cache entry.

Retention lifetime is explicit in the choice to use and retain a stateful
resolver rather than in `Viewport`. The stateless path need not allocate cache
metadata at all.

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
