# Tree Node Presentation

How Tree retains typed semantic values while a typed node presentation produces
the plain text and node styles stored in its composed frame. Connector topology
and connector styling remain Tree-wide policy, as defined in
[Tree Lowering](tree-lowering.md).

## One value type covers root and descendants

`Tree<T>` owns an optional root value and recursive `TreeNode<T>` children. The
same `T` throughout the hierarchy rules out a root or child that cannot be
inspected by the configured node presentation:

```rust
pub struct Tree<T> { /* ... */ }
pub struct TreeNode<T> { /* ... */ }

impl<T> TreeNode<T> {
    pub fn new(value: T) -> Self;
    pub const fn value(&self) -> &T;
}
```

Construction is uniform across value types. `TreeNode::new(value)` creates a
node, `Tree::new().root(value)` supplies the optional root, and `child` and
`children` accept raw values through `From<T> for TreeNode<T>` or configured
`TreeNode<T>` values. An empty Tree states its value type when surrounding
context cannot infer it. String receives no separate constructor, conversion,
or default-type rule.

The value remains semantic data. Formatting, Tree roles, selection, focus, and
attention do not become fields or variants of `TreeNode`.

## Position distinguishes the root

`TreePosition` is either `Root` or `Child { index, len, depth }`. The root has no
sibling group, so representing it as a synthetic child position would make a
state that does not exist in the semantic Tree. For each child group, offsets
and hidden-node filtering run before zero-based `index` and sibling `len` are
assigned. Top-level children have depth zero, matching their canonical
connector column; each nested group increments depth by one.

Tree has its own position type rather than borrowing `ListPosition`. The two
components currently share some positional facts, but their root semantics and
future presentation needs remain independent public contracts.

## Typed node presentation is a separate value

`TreePresentation` remains the non-generic Tree-wide policy stored by
`ComponentTheme`: root and item style defaults, connector style and glyphs, and
indentation geometry. `TreeNodePresentation<'a, T>` separately owns the typed
mapping from each root or child value and its `TreePosition` to text and an
optional complete text-style replacement:

```rust
let nodes = TreeNodePresentation::<Entry>::new(|entry, position| match position {
    TreePosition::Root => format!("{} (root)", entry.label),
    TreePosition::Child { depth, .. } => format!("{} @ {depth}", entry.label),
})
.per_node_style(|entry, _position| entry.selected.then(|| selected.clone()));

let view = theme
    .components()
    .tree()
    .compose_with(&tree, &nodes);
```

The formatter required by `TreeNodePresentation::new` allows `T` not to
implement `Display`. `TreeNodePresentation::display()` supplies the canonical
formatter when `T: Display`; callers can then configure only per-node styles.
When neither custom formatting nor style overrides are needed,
`TreePresentation::compose` and `Theme::tree` use `Display` directly.

The style callback returns `None` to retain the `TreePresentation` root default
for `TreePosition::Root` or item default for `TreePosition::Child`. `Some(style)`
replaces that complete text style rather than patching or inheriting unspecified
properties.

`TreeNodePresentation<'a, T>` owns its callbacks behind `Arc`, so the public
type depends on the node type rather than each closure's concrete type. It is
cloneable but deliberately not comparable: callback behavior has no structural
equality. The callbacks may borrow local state and need not be `Send`, `Sync`,
or `'static`. This erasure uses no `Any`, downcast, or runtime type mismatch.
`TreePresentation` remains cloneable and comparable as a Theme value.

## Composition snapshots application policy

For each visible root or child, composition evaluates the formatter once and
the node-style policy once, then stores their text and style results in the
bound frame. Intrinsic measurement and Canvas drawing read only that frame.
They never borrow `T` and never invoke application callbacks again. Repeated
resolution of one composed `View` therefore cannot observe changing
application state; the application composes a new frame when its state changes.

Formatting is area-independent, while wrapping remains area-dependent. The
snapshot retains plain text, and the existing Tree row planner wraps it only
after the parent selects the Canvas width.

## Connectors are not owned by individual nodes

Per-node policy replaces root or item text styles only. `TreeRole::Connector`,
`LineGlyphs`, and indentation stay in `TreePresentation`. A junction cell may
combine a node's horizontal branch with a vertical line that connects several
siblings, and a continuation may visually serve an entire descendant group.
There is no single node whose style can own those shared cells without changing
the connector model or making drawing order observable.

A future named presentation that draws no connectors can consume the same
typed `Tree<T>` and choose its own structural policy. It does not justify a
shared appearance abstraction or connector state in the semantic nodes before
that concrete presentation exists.

## Rejected designs

- **Keep the root as String while typing children.** One formatter could not
  inspect the complete semantic hierarchy, and an application would need a
  separate root identity channel.
- **Give the root a synthetic sibling index.** It has no sibling group, so the
  representation would admit position facts with no semantic meaning.
- **Require `Display` on Tree data.** Display is the canonical presentation,
  not a requirement of the semantic value or a custom formatter.
- **Parameterize `TreePresentation` by `T`.** That would make the Theme-owned
  Tree-wide policy change type for a node-formatting concern.
- **Put typed callbacks or application flags on `TreeNode`.** Either choice
  couples reusable hierarchy to one presentation or application state machine.
- **Allow per-node connector styles.** Connected-line cells do not have unique
  node ownership, so this would make shared topology depend on arbitrary
  ownership or draw order.
- **Introduce a common List/Tree presenter or position abstraction.** Similar
  callbacks do not erase the components' different semantic contracts.
