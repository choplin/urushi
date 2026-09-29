---
title: Quickstart
description: Install Urushi and render a bordered, terminal-aware status view.
---

This tutorial builds a small status panel and writes it to stdout. It uses no
raw mode and starts no event loop.

## 1. Add Urushi

```sh
cargo add urushi
```

## 2. Build a view

Create a `TextStyle` for the message, wrap the text in a bordered
`BlockStyle`, and compose both values into a `View`.

```rust
use urushi::{BlockStyle, Border, Color, TextStyle, View};

fn status_view() -> View {
    let text = TextStyle::new()
        .foreground(Color::GREEN)
        .bold();

    let panel = BlockStyle::new()
        .border(Border::ROUNDED)
        .border_foreground(Color::BRIGHT_BLACK)
        .padding((0, 1));

    View::block(panel, View::text("Build complete", text))
}
```

`TextStyle` describes inline appearance. `BlockStyle` describes geometry such
as border and padding. `View` holds the renderer-neutral composition.

## 3. Write the view

```rust
fn main() -> std::io::Result<()> {
    urushi::println_view(&status_view())
}
```

`println_view` detects stdout, resolves the view against the terminal width,
selects supported color and attribute features, writes it, and appends a
newline.

Run the program:

```sh
cargo run
```

The program renders this panel:

```text
╭────────────────╮
│ Build complete │
╰────────────────╯
```

In a color-capable terminal, the message is green and bold, and the border is
bright black. The exact appearance follows the terminal's capabilities.

## 4. Check redirected output

```sh
cargo run > result.txt
```

When stdout is redirected, Urushi emits an unbounded plain-text representation.
The border and padding remain because they are layout, but Urushi does not write
ANSI styling into the file. `NO_COLOR=1 cargo run` removes color on a terminal
while retaining supported non-color attributes.

## Next steps

- [Compose rows, columns, and blocks](/docs/cli/layout/)
- [Create a reusable semantic theme](/docs/core/themes/)
- [Choose another terminal surface](/docs/use-cases/)
