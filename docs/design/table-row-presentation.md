# Table Row Presentation

How Table retains typed application rows while preserving direct-text input,
ragged cells, and one type-independent Theme presentation.

## Table owns rows, not columns

`Table<Row>` stores one homogeneous body-row type. Heterogeneous columns are
ordinary fields of that row type, so a process table can retain `String`,
`u32`, percentage, and state fields without a public heterogeneous Column
collection or a typed-cell enum:

```rust
#[derive(urushi::TableRow)]
struct Process {
    name: String,
    pid: u32,
    cpu: Percent,
    #[table(skip)]
    selected: bool,
}

let table = Table::new()
    .headers(["Name", "PID", "CPU"])
    .rows(processes);
let view = theme.table(&table);
```

Headers remain explicit plain text. Rust field names are not user-facing
labels and cannot encode localization or abbreviations. Header length and each
formatted body-row length remain independent: the widest visible row or header
sets the composed column count, and missing cells are padded as before.

`Table::text()` is the separate direct-text entry. It normalizes iterable rows
into one `TextTableRow` type, so differently sized arrays retain the existing
ragged call shape without admitting typed and text rows in one Table.

## Canonical and custom row formatting

The public `TableRow` contract is the multi-cell counterpart of `Display`.
`#[derive(TableRow)]` writes every non-skipped struct field in declaration
order through `Display`. The short `Theme::table` and
`TablePresentation::compose` paths use this canonical formatter.

`TableRowPresentation<'a, Row>` is the separate application-specific path. Its
formatter receives `&Row`, the visible `TableRowPosition`, and a short-lived
`TableRowCells` destination. The destination formats borrowed fields directly;
the caller need not clone a source String merely to satisfy the callback API.
A custom policy may borrow local state and need not be `Send`, `Sync`, or
`'static`.

The trait exists because a row needs one canonical decomposition into several
cells and Rust has no field reflection. It does not make formatting semantic
Table data: an application can use `compose_with` and another
`TableRowPresentation<Row>` for every non-canonical view.

## Complete style fallback

`TablePresentation` remains the non-generic, cloneable, comparable value stored
by `ComponentTheme`. In addition to its header and body defaults, it stores
positional lists of optional complete styles:

```rust
let presentation = theme.components().table().clone()
    .header_styles([None, Some(numeric_header.clone())])
    .column_styles([None, Some(numeric_cell.clone())]);
```

A missing, out-of-range, or `None` header entry uses the table header default.
A body cell first selects its column entry or the table body default. A typed
cell callback receives that selected value as `fallback_style`: returning
`None` keeps it, while `Some(style)` replaces it completely.

```text
header style: header list > table header default
body style:   typed cell > column list > table body default
```

There is no property patch, implicit merge, or inheritance. A caller that
wants one change clones the fallback and constructs another complete value.
The bound frame retains table and column defaults once and stores only actual
per-cell overrides, rather than cloning a default `BlockStyle` into every cell.
Style-list length never creates semantic columns.

## Composition snapshots policy

Offsets select visible body rows before positions are assigned. Composition
formats each visible row once and invokes an installed typed style callback
once per resulting or padded body cell. The bound frame owns the formatted
plain text and any complete cell overrides. Measurement and drawing read only
that frame, so repeated resolution cannot observe changing application state.

The existing Table Canvas item continues to own width requirements, wrapping,
row heights, borders, and rule recording after the final local size is known.
Typed input changes the binding edge, not the area-dependent geometry or the
Table/Grid separation.

## Rejected designs

- **A public `Column<Row, Value>` collection.** Rust cannot store heterogeneous
  Value parameters without erasure, and the ordinary row struct already
  expresses every field type. A positional style list supplies the only
  column-wide policy currently needed.
- **One typed value for every cell.** Real tables usually have heterogeneous
  columns; forcing them into one application enum adds construction ceremony
  and a state that the row already rules out.
- **Mix typed and direct-text rows.** A sum would leave some rows outside the
  typed formatter and style contract. `Table::text()` keeps the modes explicit.
- **Derive headers from fields.** Source identifiers are not presentation text.
- **Layer partially specified styles.** `BlockStyle` is a complete value with
  no unset state. Explicit cloning keeps replacement semantics local and
  avoids a second patch language.
