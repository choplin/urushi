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
| `urushi-tui` | Synchronous full-screen `Screen` and draw-scoped `Frame`, with Urushi-owned cell buffers, diffing, and transactional output. |
| `urushi-tui-app` | TEA-style `Application` and `Runtime`, including effects, subscriptions, delivery, drawing, input, and terminal-session ownership. |
| `urushi-adapter-ratatui` | Stateless style, view, cell, and anchor adapters for a buffer owned by an existing Ratatui application. |
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
urushi-adapter-ratatui = "0.1.0"
```

Caller-owned synchronous frame loop:

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui = "0.1.0"
```

Full-screen Urushi application model:

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui-app = "0.1.0"
```

`urushi_tui_app::run(app)` starts the TEA runtime with production defaults;
`Runtime::new(app)` provides the configurable builder. Applications that drive
their own loop can instead construct `urushi_tui::Screen` directly.

## Cargo features

### `urushi-tui`

| Feature | Default | Purpose |
|---|---:|---|
| `crossterm` | Yes | Re-exports the Crossterm command-writer implementation for a caller-owned frame loop. |

Disable default features when the caller supplies another
`urushi_terminal::CommandWriter`. The crate contains no application runtime,
Tokio dependency, terminal input, or Ratatui integration.

### `urushi-tui-app`

| Feature | Default | Purpose |
|---|---:|---|
| `crossterm` | Yes | Enables the production terminal backend and the short `run(app)` entry point. |
| `graphics` | No | Integrates Kitty or Sixel image presentation and exposes `GraphicsPreference`; applications construct images through a direct `urushi-graphics` dependency. |

With default features disabled, the application model, effects,
subscriptions, and configurable `Runtime` remain available; a caller must
provide the physical terminal integration it uses. The crate depends on
`urushi-tui` with its default features disabled and does not depend on Ratatui.

The `graphics` feature makes the runtime own image protocol selection, Kitty
upload retention, Sixel encoded-band caching and full-scene repaint, recovery,
and cleanup alongside its cell frames. It does not make image construction part
of the application crate, so image-bearing applications also depend on
`urushi-graphics`.

`urushi-adapter-ratatui` has no feature flags. Choosing that dependency is the
explicit opt-in to Ratatui, independently of either Urushi TUI crate.

### `urushi-terminal`

| Feature | Default | Purpose |
|---|---:|---|
| `crossterm` | No | Enables the Crossterm-backed implementation. |

On Unix, `urushi-prompt` uses the native Unix backend. On non-Unix targets it
enables the Crossterm adapter through its target-specific dependency.

## Version coordination

Urushi workspace crates use exact internal version requirements. Keep all
Urushi crates on the same release version.
