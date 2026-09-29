---
title: Crates and features
description: Choose the Urushi crates and Cargo features required by each terminal surface.
---

Urushi is split by responsibility so applications can depend only on the
terminal surfaces they use.

| Crate | Purpose |
|---|---|
| `urushi` | Logical styles, themes, renderer-neutral views, components, layout, ANSI rendering, and standard-stream output. |
| `urushi-cli` | Opinionated presentations for human-facing, non-interactive CLI output; 0.1.0 includes summaries and warnings. |
| `urushi-prompt` | Typed input, select, and confirm fields with validation; 0.1.0 exposes inline presentation, while alternate-screen presentation is part of the target architecture. |
| `urushi-tui` | In 0.1.0, the runnable TEA-style full-screen runtime, terminal/frame contracts, the Ratatui-backed presentation, and adapters for caller-owned Ratatui buffers. |
| `urushi-graphics` | Image components plus Kitty and Sixel terminal graphics adapters. |
| `urushi-terminal` | Shared terminal commands, events, geometry, capabilities, and session restoration. |
| `urushi-derive` | Derive support used by the Urushi crates. |

All crates currently share the same release version. Start with `urushi`, then
add the crates that correspond to the surfaces the application presents.

## Common dependency sets

Ordinary output:

```toml
[dependencies]
urushi = "0.1.0"
```

Structured non-interactive CLI output:

```toml
[dependencies]
urushi = "0.1.0"
urushi-cli = "0.1.0"
```

Interactive prompts:

```toml
[dependencies]
urushi-prompt = "0.1.0"
```

Existing Ratatui application:

```toml
[dependencies]
ratatui = "0.30"
urushi = "0.1.0"
urushi-tui = { version = "0.1.0", default-features = false }
```

Full-screen Urushi application model:

```toml
[dependencies]
urushi-tui = "0.1.0"
```

The default features expose the complete application runtime and its production
Crossterm backend. `urushi_tui::run(app)` starts it with production defaults;
`Runtime::new(app)` provides the configurable builder.

## Cargo features

### `urushi-tui`

| Feature | Default | Purpose |
|---|---:|---|
| `runtime` | Yes | Exposes the TEA-style `Application`, `Effect`, `Subscription`, and related runtime model types. |
| `crossterm` | Yes | Supplies and re-exports the production Crossterm terminal backend. |

Disable default features when an existing Ratatui application only needs the
stateless widgets and style adapter.

The `runtime` feature alone does not select a physical backend. The default
feature set enables both `runtime` and `crossterm`, which is why the short
`run(app)` entry point is available with an ordinary dependency. The current
full-screen presentation implementation, `RatatuiTerminal`, uses Ratatui for
buffers and cell diffing; the `Application` model does not expose a Ratatui
backend type.

The target architecture splits this package into the synchronous `urushi-tui`
frame engine, the `urushi-tui-app` TEA runtime, and
`urushi-adapter-ratatui`. Those package names describe the intended boundary
and are not available as 0.1.0 dependencies.

### `urushi-terminal`

| Feature | Default | Purpose |
|---|---:|---|
| `crossterm` | No | Enables the Crossterm-backed implementation. |

On Unix, `urushi-prompt` uses the native Unix backend. On non-Unix targets it
enables the Crossterm adapter through its target-specific dependency.

## Version coordination

Urushi workspace crates use exact internal version requirements. Keep all
Urushi crates on the same release version.
