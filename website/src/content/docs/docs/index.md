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

## Find a presentation concept

The documentation follows the same vocabulary as Urushi's presentation model.

| Concept | Read this for |
|---|---|
| [View](/docs/core/views/) | The renderer-neutral presentation value shared by every Urushi surface |
| [Styles](/docs/core/styles/) | Cell colors, attributes, underlines, and hyperlinks |
| [Layout](/docs/core/layout/) | Box geometry, sizing, rows, columns, grids, viewports, and anchors |
| [Themes](/docs/core/themes/) | Semantic roles shared across output, prompts, and TUIs |
| [Components](/docs/core/components/) | Lists, tables, trees, and scrollbars |
| [Canvas](/docs/core/canvas/) | Positioned cells, overlapping content, and connected lines |
| [Text width](/docs/core/text-width/) | Graphemes, CJK, emoji, tabs, and wrapping |

## Start from what you are building

| I want to… | Start with | Crate |
|---|---|---|
| Write terminal-aware stdout or stderr and return | <a class="docs-task-link" href="/docs/cli/">CLI overview</a> | `urushi` |
| Apply an opinionated CLI visual language | <a class="docs-task-link" href="/docs/cli/presentations/">CLI presentations</a> | `urushi-cli` |
| Ask the user for input or a selection | <a class="docs-task-link" href="/docs/prompts/">Prompt overview</a> | `urushi-prompt` |
| Build a full-screen application from `init`, `update`, `view`, and `subscriptions` | <a class="docs-task-link" href="/docs/tui/">TUI overview</a> | `urushi-tui-app` |
| Drive a synchronous frame loop yourself | <a class="docs-task-link" href="/docs/tui/">TUI overview</a> | `urushi-tui` |
| Add Urushi views to an existing Ratatui app | <a class="docs-task-link" href="/docs/tui/ratatui/">Ratatui adapter</a> | `urushi-adapter-ratatui` |
| Display Kitty or Sixel images with text fallback | <a class="docs-task-link" href="/docs/graphics/">Display terminal images</a> | `urushi-graphics` |

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

<pre class="terminal-preview" aria-label="Bold terminal output"><code><span class="ansi-bold">Build complete</span></code></pre>

`println_view` inspects stdout, resolves the view against the available terminal
width, selects supported rendering features, and writes one static result. It
does not enter raw mode or start an event loop.

## Run a full-screen application

The full-screen model exposed by `urushi-tui-app` is an application framework
in the style of The Elm Architecture. An application describes its model
through `init`, `update`, `view`, and `subscriptions`; the runtime owns
accepted-message ordering, effects, draw scheduling, and terminal lifecycle.

The runtime resolves each `View` into the concrete `urushi-tui` `Screen` and
draw-scoped `Frame`. That lower crate owns cell buffers, diffing, and
transactional output without depending on Ratatui.

`urushi_tui_app::run(app)` starts that complete lifecycle with production defaults.
`Runtime::new(app)` provides the same runtime as a builder when the application
needs to replace the terminal backend, executor, clock, or session options.
The caller-owned [Ratatui adapter](/docs/tui/ratatui/) is a separate crate for
applications that already have a loop.

[Build your first application →](/docs/tui/application/)

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

- static output locks one standard stream, writes its result, and returns;
- a prompt temporarily owns a blocking terminal session;
- the Urushi TUI runtime owns delivery, drawing, and terminal lifecycle; and
- when an application uses only the Ratatui adapter, that application keeps its
  existing terminal and event loop.

Urushi does not require an application to migrate from one of these surfaces to
another. Choose the surface that matches the application you are building.
