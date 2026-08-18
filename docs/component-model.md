# Component Model

This document defines the contract every reusable Urushi component follows:
semantic data on one side, presentation on the other, composed into a
[`View`](view-model.md). The reasoning behind that separation, and the criteria
for applying it to a new component, live in
[`design/component-data-and-style.md`](design/component-data-and-style.md).

## Component style plus component data composes a View

Component data describes what a component contains. A component style describes
how that data is composed into a `View` — the renderer-neutral tree of text,
blocks, rows, and columns, and its logical styles, that a backend later turns
into output:

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

A component style composes a `View`; it does not render output.

## Responsibilities

| Value | Owns | Does not own |
| --- | --- | --- |
| Component data | Semantic content, hierarchy, structural visibility, offsets, and data inspection | `TextStyle` values, marker callbacks, terminal capabilities, or output lifecycle |
| Component style | Logical styles, marker and indentation policy, component layout, and `View` composition | Component content, writers, terminal state, or event loops |
| `ComponentStyles` | Theme-wide default component styles | Runtime component data |
| `View` | The composed tree of text, blocks, rows, and columns, and its logical styles | Terminal capability decisions or output |
| Output adapter | Translation of `View` or logical styles for a concrete backend | Component semantics or application workflow |

The builder that constructs component data — `List::new().item(...)` above —
should describe content and structure rather than appearance. The same data
value should remain usable without a theme and composable with different
component styles.

A component style may perform component-specific layout — aligning summary
labels, composing tree branches — but it does not resolve terminal
capabilities, emit output, or discover the terminal's size: when layout
requires a display width, the caller supplies that constraint explicitly.

## Default styles and local variation

On the style side, themes expose default component styles by reference. A
caller clones a default when one use needs a local variation:

```rust
let numbered = theme
    .components()
    .list()
    .clone()
    .enumerator(arabic_enumerator);

let view = numbered.view(&list);
```

Local presentation belongs to the component style, not to the data builder.
Global customization, by contrast, replaces or derives the style in
`ComponentStyles`.

## Naming

A component style is named `…Style`, not `…Renderer`, because it composes a
`View` rather than rendering output.

## Public API and internal reuse

Similar components keep independent public data models; they do not share
public aliases or traits.

What may be shared is genuinely common implementation, kept private. A private
trait or generic function may own recursive traversal, marker alignment,
multiline continuation, or CJK cell-width handling while each component keeps
independent public data and callback types.

A shared contract is promoted to a public trait only when external callers need
to write generic code over multiple component models. Internal deduplication
alone does not justify a public abstraction.

## Reference components

`List` / `ListStyle`, `Tree` / `TreeStyle`, and `Table` / `TableStyle` are the
reference cases for the data-and-style separation. `Summary` and `Warning`
consume `ComponentStyles` directly.
