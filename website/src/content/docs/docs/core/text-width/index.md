---
title: Text width
description: Understand terminal cells, then measure and wrap grapheme-safe text.
---

Terminal layout uses display cells, not UTF-8 bytes, Unicode scalar values, or
Rust string length. Urushi segments text into grapheme clusters and keeps each
cluster atomic during wrapping and clipping.

## See width-sensitive behavior

<div class="overview-catalog">
  <a href="#quickstart">
    <pre>8 cells
日本語
status</pre>
    <strong>Grapheme-safe wrapping</strong>
    <span>CJK, combining sequences, and emoji stay atomic at line edges.</span>
  </a>
  <a href="/docs/core/text-width/tabs-and-wrapping/">
    <pre>a→      b
12345678
next row</pre>
    <strong>Tabs and finite width</strong>
    <span>Configure tab stops and wrap against terminal-cell columns.</span>
  </a>
</div>

[Run the complete Text width quickstart ↓](#quickstart)

## Quickstart

```sh
cargo new width-demo
cd width-demo
cargo add urushi
```

Replace `src/main.rs` with:

```rust
use std::error::Error;

use urushi::{
    Available, PrintableText, RenderSettings, TextStyle, View, render, resolve,
};

fn main() -> Result<(), Box<dyn Error>> {
    assert_eq!(PrintableText::new("日本語").width(), 6);

    let view = View::text("日本語 status", TextStyle::new());
    let resolved = resolve(&view, Available::columns(8))?;
    print!("{}", render(&resolved, &RenderSettings::default()));
    Ok(())
}
```

Run it with `cargo run`:

```text title="Rendered at eight columns"
日本語
status
```

The three Japanese graphemes occupy six cells. Wrapping keeps every grapheme
whole and removes the break-space before `status`.

## How terminal width works

One visible grapheme may contain several Unicode scalar values and occupy zero,
one, or two terminal cells. This affects CJK text, combining marks, and emoji
sequences.

Urushi applies one width model to wrapping, alignment, grids, padding, borders,
dimensions, Canvas clipping, and Ratatui target rectangles. A wide grapheme is
never split between rows or left as a half-cell at an edge.

`str::len()` counts bytes. Do not use it as a cursor column unless the input is
known to be ASCII. Use Urushi's printable text and grapheme types for terminal
measurement.

Raw ANSI and cursor controls are not text. They break measurement and are
rejected by plain-text constructors. Express appearance through
[Styles](/docs/core/styles/) and terminal actions through the appropriate
terminal API.

## Continue by task

- [Configure tabs and wrapping](/docs/core/text-width/tabs-and-wrapping/)
- [Look up the complete Text width API](/docs/core/text-width/reference/)
