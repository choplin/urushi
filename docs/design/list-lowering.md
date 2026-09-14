# List Presentation Lowering

Which renderer-neutral primitive carries the canonical List presentation from
area-independent composition through width-dependent resolution. The marker
and row algorithms are specified separately in
[List Marker Layout](list-marker-layout.md) and
[List Width Resolution](list-width-resolution.md).

## One intrinsically sized Canvas item

`ListPresentation::compose` binds one area-independent List frame. It installs
an intrinsic sizing value derived from that frame on a Canvas and adds one item
that records the resolved rows as generic Canvas text or cell contributions.
The frame imports the [enumerator](list-enumerator.md) and
[marker-placement](list-marker-layout.md) contracts rather than redefining
them.

The sizing value implements the List's
[width-resolution contract](list-width-resolution.md). Once the Canvas viewport
is final, the item records that contract's rows as drawing commands. Canvas
remains responsible for command composition and clipping and never inspects
List semantics.

## Why Canvas fits the contract

List needs an area-independent width claim followed by width-dependent height,
continuations, and drawing. Canvas intrinsic sizing already represents that
sequence for component presentations. Reusing it keeps the recursive algorithm
private to `ListPresentation`, adds no semantic `View` variant, and requires no
new public layout protocol.

The other viable representations do not cover the complete sequence:

- **Nested Row and Column.** They can place one marker beside one text value,
  but cannot share a marker track among siblings or distinguish a first row
  from continuations that appear only after width selection.
- **Grid.** It can align a flat marker column, but recursive groups have local
  tracks and each item produces an area-dependent number of rows. Nested Grids
  do not remove that staged-height dependency.
- **Public shared tracks or Grid spans.** They expose a general constraint
  vocabulary for a component-local algorithm without another external generic
  consumer.
- **A List-specific `View` node.** It moves semantic component policy into the
  closed primitive vocabulary and couples every resolver to List evolution.
- **A generic custom-layout trait.** It publishes a second escape hatch for the
  same staged measurement already supplied by intrinsic Canvas sizing.

Tree may also choose Canvas, but sharing a carrier does not merge component
contracts. A Tree presentation can record connector topology as a
`LineNetwork`; the List item records marker-and-flow rows. Canvas sees only its
generic commands in either case.
