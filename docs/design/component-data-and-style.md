# Component Data and Style

This document defines how reusable Urushi components separate semantic data
from presentation. It is a design criterion for new component APIs.

## Rule

Component data describes what a component contains. A component-specific style
describes how that data is composed into a renderer-neutral `View`:

```text
component style + component data -> View
```

Composition borrows both inputs and owns no terminal I/O:

```rust
let list = List::new()
    .item("Define the API")
    .item(ListItem::new("Implement").items(["model", "view"]));

let view = theme.components().list().view(&list);
```

The style value composes a `View`; it does not render output. Names such as
`ListStyle` and `TreeStyle` therefore fit this role better than `ListRenderer`
or `TreeRenderer`.

## Responsibilities

| Value | Owns | Does not own |
| --- | --- | --- |
| Component data | Semantic content, hierarchy, structural visibility, offsets, and data inspection | `Style` values, marker callbacks, terminal capabilities, or output lifecycle |
| Component style | Logical styles, marker and indentation policy, component layout, and `View` composition | Component content, writers, terminal state, or event loops |
| `ComponentStyles` | Theme-wide default component styles | Runtime component data |
| `View` | Composed logical lines, spans, and styles | Terminal capability decisions or output |
| Output adapter | Translation of `View` or logical styles for a concrete backend | Component semantics or application workflow |

Data builders should describe content and structure rather than appearance.
The same data value should remain usable without a theme and composable with
different component styles.

Themes expose component defaults by reference. A caller clones a default when
one use needs a local variation:

```rust
let numbered = theme
    .components()
    .list()
    .clone()
    .enumerator(arabic_enumerator);

let view = numbered.view(&list);
```

Local presentation belongs to the component style, not to the data builder.
Global customization replaces or derives the style in `ComponentStyles`.

## When to introduce a component style

Introduce a dedicated component style when either condition holds:

- callers need to build or reuse the data independently of its presentation;
  or
- callers need to configure or reuse the presentation policy independently,
  including both theme defaults and per-use variations.

Nested inheritance, custom markers, and component-specific layout are strong
signals that presentation is an independent policy. Simple, short-lived
components may continue to accept `ComponentStyles` directly when neither the
data nor presentation has an independent reuse requirement.

Do not introduce a dedicated style merely for symmetry. When uncertain, ask
whether callers may reasonably build the data before choosing a theme or show
the same data in two different ways.

`List` / `ListStyle`, `Tree` / `TreeStyle`, and `Table` / `TableStyle` are the
current reference cases.
`Summary` and `Warning` remain direct consumers of `ComponentStyles`.

## Public API and internal reuse

Similar components do not need to share a public data model. Public aliases or
traits couple their future evolution even when their current implementation is
identical.

Share genuinely common implementation privately. A private trait or generic
function may own recursive traversal, marker alignment, multiline continuation,
or CJK cell-width handling while each component keeps independent public data
and callback types.

Promote a shared contract to a public trait only when external callers need to
write generic code over multiple component models. Internal deduplication alone
does not justify a public abstraction.

When reviewing a component API, verify that the composition direction remains
borrowed component style plus borrowed data to `View`, performs no output, and
does not couple otherwise independent public models through implementation
reuse.
