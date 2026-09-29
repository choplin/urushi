---
title: Lists, tables, trees, and scrollbars
description: Compose structured data and viewport indicators with canonical component presentations from a Theme.
---

Core components separate owned data from presentation. Build the data value,
then ask a `Theme` to compose it into a `View`.

## List

```rust
use urushi::List;

let steps = List::new()
    .item("Compile")
    .item("Test")
    .item("Package");

let view = theme.list(&steps);
```

```text title="Canonical presentation"
• Compile
• Test
• Package
```

`ListItem` supports nested items, visibility, and offsets when the list needs a
hierarchy or windowed subset.

## Text table

```rust
use urushi::Table;

let table = Table::text()
    .headers(["Package", "Status"])
    .row(["urushi", "ready"])
    .row(["urushi-prompt", "ready"]);

let view = theme.table(&table);
```

```text title="Canonical presentation"
┌───────────────┬────────┐
│ Package       │ Status │
├───────────────┼────────┤
│ urushi        │ ready  │
│ urushi-prompt │ ready  │
└───────────────┴────────┘
```

For application structs, derive `urushi::TableRow` or implement the `TableRow`
trait and use `Table<Row>`. Headers and direct text cells are plain text, not
pre-rendered ANSI strings.

## Tree

```rust
use urushi::{Tree, TreeNode};

let tree = Tree::new()
    .root("workspace")
    .child("Cargo.toml")
    .child(
        TreeNode::new("src")
            .child("lib.rs")
            .child("main.rs"),
    );

let view = theme.tree(&tree);
```

```text title="Canonical presentation"
workspace
├── Cargo.toml
└── src
    ├── lib.rs
    └── main.rs
```

## Scrollbar

`Scrollbar` describes one finite viewport using caller-defined units. It does
not own navigation or choose which edge it occupies; the parent layout supplies
those decisions.

```rust
use urushi::{Available, Scrollbar, ScrollbarOrientation, resolve};

let scrollbar = Scrollbar::new(
    ScrollbarOrientation::Vertical,
    100, // complete content length
    25,  // visible viewport length
)
.position(50);

let view = theme.scrollbar(&scrollbar);
let resolved = resolve(&view, Available::size(1, 8))?;
# Ok::<(), urushi::LayoutError>(())
```

The canonical proportional presentation uses the finite eight-row allocation
to produce:

```text title="Canonical vertical scrollbar"
↑
│
│
│
█
█
│
↓
```

Use `ScrollbarPresentation` to select proportional or one-cell marker sizing,
replace the vertical and horizontal glyph sets, or style the thumb, track, and
endpoints independently. Compose the resulting one-cell-cross-axis view beside
the content with an ordinary row or column.

Connectors, enumerators, scrollbar geometry, spacing, and styles belong to the
presentation rather than the data model. Use `theme.list`, `theme.table`,
`theme.tree`, and `theme.scrollbar` for the canonical theme-derived versions.
Reach for `ListPresentation`, `TablePresentation`, `TreePresentation`, or
`ScrollbarPresentation` when the application needs a custom presentation
policy.

## Resolve after composition

Component methods return ordinary views. Combine them with blocks, rows, or
columns before resolving so the entire output participates in one layout pass.
