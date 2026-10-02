---
title: Build trees
description: Build typed hierarchies and configure connectors, visibility, and indentation.
---

```rust
use std::io;

use urushi::{ThemePreset, Tree, TreeNode};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let tree = Tree::new()
        .root("workspace")
        .child("Cargo.toml")
        .child(
            TreeNode::new("src")
                .child("lib.rs")
                .child("main.rs"),
        );

    urushi::println_view(&theme.tree(&tree))
}
```

```text title="Rendered output"
workspace
├── Cargo.toml
└── src
    ├── lib.rs
    └── main.rs
```

`hidden(true)` removes a node and its descendants. `child_offset(start, end)`
selects one child window while preserving the node itself. The top-level Tree
can have an optional root plus any number of children.

Clone `theme.components().tree()` to change root, item, or connector styles;
choose another `LineGlyphs` repertoire; or change `indent_width`. Indentation
must be at least three cells so a junction, horizontal continuation, and gap
all fit.

## Configure connectors and visible children

```rust
use std::io;

use urushi::{LineGlyphs, ThemePreset, Tree, TreeNode};

fn main() -> io::Result<()> {
    let theme = ThemePreset::get("Catppuccin Mocha")
        .expect("built-in theme")
        .theme();
    let tree = Tree::new().root("workspace").child(
        TreeNode::new("src")
            .child("hidden.rs")
            .child("lib.rs")
            .child("main.rs")
            .child_offset(1, 1),
    );
    let presentation = theme
        .components()
        .tree()
        .clone()
        .line_glyphs(LineGlyphs::ASCII)
        .indent_width(3);

    urushi::println_view(&presentation.compose(&tree))
}
```

```text title="Rendered output"
workspace
+- src
   +- lib.rs
```

`child_offset` preserves the parent while selecting a window from its direct
children. `hidden(true)` instead removes the selected node and its complete
subtree. Connector styles affect line cells; root and item styles affect node
text.

The visibility operations are independent:

```rust
# use urushi::{Tree, TreeNode};
let tree = Tree::new().root("workspace").child(
    TreeNode::new("src")
        .child(TreeNode::new("target").child("debug").hidden(true))
        .child("lib.rs")
        .child("main.rs")
        .child_offset(0, 1),
);
# let _ = tree;
```

```text title="Rendered output"
workspace
└── src
    └── lib.rs
```

Here `hidden(true)` removes `target` and `debug`; `child_offset(0, 1)` then
removes only the final visible sibling, `main.rs`.

To see which style controls which cells, set the roles independently:

```rust
# use urushi::{Color, TextStyle, ThemePreset};
# let theme = ThemePreset::get("Catppuccin Mocha").unwrap().theme();
let presentation = theme.components().tree().clone()
    .root_style(TextStyle::new().foreground(Color::CYAN).bold())
    .item_style(TextStyle::new().foreground(Color::GREEN))
    .connector_style(TextStyle::new().foreground(Color::YELLOW));
# let _ = presentation;
```

<pre class="terminal-preview" aria-label="Tree root, connector, and item roles"><code><span class="ansi-cyan ansi-bold">workspace</span>
<span class="ansi-yellow">└──</span> <span class="ansi-green">src</span>
    <span class="ansi-yellow">└──</span> <span class="ansi-green">lib.rs</span></code></pre>
