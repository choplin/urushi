---
title: Style ordinary output
description: Write styled text or complete box-model views without entering an interactive terminal mode.
---

Use the core `urushi` crate when a command writes output and returns. This path
does not enter raw mode, read input, or start an event loop.

## Write styled text directly

`StyledText` preserves the source line structure and tabs. Use it when layout
does not need to add borders, padding, dimensions, or composition.

```rust
use urushi::{Color, StyledText, TextStyle};

let text = StyledText::new(
    "Build complete",
    TextStyle::new().foreground(Color::GREEN).bold(),
);

urushi::println(&text)?;
# Ok::<(), std::io::Error>(())
```

The result is one line. This preview preserves the color and weight selected by
the `TextStyle`:

<pre class="terminal-preview" aria-label="Green bold text reading Build complete"><code><span class="ansi-green ansi-bold">Build complete</span></code></pre>

`print` and `println` target stdout. `eprint` and `eprintln` target stderr.

## Write a layout-aware view

Use a `View` when output needs wrapping, alignment, borders, spacing, or a
specific available width.

```rust
use urushi::{BlockStyle, Border, Color, TextStyle, View};

let style = BlockStyle::new()
    .border(Border::ROUNDED)
    .border_foreground(Color::BRIGHT_BLACK)
    .padding((0, 1));

let view = View::block(
    style,
    View::text("Build complete", TextStyle::new().bold()),
);

urushi::println_view(&view)?;
# Ok::<(), std::io::Error>(())
```

The same message becomes a complete terminal rectangle. The border uses the
muted bright-black color and the child text remains bold:

<pre class="terminal-preview" aria-label="Bright-black rounded border around bold Build complete text"><code><span class="ansi-bright-black">╭────────────────╮</span>
<span class="ansi-bright-black">│</span> <span class="ansi-bold">Build complete</span> <span class="ansi-bright-black">│</span>
<span class="ansi-bright-black">╰────────────────╯</span></code></pre>

The `*_view` functions resolve the complete view against the detected terminal
width before rendering it.

| Target | Text | View |
|---|---|---|
| stdout, no newline | `print` | `print_view` |
| stdout, newline | `println` | `println_view` |
| stderr, no newline | `eprint` | `eprint_view` |
| stderr, newline | `eprintln` | `eprintln_view` |

## Keep raw ANSI out of content

Text passed to Urushi is plain text. Apply color, attributes, underline, and
hyperlinks through `TextStyle`; do not embed escape sequences in strings.
Embedded control sequences cannot participate correctly in width measurement,
wrapping, or capability degradation.

## Choose text or view output

Choose `StyledText` when the exact source lines should be preserved. Choose a
`View` when the output should adapt to an available terminal rectangle.

Next, [compose blocks and layouts](/docs/cli/layout/) or read the
[standard-stream behavior](/docs/cli/output-behavior/). For every output
helper, setting, and default, use the [CLI reference](/docs/cli/reference/).
