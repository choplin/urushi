---
title: Urushi documentation
description: Build command output, prompts, and full-screen terminal applications in Rust on one presentation foundation.
---

Urushi is a UI library for Rust terminal applications. Use it to style ordinary
command output, present structured results, collect interactive input, draw
views in an existing Ratatui application, build a full-screen application
around a TEA-style runtime, or place terminal images.

Those surfaces do not share one event loop. They share the presentation choices
that should remain consistent: semantic themes, renderer-neutral views,
box-model layout, and terminal-aware width measurement.

## Start from what you are building

| I want to… | Start with | Crate |
|---|---|---|
| Style text, borders, spacing, or layout | [CLI overview](/docs/cli/) | `urushi` |
| Apply an opinionated CLI visual language | [CLI presentations](/docs/cli/presentations/) | `urushi-cli` |
| Ask the user for input or a selection | [Prompt overview](/docs/prompts/) | `urushi-prompt` |
| Build a full-screen application from `init`, `update`, `view`, and `subscriptions` | [TUI overview](/docs/tui/) | `urushi-tui` |
| Add Urushi views to an existing Ratatui app | [Ratatui adapter](/docs/tui/ratatui/) | `urushi-tui` |
| Display Kitty or Sixel images with text fallback | [Display terminal images](/docs/graphics/) | `urushi-graphics` |

If you want to see the core workflow first, complete the
[quickstart](/docs/quickstart/).

## Install the core crate

```sh
cargo add urushi
```

The core crate defines styles, themes, renderer-neutral views, layout, ANSI
rendering, and standard-stream output. Additional crates add a surface without
changing that core model.

```rust
use urushi::{TextStyle, View};

let view = View::text("Build complete", TextStyle::new().bold());
urushi::println_view(&view)?;
# Ok::<(), std::io::Error>(())
```

```text title="Rendered output"
Build complete
```

`println_view` inspects stdout, resolves the view against the available terminal
width, selects supported rendering features, and writes one static result. It
does not enter raw mode or start an event loop.

## Run a full-screen application

The full-screen model exposed by `urushi-tui` 0.1.0 is an application framework
in the style of The Elm Architecture. An application describes its model
through `init`, `update`, `view`, and `subscriptions`; the runtime owns
accepted-message ordering, effects, draw scheduling, and terminal lifecycle.

The 0.1.0 runtime uses Urushi's `Terminal`, `Frame`, and `CellWriter` contracts
rather than exposing a backend type to `Application`. Ratatui supplies the
current buffer and cell-diff implementation, and a separate adapter lets an
existing Ratatui application keep its own loop.

`urushi_tui::run(app)` starts that complete lifecycle with production defaults.
`Runtime::new(app)` provides the same runtime as a builder when the application
needs to replace the terminal backend, frame terminal, executor, clock, or
session options. The caller-owned [Ratatui adapter](/docs/tui/ratatui/) remains
a separate integration path for applications that already have a loop.

[Build a runnable TUI →](/docs/tui/runtime/)

## What stays shared

Each surface keeps responsibility for its own interaction and terminal
lifecycle. The shared foundation carries the parts that should remain coherent:

- semantic theme roles;
- terminal style values;
- semantic components and their concrete presentations;
- renderer-neutral view composition;
- box-model layout and width resolution;
- CJK-aware grapheme measurement; and
- backend-independent terminal commands, events, geometry, and restoration
  contracts.

[Read about the presentation foundation](/docs/concepts/presentation-foundation/).

## What does not stay shared

Each surface owns its interaction model:

- static output performs one write and returns;
- a prompt temporarily owns a blocking terminal session;
- the Urushi TUI runtime owns delivery, drawing, and terminal lifecycle; and
- when an application uses only the Ratatui adapter, that application keeps its
  existing terminal and event loop.

Urushi does not require an application to migrate from one of these surfaces to
another. Choose the surface that matches the application you are building.
