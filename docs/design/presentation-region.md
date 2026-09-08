# Presentation Regions

How a presentation can retain an area-dependent layout algorithm without
becoming a semantic `View` node: composition binds semantic data and
presentation policy into an owned `RegionPlan`, the ordinary layout pass gives
that plan its local available area, and the plan resolves to the same
renderer-neutral scene as every built-in primitive. The architectural boundary
is summarized in [`component-model.md`](../component-model.md) and the node in
[`view-model.md`](../view-model.md).

## The contract

A presentation still composes a `View` without receiving `Available`. It may
construct a tree entirely from built-in primitives, or place one owned plan in
a region:

```text
semantic data + concrete presentation [+ frame input]
                         |
                      compose
                         v
                 View::Region(RegionPlan)
                         |
              resolve(local Available)
                         v
                    ResolvedView
```

`RegionPlan` is an opaque, cloneable, comparable value that erases one concrete
implementation of the public region-layout contract. That implementation is a
bound presentation program: it may own component data, presentation policy,
styles, and immutable frame input, but accepts no further semantic argument
when layout invokes it. The exact Rust names of the sizing records may follow
the implementation, but the capability boundary is:

```rust
pub trait RegionLayout {
    fn width_requirements(&self) -> WidthRequirements;
    fn height_requirements(&self, width: usize) -> HeightRequirements;
    fn resolve(&self, available: Available) -> ResolvedView;
}

impl RegionPlan {
    pub fn new<P>(plan: P) -> Self
    where
        P: RegionLayout + Debug + PartialEq + Send + Sync + 'static;
}
```

The wrapper performs type erasure. Equality first requires the same concrete
plan type, then delegates to that type's `PartialEq`; different concrete types
are unequal. Cloning the wrapper preserves the same immutable plan value. The
public implementer writes an ordinary named plan type and its value equality,
not an erased `eq_dyn` method. An implementation may use shared ownership
internally, but sharing is an ownership choice rather than the meaning of
equality.

`View::region(plan)` is the ordinary constructor and performs the same
erasure. Canonical component calls hide it:

```rust
let table_view = theme.table(&table);
let custom_view = View::region(MyLayout::new(/* ... */));
```

Most callers use the first form. The second is the extension point for a
presentation whose layout cannot be expressed faithfully as a tree of the
built-in primitives.

## Measurement and resolution

A region participates in the same width-first layout as every other node. It
reports the demand it has with no width bound and the floor below which its
content cannot be fitted. Once its parent has selected a width, it reports the
height demand and floor at that width. Only then does it resolve within the
local `Available` derived by its parent.

The three operations must agree:

- repeating one operation with equal inputs produces an equal result;
- reported requirements depend only on the plan value and their stated input;
- resolution uses the same width and height rules that measurement reports;
- the resolved rows and size obey the `ResolvedView` invariants;
- a bounded result remains within the local area except for the same explicit
  unbreakable-floor cases permitted to built-in primitives; and
- anchors returned by a plan are relative to the region and are translated and
  clipped by its ancestors like any other anchor.

The parent still owns allocation. A plan cannot inspect the terminal, the root
area, a Ratatui `Rect`, its siblings, or its eventual absolute position. The
area passed during `RegionLayout::resolve` is the region's local bound after
sibling sharing, not an area supplied to presentation composition.

The requirements records are constructed through checked public values. They
cannot state a floor greater than their demand, a zero fill weight, or another
claim the parent allocator cannot honor. Region uses the same claim vocabulary
as built-in nodes rather than exposing the resolver's private intermediate
types.

`Block` remains the only primitive that adds box geometry. A region has no
border, margin, padding, stated `Length`, alignment, or overflow policy of its
own. A caller that needs those properties wraps it:

```rust
View::block(panel, View::region(plan))
```

A borderless region needs no otherwise-empty `Block`. This keeps one box model
and lets `BlockStyle` state whether a region fills, clips, or aligns within its
parent in exactly the same way as any other child.

## Constructing the resolved rectangle

A public `RegionLayout` must have a public, checked way to produce its
`ResolvedView`. The current crate-private constructors for `StyledGrapheme` and
`ResolvedView` therefore cannot remain the only construction path once Region
is implemented.

The Region API supplies a row-oriented builder or equivalent checked value
constructors. It accepts plain grapheme text with logical `TextStyle`, derives
display width through Urushi's one grapheme-width implementation, requires all
completed rows to have the reported rectangle width, and accepts anchors in
region-local coordinates. It produces `ResolvedView` only after those
invariants hold. A plan never supplies a claimed grapheme width or constructs
backend cells.

This is not the positioned Canvas API. Region output describes completed rows
inside one local rectangle; it does not establish public overlap, z-order,
absolute-coordinate, or backend-buffer operations. Those remain owned by the
separate positioned-layout design.

## What a plan may contain

Composition binds every input the concrete presentation needs into the plan.
The plan may own a component snapshot, presentation policy, styles, viewport
origin, camera, selection, normalized content, or another immutable one-frame
input. It may interpret those values when local width or height affects the
presentation choice. Requiring composition to translate every possible choice
into a closed declarative vocabulary would recreate the layout DSL Region is
intended to avoid.

