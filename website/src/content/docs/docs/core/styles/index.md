---
title: Styles
description: Control terminal-cell color, attributes, underlines, and hyperlinks.
---

Use Styles when the question is **how terminal cells look**:

- foreground and background color;
- bold, dim, italic, blinking, reverse, hidden, and strikethrough attributes;
- underline shape and color; and
- OSC 8 hyperlinks.

Styles do not decide where content goes or how much space it receives. Go to
[Layout](/docs/core/layout/) for margin, borders, padding, dimensions,
alignment, rows, columns, grids, and viewports. The layout API includes a type
named `BlockStyle`, but its geometry belongs under Layout in this documentation
because that is the task it solves.

Use [Themes](/docs/core/themes/) when appearance expresses application meaning
such as success, warning, selected, or focused.

## See the appearance controls

<div class="overview-catalog">
  <a href="/docs/core/styles/text/#set-colors-and-attributes">
    <pre><span class="demo-success">green foreground</span>
<span class="demo-warning">yellow background</span>
<strong>bold</strong> · <em>italic</em></pre>
    <strong>Colors and attributes</strong>
    <span>Foreground, background, emphasis, reverse, hide, and strikethrough.</span>
  </a>
  <a href="/docs/core/styles/text/#configure-an-underline">
    <pre><span style="text-decoration: underline">single</span>
<span style="text-decoration: double underline #f9e2af">double</span>
<span style="text-decoration: wavy underline #f9e2af">curly</span></pre>
    <strong>Underline shape and color</strong>
    <span>Single, double, curly, dotted, and dashed underlines.</span>
  </a>
  <a href="/docs/core/styles/text/#add-an-osc-8-hyperlink">
    <pre><span class="demo-accent" style="text-decoration: underline">Open documentation ↗</span>

fallback: same label</pre>
    <strong>Terminal hyperlinks</strong>
    <span>OSC 8 targets with a readable fallback on unsupported output.</span>
  </a>
</div>

[Run the complete Styles quickstart ↓](#quickstart)

## Quickstart

Create a project and install Urushi:

```sh
cargo new style-demo
cd style-demo
cargo add urushi
```

Replace `src/main.rs` with:

```rust
use std::io;

use urushi::{Color, StyledText, TextStyle};

fn main() -> io::Result<()> {
    let text = StyledText::new(
        "ready",
        TextStyle::new().foreground(Color::GREEN).bold(),
    );

    urushi::println(&text)
}
```

Run it with `cargo run`:

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Green bold text reading ready"><code><span class="ansi-green ansi-bold">ready</span></code></pre>

`TextStyle` travels with the text instead of writing raw ANSI into the string.
The renderer keeps the content usable when a terminal or redirected stream
does not support a requested capability.

## Continue by task

- [Configure text appearance](/docs/core/styles/text/)
- [Look up the complete text Styles API](/docs/core/styles/reference/)
- [Arrange and size content with Layout](/docs/core/layout/)
