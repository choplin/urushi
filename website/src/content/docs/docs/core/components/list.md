---
title: Build lists
description: Build flat or nested lists, select visible ranges, and customize enumeration.
---

```rust
use std::io;

use urushi::{List, ListItem, ThemePreset};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let steps = List::new()
        .item("Compile")
        .item(ListItem::new("Test").item("unit").item("integration"))
        .item("Package");

    urushi::println_view(&theme.list(&steps))
}
```

```text title="Rendered output"
• Compile
• Test
  • unit
  • integration
• Package
```

`hidden(true)` removes an item and all descendants. `offset(start, end)` omits
visible candidates from the beginning and end of one sibling group. The same
operations exist on the top-level `List` and nested `ListItem` values.

For numbering, clone the theme presentation and replace its enumerator:

```rust
use std::io;

use urushi::{List, ListItem, ThemePreset, arabic_enumerator};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let steps = List::new()
        .item("Compile")
        .item(ListItem::new("Test").item("unit").item("integration"))
        .item("Package");
    let presentation = theme
        .components()
        .list()
        .clone()
        .enumerator(arabic_enumerator)
        .nesting_indent(4);

    urushi::println_view(&presentation.compose(&steps))
}
```

```text title="Rendered output"
1. Compile
2. Test
    1. unit
    2. integration
3. Package
```

Built-ins include bullet, dash, asterisk, Arabic numeral, uppercase alphabetic,
and uppercase Roman numeral enumerators.

```text title="Built-in enumerators"
• item   - item   * item   1. item   A. item   I. item
```

## Select a visible range

Visibility is data, not presentation. `hidden(true)` removes an item and its
descendants. `offset(start, end)` is applied independently to each sibling
group and saturates to an empty group when it omits every candidate.

```rust
# use urushi::{List, ListItem, ThemePreset};
# let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let list = List::new()
    .item("hidden first")
    .item(
        ListItem::new("kept")
            .item("child")
            .item(ListItem::new("hidden subtree").item("descendant").hidden(true)),
    )
    .item("hidden last")
    .offset(1, 1);
let view = theme.list(&list);
# assert_eq!(urushi::measure(&view).height(), 2);
```

```text title="Rendered output"
• kept
  • child
```

The top-level `offset(1, 1)` removes the first and last siblings. The nested
`hidden(true)` removes both `hidden subtree` and its descendant; hiding differs
from selecting a sibling window.