For a Table, the bound plan may own the selected Table data and
`TablePresentation`. Its area-dependent algorithm decides column widths,
applies header and body styles, selects the presentation's concrete rules, and
draws directly in the region. The semantic boundary is opacity rather than
absence: `View`, the core resolver, and output adapters cannot inspect the
Table, match on a header, or branch on the plan's concrete type. Only that
Table presentation interprets its own bound inputs.

The canonical Table plan preserves the component's public presentation
choices: visible-row offset, total width, cell padding and alignment, per-cell
style hooks, `Border` preset or custom glyphs, four outer-edge switches, and
header, row, and column rule switches. Its current data model has no row or
column span, so the plan introduces neither; a future colspan begins as a Table
data and presentation decision rather than widening Grid.

The plan owns the complete line network it draws. An omitted outer edge or
internal rule occupies no cell. A selected edge or rule reserves one row or
column even when its glyph is a blank, so changing visible glyphs cannot change
geometry. Once the selected rules and final track sizes are known, the plan
derives every straight, corner, tee, and cross glyph from the incident
directions. No cell owns or supplies a junction. These rules preserve normal,
rounded, thick, double, ASCII, hidden, booktabs, Markdown, and custom Table
looks without placing any of their vocabulary on Grid.

A List or Tree presentation need not use a region when ordinary `Text`, `Row`,
`Column`, and `Block` nodes express its chosen structure. Region is a capability,
not a mandatory intermediate representation for components.

## Value and execution rules

A region plan is executable layout policy carried as a value. That combination
requires a stricter contract than a passive style value:

- it owns every bound input; the erased plan is `'static` and does not retain a
  borrow from the call to `compose`;
- it is `Send + Sync`, preserving those auto traits for the containing `View`;
- it is immutable after construction;
- measurement and resolution are pure and deterministic;
- it performs no terminal I/O and reads no ambient terminal capability;
- it emits logical `TextStyle` values rather than ANSI or backend cells;
- its `PartialEq` compares the presentation, component snapshot, frame input,
  and every other value capable of changing measurement, graphemes, styles, or
  placements;
- caches and other operational details do not affect equality or output; and
- if the concrete plan stores a closure or another value without structural
  equality, that plan supplies a lawful manual `PartialEq` for the observable
  plan value.

`PartialEq` does not compare function addresses or resolved output. The
concrete plan's type identifies the implementation, and its value equality
identifies the inputs to that implementation. This preserves structural
equality for `View` without requiring a declarative layout DSL or closing the
set of presentation algorithms.

## Why the extension point is below presentation

Presentations do not share an input contract. A Table presentation consumes a
Table, a Tree presentation consumes a Tree, and an interactive presentation
may also consume its own immutable frame snapshot. A common `Presentation`
trait would erase useful distinctions before the semantic interpretation has
happened.

Region plans do share a contract because their differing inputs have already
been bound into owned values. Every plan answers the same questions the parent
layout needs: horizontal requirements, height at a selected width, and
renderer-neutral output inside a local area. The trait therefore generalizes
layout participation after binding; it does not impose one input signature on
presentations or expose component identity to the resolver.

## Why the region is an open primitive

A closed `RegionStrategy` enum would preserve derived equality, but every new
presentation algorithm would require a new core variant and a new resolver
branch. A sufficiently general closed instruction set would instead become a
layout DSL capable of encoding Table, List, Tree, Graph, and future
presentations. That moves complexity into an interpreter without removing it.

Type-erased concrete plans keep the primitive vocabulary small and let a
presentation own its specialized algorithm. The generic constructor enforces
the value requirements, while the erased wrapper implements `Clone`, `Debug`,
and `PartialEq` for `View`. Urushi and Noctui expose the same open region-layout
capability and enforce the same behavioral invariants; their language-specific
mechanisms for type erasure and dynamic equality may differ.

## Rejected designs

- **Put `Presentation` itself in `Block`.** `Block` would gain component
  interpretation and a second kind of child, while every other container would
  still be unable to compose the presentation directly. A region is an
  ordinary `View` child and `Block` remains only a box.
- **Give `compose` the root `Available`.** A nested presentation does not know
  its share until sibling allocation. Region resolution receives the correct
  local area after that allocation.
- **Add `View::Table`, `View::Tree`, or another semantic node.** The core
  resolver would become a registry of component concepts. An erased region is
  dispatched through one layout contract and exposes no component branch to
  the resolver.
- **Require every presentation to lower to built-in nodes.** A Table would have
  to turn its own area-dependent column and rule calculation into another
  primitive's public vocabulary, making that primitive a de facto presentation
  intermediate representation.
- **Use a closed strategy enum or declarative layout DSL.** The former closes
  extension over the same algorithms Region is intended to admit; the latter
  must grow enough instructions to encode arbitrary presentation algorithms.
- **Compare plan allocation or function addresses.** Two separately composed
  plans with equal observable inputs would compare unequal, while a mutable
  plan at one address could change output and still compare equal.
- **Let Region duplicate `BlockStyle`.** Border, padding, sizing, alignment, and
  overflow would acquire a second implementation and could disagree with the
  box model surrounding every other primitive.
