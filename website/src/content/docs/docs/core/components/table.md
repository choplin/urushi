---
title: Build tables
description: Build text or typed tables and configure borders, rows, columns, and width.
---

Use `Table::text` when every cell is already text:

```rust
use std::io;

use urushi::{Table, ThemePreset};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let table = Table::text()
        .headers(["Name", "Count"])
        .row(["Errors", "0"])
        .row(["Warnings", "2"]);

    urushi::println_view(&theme.table(&table))
}
```

```text title="Rendered output"
┌──────────┬───────┐
│ Name     │ Count │
├──────────┼───────┤
│ Errors   │ 0     │
│ Warnings │ 2     │
└──────────┴───────┘
```

## Keep application rows typed

Use `Table<Row>` for application data. Implementing `TableRow` keeps formatting
at the boundary between application data and presentation. Headers remain
explicit table data.

```rust
use std::io;

use urushi::{Table, TableRow, TableRowCells, ThemePreset};

struct Process<'a> {
    name: &'a str,
    pid: u32,
    selected: bool,
}

impl TableRow for Process<'_> {
    fn write_cells(&self, cells: &mut TableRowCells) {
        cells.text(self.name);
        cells.display(self.pid);
    }
}

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let table = Table::new().headers(["Name", "PID"]).row(Process {
        name: "server",
        pid: 42,
        selected: true,
    });

    urushi::println_view(&theme.table(&table))
}
```

```text title="Rendered output"
┌────────┬─────┐
│ Name   │ PID │
├────────┼─────┤
│ server │ 42  │
└────────┴─────┘
```

The `selected` field remains application state because `write_cells` does not
emit it. For plain structs whose displayed fields already have the right order,
`#[derive(urushi::TableRow)]` writes every non-skipped field through `Display`;
mark application-only fields with `#[table(skip)]`. The data stays typed until
the presentation formats a row.

`hidden(true)` removes the complete table. `offset(start, end)` selects a body
row window without changing headers.

```rust
# use urushi::{Table, ThemePreset};
# let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let window = Table::text()
    .headers(["Name", "State"])
    .row(["first", "done"])
    .row(["kept", "ready"])
    .row(["last", "done"])
    .offset(1, 1);
let hidden = window.clone().hidden(true);
let visible = theme.table(&window);
let empty = theme.table(&hidden);
# let _ = (visible, empty);
```

```text title="Rendered row window"
┌──────┬───────┐
│ Name │ State │
├──────┼───────┤
│ kept │ ready │
└──────┴───────┘
hidden(true) → empty View
```

## Configure presentation

Clone the canonical presentation when changing one table:

```rust
use std::io;

use urushi::{Table, TableBorder, ThemePreset};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let table = Table::text()
        .headers(["Name", "State"])
        .row(["urushi", "ready"])
        .row(["docs", "draft"]);
    let presentation = theme
        .components()
        .table()
        .clone()
        .border(TableBorder::ROUNDED)
        .border_row(true)
        .padding(2)
        .width(32);

    urushi::println_view(&presentation.compose(&table))
}
```

```text title="Rendered output"
╭──────────────┬───────────────╮
│  Name        │  State        │
├──────────────┼───────────────┤
│  urushi      │  ready        │
├──────────────┼───────────────┤
│  docs        │  draft        │
╰──────────────┴───────────────╯
```

Read the configured result against the canonical baseline:

| Canonical baseline | Setting | Visible change |
|---|---|---|
| Normal border | `border(ROUNDED)` | Square `┌` becomes rounded `╭` |
| Body rows touch | `border_row(true)` | Adds a `├───┼───┤` separator |
| One inner blank | `padding(2)` | Places two blanks around each value |
| Intrinsic width | `width(32)` | Makes the total outer width 32 cells |

Border models are normal, rounded, thick, double, ASCII, Markdown, booktabs,
hidden, or a custom connected `LineGlyphs` network. Each outer edge and the
header, column, and row separators can be enabled independently.

[Compare a geometric Grid with a semantic Table →](/docs/core/views/grid/)
