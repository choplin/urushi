---
title: Components reference
description: Complete reference for List, Table, Tree, Scrollbar, and their presentations.
---

This page is the task-oriented index to the component API. Use the generated
Rust API reference for every signature, generic bound, trait implementation,
and source link:

- [Complete `urushi` API](/api/urushi/index.html)
- [`List`](/api/urushi/struct.List.html) and
  [`ListPresentation`](/api/urushi/struct.ListPresentation.html)
- [`Table`](/api/urushi/struct.Table.html) and
  [`TablePresentation`](/api/urushi/struct.TablePresentation.html)
- [`Tree`](/api/urushi/struct.Tree.html) and
  [`TreePresentation`](/api/urushi/struct.TreePresentation.html)
- [`Scrollbar`](/api/urushi/struct.Scrollbar.html) and
  [`ScrollbarPresentation`](/api/urushi/struct.ScrollbarPresentation.html)

## Canonical Theme methods

`Theme::list`, `table`, `tree`, and `scrollbar` borrow semantic data and return
an owned `View`. `Theme::components()` exposes the canonical presentation
values for cloning or inspection.

## List

| Type | Public API |
|---|---|
| `List<T>` | `new`, `item`, `items`, `hidden`, `offset`, `item_nodes` |
| `ListItem<T>` | `new`, `item`, `items`, `hidden`, `offset`, `value`, `item_nodes`, `is_hidden` |
| `ListPosition` | `new`, `index`, `len`, `is_empty`, `depth` |
| `ListItemPresentation<T>` | `new`, `item_style`, `display` |
| `ListPresentation` | `new`, `get_style`, `style`, `item_style`, `enumerator_style`, `enumerator`, `nesting_indent`, `compose`, `compose_with` |

`ListEnumerator` is `fn(ListPosition) -> String`. Built-ins are
`bullet_enumerator`, `dash_enumerator`, `asterisk_enumerator`,
`arabic_enumerator`, `alphabet_enumerator`, and `roman_enumerator`.

## Table

| Type | Public API |
|---|---|
| `Table<Row>` | `new`, `headers`, `row`, `rows`, `hidden`, `offset`, `is_hidden`, `column_count`, `text` |
| `TextTable` | `headers`, `row`, `rows`, `hidden`, `offset`, `column_count` |
| `TextTableRow` | Owned row type produced from an iterable of displayable cells and used by `Table::text()` / `TextTable` |
| `TableRow` | `write_cells(&self, cells: &mut TableRowCells)` |
| `TableRowCells` | `display`, `text` |
| `TableCell` | `row`, `column`, `text` |
| `TableRowPosition` | `index`, `len`, `is_empty` |
| `TableRowPresentation<T>` | `new`, `cell_style`, `display` |
| `TablePresentation` | `new`, style, border, separator, padding, width, `compose`, and `compose_with` methods |

`TablePresentation` style methods are `get_style`, `get_border_style`, `style`,
`header_style`, `cell_style`, `header_styles`, `column_styles`, and
`border_style`. Geometry methods are `border`, `border_top`, `border_bottom`,
`border_left`, `border_right`, `border_header`, `border_column`, `border_row`,
`padding`, and `width`.

`TableBorder` constants are `NORMAL`, `ROUNDED`, `THICK`, `DOUBLE`, `ASCII`,
`MARKDOWN`, `BOOKTABS`, and `HIDDEN`; `network(LineGlyphs)` constructs a custom
connected grid.

## Tree

| Type | Public API |
|---|---|
| `Tree<T>` | `new`, `root`, `child`, `children`, `hidden`, `child_offset`, `root_value`, `child_nodes` |
| `TreeNode<T>` | `new`, `child`, `children`, `hidden`, `child_offset`, `value`, `child_nodes`, `is_hidden` |
| `TreePosition` | `Root` or `Child { index, len, depth }` |
| `TreeNodePresentation<T>` | `new`, `node_style`, `display` |
| `TreePresentation` | `new`, `get_style`, `style`, `root_style`, `item_style`, `connector_style`, `line_glyphs`, `indent_width`, `compose`, `compose_with` |

`indent_width` panics below three cells. Custom line glyphs follow the Canvas
one-cell glyph contract.

## Scrollbar

| Type | Public API |
|---|---|
| `Scrollbar` | `new`, `position`, `orientation`, `content_length`, `viewport_length`, `get_position` |
| `ScrollbarOrientation` | `Vertical` (default), `Horizontal` |
| `ScrollbarThumbSizing` | `Proportional` (default), `Marker` |
| `ScrollbarGlyphs` | `new`, `track`, `begin`, `end`, and four getters |
| `ScrollbarPresentation` | `new`, `get_glyphs`, `glyphs`, `get_thumb_sizing`, `thumb_sizing`, `get_style`, `style`, four role-style helpers, `compose` |

The four role-style helpers are `thumb_style`, `track_style`, `begin_style`, and
`end_style`. A vertical presentation requires finite height; a horizontal one
requires finite width. Glyph construction panics unless every present glyph is
one printable, one-cell grapheme.

## Visibility and offsets

Hidden containers resolve to an empty View. A hidden list item or tree node also
hides its descendants. Offsets saturate: if the omitted prefix and suffix
consume the sibling group, the visible result is empty rather than an error.
