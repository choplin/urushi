---
title: Choose by use case
description: Select the Urushi surface and crate that match the terminal experience you are building.
---

Urushi provides one presentation foundation across several terminal surfaces.
It does not turn every terminal program into a full-screen application.

## Style ordinary command output

Use `urushi` when the program writes output and returns. This includes status
messages, bordered panels, lists, tables, trees, scrollbars, and composed rows
or columns.

```toml
[dependencies]
urushi = "0.1.0"
```

[Style ordinary output →](/docs/cli/styled-output/)

To choose the drawing vocabulary first, compare
[View](/docs/core/views/), [layouts](/docs/core/layout/), [components](/docs/core/components/),
and [Canvas](/docs/core/canvas/).

## Build opinionated CLI presentations

Use `urushi-cli` for an opinionated visual language for human-facing,
non-interactive command output. It owns CLI-specific structure such as rails,
status glyphs, title hierarchy, and semantic roles while producing ordinary
Urushi `View` values. `Summary` and `Warning` are the presentations available
in 0.1.0, not the boundary of the crate.

```toml
[dependencies]
urushi = "0.1.0"
urushi-cli = "0.1.0"
```

[Explore CLI presentations →](/docs/cli/presentations/)

## Collect interactive input

Use `urushi-prompt` for a short blocking flow made of text input, selection,
and confirmation fields. A form temporarily manages terminal interaction and
returns submitted typed values or cancellation.

```toml
[dependencies]
urushi-prompt = "0.1.0"
```

[Build a prompt form →](/docs/prompts/form/)

## Build a full-screen application

`urushi-tui-app` provides a TEA-style application runtime. The
application supplies `init`, `update`, `view`, and `subscriptions`; the runtime
owns the live model, ordered delivery, effects, frame scheduling, and terminal
lifecycle. Focus, navigation, key bindings, and other application semantics
remain ordinary model and message logic.

The runtime does not expose a backend type to `Application`. It resolves views
into the concrete `urushi-tui` `Screen` and draw-scoped `Frame`; that lower
crate owns cell buffers, diffing, and transactional output without Ratatui.

```toml
[dependencies]
urushi = "0.1.0"
urushi-tui-app = "0.1.0"
```

Call `urushi_tui_app::run(app)` for the production defaults. Use
`Runtime::new(app)` when the application needs to replace the terminal backend,
executor, clock, or session options.

[Build your first full-screen application →](/docs/tui/application/)

## Add UI to an existing Ratatui application

Use the separate Ratatui adapter when the application already owns a Ratatui
`Terminal`, event loop, state, and frame timing. `ViewWidget` draws a complete
Urushi view into a Ratatui `Rect`; `RatatuiStyleExt` adapts one themed block.
This integration path does not depend on either Urushi TUI crate.

```toml
[dependencies]
urushi = "0.1.0"
urushi-adapter-ratatui = "0.1.0"
```

[Use the Ratatui adapter →](/docs/tui/ratatui/)

## Display terminal images

Use `urushi-graphics` for an image region with text fallback. The renderer uses
Kitty when positively detected, otherwise Sixel when both protocol support and
cell-pixel geometry are available.

For one-shot output:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-terminal = "0.1.0"
```

For images owned by the full-screen runtime, depend on the image model directly
and enable its runtime integration:

```toml
[dependencies]
urushi = "0.1.0"
urushi-graphics = "0.1.0"
urushi-tui-app = { version = "0.1.0", features = ["graphics"] }
```

[Display terminal images →](/docs/graphics/)
