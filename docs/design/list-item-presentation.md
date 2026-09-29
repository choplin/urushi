# List Item Presentation

How List retains typed semantic values while a typed item presentation produces
the plain text and role styles stored in its composed frame. Marker selection
is a separate position-based policy defined in
[List Enumerator Contract](list-enumerator.md).

## List retains one homogeneous value type

`List<T>` owns `ListItem<T>` values recursively. The same `T` at every depth
rules out a child whose value cannot be inspected by its item presentation:

```rust
pub struct List<T> { /* ... */ }
pub struct ListItem<T> { /* ... */ }

impl<T> ListItem<T> {
    pub fn new(value: T) -> Self;
    pub const fn value(&self) -> &T;
}
```

Construction is uniform across value types. `ListItem::new(value)` creates a
node, while `List::new().item(value)` and `items(values)` append values directly
through `From<T> for ListItem<T>`. The existing `item` and `items` operations
also accept configured `ListItem<T>` nodes; no separate recursive construction
API is introduced. An empty List states its value type explicitly when
surrounding context cannot infer it. String receives no separate constructor,
conversion, or default-type rules.

The value remains semantic data. Formatting, List roles, selection, focus, and
attention do not become fields or variants of `ListItem`.

## Typed item presentation is a separate value

`ListPresentation` remains the non-generic List-wide policy stored by
`ComponentTheme`: role defaults, marker selection, and nesting indentation.
`ListItemPresentation<'a, T>` separately owns the typed mapping from one value
and its visible position to text and optional role-style replacements:

```rust
let items = ListItemPresentation::<Task>::new(|task, position| {
    format!("{} ({}/{})", task.title, position.index() + 1, position.len())
})
.item_style(|task, _position, role| {
    task.selected.then(|| match role {
        ListRole::Item => selected_item.clone(),
        ListRole::Enumerator => selected_marker.clone(),
    })
});

let view = theme
    .components()
    .list()
    .compose_with(&tasks, &items);
```

The formatter is required by `ListItemPresentation::new`, so its `T` need not
implement `Display`. `ListItemPresentation::display()` supplies the canonical
formatter when `T: Display`; callers can then configure only per-item styles.
When neither custom formatting nor style overrides are needed,
`ListPresentation::compose` and `Theme::list` use `Display` directly without
constructing an item presentation.

The style callback returns `None` to retain the List presentation's Item or
Enumerator role default. `Some(style)` replaces that complete style rather than
patching or inheriting unspecified properties. One callback uses `ListRole` so
both role decisions see the same value and position contract.

`ListItemPresentation<'a, T>` owns its callbacks behind `Arc` so the public type
depends on the item type rather than each closure's concrete type. It is
cloneable but deliberately not comparable: callback behavior has no structural
equality. This erasure uses no `Any`, downcast, or runtime type mismatch.
`ListPresentation` itself remains an ordinary cloneable and comparable Theme
value, consistent with the Tree and Table presentation accessors.

## Composition snapshots policy

Visibility and each sibling group's offset are applied before its
`ListPosition` values are assigned. For each remaining item, composition
evaluates its formatter once and its style policy once for each List role. It
also evaluates the position-only enumerator once, normalizes the marker, and
stores text, both resolved styles, marker, and geometry inputs in the bound
frame.

Intrinsic measurement and Canvas drawing read only that frame. They never
borrow `T` and never invoke the item presentation or enumerator again. Repeated
resolution of one composed `View` therefore cannot observe changing
application state; the application composes a new frame when its state changes.

## Why the enumerator remains separate

An enumerator chooses the marker system for a visible sibling group: bullet,
Arabic, alphabetic, Roman, or another position-derived sequence. Per-item
presentation answers a different question: how this typed value appears under
the Item and Enumerator roles. Keeping the enumerator position-only preserves
one reusable numbering policy and the existing marker-layout contract. An item
style callback can still change the marker's complete style from the value
without turning the semantic List into presentation state.

## Rejected designs

- **Store `String` plus application metadata.** Any fixed metadata vocabulary
  immediately makes generic List data application-specific; erased metadata
  adds downcast failure instead of ruling mismatches out.
- **Require `Display` on List data.** Display is only the canonical
  presentation. A formatter strategy must be able to present types whose
  general-purpose `Display` spelling is absent or inappropriate for this view.
- **Parameterize `ListPresentation` by the formatter or item type.** That makes
  the Theme-owned List-wide value change type for an item-formatting concern and
  breaks the uniform `components().list()` / `tree()` / `table()` /
  `scrollbar()` access shape.
- **Require a public presenter trait.** Formatting and style selection are
  ordinary functions of `&T` and `ListPosition`; asking every application to
  name and implement a strategy type adds ceremony without expressing another
  invariant.
- **Implement presentation on `T`.** One trait implementation would couple the
  semantic value to urushi and permit only one List-specific presentation per
  type. Keeping `ListItemPresentation<T>` separate allows multiple views and
  Theme-derived captured styles.
- **Put typed callbacks directly in `ListPresentation`.** A Theme cannot store
  one value containing callbacks for every possible `T`. Keeping their erased
  storage in `ListItemPresentation<T>` preserves the type-independent Theme
  boundary.
- **Put presentation flags on `ListItem`.** Selected, focused, and attention
  states belong to the application value or item presentation. Adding them to the
  semantic node would privilege one UI state machine and couple every
  alternate presentation to it.
