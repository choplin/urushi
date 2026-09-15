# Tree Lowering

[`component-model.md`](../component-model.md) places Tree hierarchy in semantic
data and connector geometry in its presentation. This document answers how the
canonical Tree presentation lowers that hierarchy without adding Tree meaning
to the primitive `View` vocabulary.

## Decision

`TreePresentation::compose(&Tree)` applies visibility and child offsets, derives
plain text and root/item styles from each typed value, then binds those
snapshots, the remaining hierarchy, the connector style, one `LineGlyphs`
repertoire, and its indentation width into a comparable owned Canvas item. The
typed policy and its position contract are defined in
[Tree Node Presentation](tree-node-presentation.md). `Tree` and `TreeNode`
contain no connector glyph, callback, style, or resolved coordinate. The Canvas
item and its intrinsic sizing value share the same bound frame, so equal bound
values produce equal measurement and drawing independent of allocation
identity.

The default indentation width is four terminal cells, producing `├── ` and
`└── ` with `LineGlyphs::NORMAL`. A custom width must be at least three cells:
one junction cell, at least one horizontal continuation cell, and one gap
before content. For a node at depth `d`, the connector column and content
origin are:

```text
connector_x = d * indent_width
content_x   = connector_x + indent_width
```

Arithmetic saturates at the coordinate boundary. The resulting content origin
is the boundary between presentation-owned connector space and semantic node
content. How that boundary contributes to intrinsic sizing is defined in
[`tree-width-resolution.md`](tree-width-resolution.md).

For every visible sibling group, the item records a vertical segment at its
connector column from the first node's first row through the last node's first
row, with `LineContinuations::START`. It also records one horizontal segment on
each node's first row, from the connector column through
`connector_x + indent_width - 2`. The remaining cell is the gap before content.
The vertical and horizontal segments that decide a junction are in the same
`LineNetwork`: START supplies upward incidence, a following sibling supplies
downward incidence, and the branch supplies rightward incidence. The first of
several siblings therefore becomes `├`, while a single or final sibling becomes
`└`.

No segment occupies the row above a sibling group, no later command masks a
connector cell, and separate Canvas commands are never expected to union line
incidence. An implementation may collect independently complete groups into
one Tree-wide network, but that grouping is not a public Tree invariant. The
connector has one Tree-wide `TreeRole::Connector` style because topology does
not distinguish an enumerator string from an indenter string, and shared
junction or continuation cells do not have unique node ownership. Custom
one-cell connector repertoires use `LineGlyphs`.

## Reasoning

Canvas postpones both coordinates and drawing until the component's local size
is known, while keeping the resolver unaware of Tree semantics. A single line
network owns every incidence needed to choose each junction glyph, so the
result follows the generic network contract instead of relying on draw order
or cell repair. Binding visibility and offsets first also gives typed node
policy, sizing, and drawing the same hierarchy. Formatting and node-style
callbacks run only while binding; width-dependent measurement and drawing
consume their snapshots.

One connector role matches the retained meaning: style varies by semantic
connector, while junction shape varies by topology. `LineGlyphs` preserves
repertoire customization without allowing callbacks to replace structural
lines with values that no longer obey that topology.

## Rejected alternatives

- **Add `View::Tree`.** This would make the resolver and every backend know a
  semantic component that can instead lower into existing generic primitives.
- **Emit separate vertical and horizontal line commands.** Incidence is united
  only within one `LineNetwork`; separate commands cannot reliably derive tee
  and corner glyphs at their crossings.
- **Draw a phantom row or mask connector cells afterward.** Either approach
  makes topology depend on out-of-group geometry or command order instead of
  describing the retained line directly.
- **Keep enumerator and indenter callbacks and styles.** Arbitrary marker text
  does not express a connected line topology, and the two callbacks split one
  connector meaning into states that the canonical presentation does not use.
