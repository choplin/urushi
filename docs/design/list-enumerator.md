# List Enumerator Contract

What information a custom List enumerator receives, which items it observes,
and when the presentation evaluates it. Marker placement is a separate issue
defined in [List Marker Layout](list-marker-layout.md).

## Position describes the visible sibling group

`ListPosition` contains a zero-based `index`, the visible sibling count `len`,
and a zero-based nesting `depth`:

```rust
pub const fn new(index: usize, len: usize, depth: usize) -> Self;
pub const fn index(self) -> usize;
pub const fn len(self) -> usize;
pub const fn is_empty(self) -> bool;
pub const fn depth(self) -> usize;
```

The root group has depth zero. Each child group increases depth by one. The
index and length are computed after the group's start and end offsets and
hidden-item filtering are applied. A hidden item excludes its complete subtree,
so neither it nor its descendants create callback positions.

Depth is part of the same value because a marker commonly depends on both
sibling position and nesting level. For example, an outline can use decimal
markers at the root and bullets below it without changing the semantic List:

```rust
fn outline_enumerator(position: ListPosition) -> String {
    if position.depth() == 0 {
        arabic_enumerator(position)
    } else {
        bullet_enumerator(position)
    }
}
```

An indenter callback is not a second way to express that policy. Nesting is
presentation geometry; the enumerator chooses only the marker for the current
item.

## Evaluation belongs to composition

`ListPresentation::compose` evaluates the enumerator once for each visible
item and stores the result in the bound frame. Intrinsic measurement and Canvas
drawing may run more than once, so neither phase evaluates application
callbacks again.

The same composition pass evaluates the typed item presentation. Its formatter
and its Item and Enumerator role-style decisions receive the same
`ListPosition` as the marker and are also stored in the bound frame. The
item-presentation contract and its relationship to the position-only
enumerator are defined in
[List Item Presentation](list-item-presentation.md).

Each callback result is normalized to one semantic line before it is stored.
CRLF, CR, LF, U+2028, and U+2029 become one space so a marker cannot introduce
rows outside the List row algorithm. Spaces supplied by the callback remain
marker content and retain their display width. Alignment cells introduced by
the presentation are geometry, not callback output.

This timing gives one composed `View` stable marker values across repeated
resolution while leaving the semantic `List` independent of presentation
policy. An application that needs new callback results composes a new frame.

## Why not a depth-only callback or separate policies

Sibling numbering requires index and length, while level-dependent bullets
require depth. Omitting any one of them forces callers to capture traversal
state outside the callback, which becomes incorrect when visibility or offsets
change. Separate root and nested callbacks encode one marker choice through two
APIs and cannot express a policy that varies across more than two levels.

`ListPosition::new(index, len, depth)` also lets callers construct every valid
callback context directly. A root-only constructor plus a private depth field
would make public callback behavior impossible to test without building and
composing a List.
