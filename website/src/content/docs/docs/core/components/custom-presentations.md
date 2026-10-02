---
title: Customize component presentations
description: Replace component-wide policy or typed value formatting without changing semantic data.
---

There are two customization levels.

## Change component-wide presentation

Clone a Theme-owned presentation and replace glyphs, spacing, borders, or role
styles. Then call its `compose` method:

```rust
use std::io;

use urushi::{Color, List, ThemePreset, TextStyle, dash_enumerator};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let list = List::new().item("Compile").item("Test");
    let presentation = theme
        .components()
        .list()
        .clone()
        .enumerator(dash_enumerator)
        .enumerator_style(TextStyle::new().foreground(Color::CYAN));

    urushi::println_view(&presentation.compose(&list))
}
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="A list with cyan dash enumerators"><code><span class="ansi-cyan">-</span> Compile
<span class="ansi-cyan">-</span> Test</code></pre>

This policy remains independent of the list's concrete value type.

## Format typed values

Use `ListItemPresentation<T>`, `TableRowPresentation<T>`, or
`TreeNodePresentation<T>` when application values need component-specific text
or per-value style overrides. Pass that policy to `compose_with`.

Callbacks are evaluated during composition. The resulting View retains owned
text and styles, not application callbacks or borrowed model state.

```rust
use std::io;

use urushi::{Color, List, ListItemPresentation, ListPosition, TextStyle, ThemePreset};

struct Job {
    name: &'static str,
    complete: bool,
}

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let jobs = List::new()
        .item(Job { name: "compile", complete: true })
        .item(Job { name: "test", complete: false });
    let items = ListItemPresentation::new(|job: &Job, position: ListPosition| {
        let mark = if job.complete { "x" } else { " " };
        format!("[{mark}] {} ({}/{})", job.name, position.index() + 1, position.len())
    })
    .item_style(|job, _, _| {
        job.complete.then(|| TextStyle::new().foreground(Color::GREEN))
    });
    let view = theme.components().list().compose_with(&jobs, &items);

    urushi::println_view(&view)
}
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Typed list formatting with a per-value green style override"><code>• <span class="ansi-green">[x] compile (1/2)</span>
• [ ] test (2/2)</code></pre>

Use the corresponding `item_style`, `cell_style`, or `node_style` callback when
a typed value must also replace the canonical role style. Returning `None`
keeps the component-wide fallback; returning `Some(style)` replaces it.

```rust
# use urushi::{BlockStyle, Color, TableRowPresentation, TextStyle, TreeNodePresentation};
let table_rows = TableRowPresentation::<(&str, u32)>::new(|row, _, cells| {
    cells.text(row.0);
    cells.display(row.1);
})
.cell_style(|row, cell, _fallback: &BlockStyle| {
    (row.1 == 0 && cell.column() == 1)
        .then(|| BlockStyle::new().foreground(Color::GREEN).bold())
});

let tree_nodes = TreeNodePresentation::<String>::display()
    .node_style(|name, _| {
        name.ends_with(".rs")
            .then(|| TextStyle::new().foreground(Color::CYAN))
    });
# let _ = (table_rows, tree_nodes);
```

<pre class="terminal-preview" aria-label="Typed table cells and tree nodes with per-value style overrides"><code>fallback table cell   Errors  <span class="ansi-green ansi-bold">0</span>
fallback tree node    └── <span class="ansi-cyan">main.rs</span></code></pre>

Only the matching count cell and Rust filename return `Some(style)`; every
other cell or node keeps its component-wide fallback.

## Keep responsibilities separate

A presentation may decide formatting, glyphs, spacing, borders, and styles. It
does not own output, available terminal area, selection state, navigation, or
an event loop. Those stay with the caller and the chosen surface.
