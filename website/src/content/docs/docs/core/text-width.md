---
title: Terminal width and CJK text
description: Understand grapheme-aware terminal-cell measurement, wrapping, alignment, and tab behavior.
---

Terminal layout is measured in display cells, not UTF-8 bytes, Unicode scalar
values, or Rust string length.

## Grapheme clusters remain atomic

Urushi segments text into grapheme clusters before layout. A wide grapheme is
never split between rows or across the edge of a Ratatui target rectangle.

This matters for:

- CJK characters that commonly occupy two terminal cells;
- combining marks;
- emoji sequences; and
- borders and aligned columns next to multilingual text.

## Width participates in layout

Wrapping, alignment, grid columns, padding, borders, and dimensions use the
same cell-width model. A summary with Japanese labels therefore aligns using
visible terminal width rather than byte count.

```rust
use urushi::{Available, TextStyle, View, resolve};

let view = View::text("日本語 status", TextStyle::new());
let resolved = resolve(&view, Available::columns(8))?;
# let _ = resolved;
# Ok::<(), urushi::LayoutError>(())
```

At eight cells, the six-cell Japanese word and the following status text wrap
without splitting a character:

```text title="Resolved at 8 columns"
日本語
status
```

## Do not use `str::len` for cursor columns

`str::len` returns bytes. It is safe as a terminal column only when the content
is known to be ASCII. This is especially important when using
`PromptStart::CurrentPosition`, where the application supplies the current
column.

## Tabs

`View::text` accepts horizontal tabs and uses the default four-space tab policy.
Construct `StyledText` directly when another `TabPolicy` is required. Direct
text output preserves the source tabs; view output expands them during layout.

## Raw control sequences are not text

Do not embed ANSI escape sequences or cursor movement in strings passed to
Urushi. Control sequences violate the plain-text measurement contract. Express
appearance through styles and terminal actions through the appropriate terminal
command API.
