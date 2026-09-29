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
5. perform one write.

The helpers do not enter raw mode or manage cursor position beyond the emitted
view.

## Redirected output

When the selected stream is not a terminal, Urushi resolves the view without a
width limit and renders plain text. It emits no ANSI styling.

```sh
my-command > result.txt
```

Direct `StyledText` output preserves source tabs and line boundaries because it
bypasses box-model layout.

## `NO_COLOR`

A non-empty `NO_COLOR` value disables color for standard-stream output.
Supported non-color attributes remain available.

```sh
NO_COLOR=1 my-command
```

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
use urushi::{Available, RenderSettings, render, resolve};

let resolved = resolve(&view, Available::columns(60))?;
let plain = render(&resolved, &RenderSettings::default());
# Ok::<(), urushi::LayoutError>(())
```

`RenderSettings::default()` produces plain text. Build settings explicitly or
convert detected `TerminalCapabilities` when the destination supports terminal
features.
