---
title: Output behavior
description: Understand stdout, stderr, terminal detection, redirection, NO_COLOR, and explicit rendering.
---

Urushi separates logical presentation from the features an output destination
can render. The standard-stream helpers perform terminal detection for each
write.

## Terminal output

For an attached terminal, `print_view`, `println_view`, `eprint_view`, and
`eprintln_view`:

1. detect the terminal width and capabilities;
2. resolve the view under the available column count;
3. narrow colors, attributes, underline features, and hyperlinks;
4. serialize ANSI output; and
5. hold the selected stream's lock while writing the rendered bytes and any
   requested trailing newline.

This is one helper operation, not a single operating-system write or an
atomic-output guarantee. A `println*` helper may issue a separate write for its
newline, and the underlying `write_all` operation may itself require multiple
writes.

The helpers do not enter raw mode or manage cursor position beyond the emitted
view.

## Redirected output

When the selected stream is not a terminal, Urushi resolves the view without a
width limit and renders plain text. It emits no ANSI styling.

```sh
my-command > result.txt
cat result.txt
```

```text title="result.txt"
Build complete
```

The redirected file contains the characters only: no SGR color/attribute
sequences and no OSC 8 hyperlink sequences. Layout geometry such as borders
remains because it is ordinary text.

Direct `StyledText` output preserves source tabs and line boundaries because it
bypasses box-model layout.

## `NO_COLOR`

A non-empty `NO_COLOR` value disables color for standard-stream output.
Supported non-color attributes remain available.

```sh
NO_COLOR=1 my-command
```

Given green bold output, the two terminal results differ only in color:

<pre class="terminal-preview" aria-label="Comparison of normal and NO_COLOR output"><code>normal:   <span class="ansi-green ansi-bold">Build complete</span>
NO_COLOR: <span class="ansi-bold">Build complete</span></code></pre>

`NO_COLOR` does not force plain text. Bold, italic, underline, and other
supported non-color attributes remain enabled. Redirection still selects the
plain-text path independently.

## Stdout or stderr

Use stdout for the command result that a caller may pipe or capture. Use stderr
for diagnostics, progress, prompts, or information that must not contaminate a
machine-readable stdout stream.

Urushi does not choose a stream based on semantic roles; the application makes
that decision by selecting the appropriate output function.

## Render explicitly

Use `resolve` and `render` when writing to an arbitrary `std::io::Write` target
or when width and capabilities come from somewhere other than a process stream.

```rust
use urushi::{
    Available, Color, RenderSettings, TextStyle, View, render, resolve,
};

let view = View::text(
    "Build complete",
    TextStyle::new().foreground(Color::GREEN).bold(),
);
let resolved = resolve(&view, Available::columns(60))?;
let plain = render(&resolved, &RenderSettings::default());

assert_eq!(plain, "Build complete");
# Ok::<(), urushi::LayoutError>(())
```

```text title="Rendered string"
Build complete
```

`RenderSettings::default()` produces plain text. To retain the complete logical
style after layout has been resolved, select every renderer feature explicitly:

```rust
# use urushi::{Available, Color, RenderSettings, TextStyle, View, render, resolve};
# let view = View::text("Build complete", TextStyle::new().foreground(Color::GREEN).bold());
# let resolved = resolve(&view, Available::columns(60))?;
let ansi = render(&resolved, &RenderSettings::all());
assert_eq!(ansi, "\u{1b}[1;32mBuild complete\u{1b}[0m");
# Ok::<(), urushi::LayoutError>(())
```

<pre class="terminal-preview" aria-label="Explicit rendering retains green bold output"><code><span class="ansi-green ansi-bold">Build complete</span></code></pre>

When capabilities come from a terminal backend, convert them rather than
claiming unsupported features:

```rust
use urushi::{RenderSettings, TerminalCapabilities};

let capabilities = TerminalCapabilities::none(); // Replace with a backend query.
let settings = RenderSettings::from(capabilities);
assert_eq!(settings, RenderSettings::default());
```

```text title="Verification"
TerminalCapabilities::none() maps to RenderSettings::default().
```

The [CLI reference](/docs/cli/reference/) lists every `RenderSettings` feature
axis and reset method.
