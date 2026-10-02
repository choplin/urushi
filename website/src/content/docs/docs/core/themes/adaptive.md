---
title: Select light and dark themes
description: Resolve ThemeMode against an observed terminal background.
---

`ThemeSet` carries matched light and dark Themes. `ThemeMode` decides whether
selection is explicit or based on an optional terminal observation. The
surface that owns the terminal connection performs that observation before it
selects a Theme.

Add the terminal crate when the application owns that connection directly:

```sh
cargo add urushi urushi-terminal
```

This Unix example queries the controlling terminal and falls back explicitly
when the terminal does not provide an OSC 11 background reply:

```rust
use std::io;

use urushi::{
    ColorScheme, ThemeMode, ThemePreset, ThemeSet,
};
use urushi_terminal::{TerminalQuery as _, backend::native::NativeTerminal};

fn main() -> io::Result<()> {
    let mut terminal = NativeTerminal::open()?;
    let observed = terminal.terminal_background()?;
    let mode = ThemeMode::Auto {
        fallback: ColorScheme::Dark,
    };
    let themes = ThemeSet::new(
        ThemePreset::get("Catppuccin Latte").unwrap().theme(),
        ThemePreset::get("Catppuccin Mocha").unwrap().theme(),
    );
    let scheme = mode.resolve(observed);
    let theme = themes.select(scheme);

    // Build every View for this terminal session from `theme`.
    let _ = theme;
    Ok(())
}
```

```text title="Selection outcomes"
observed light background   -> Light
observed dark background    -> Dark
no usable background reply  -> Dark (configured fallback)
```

The selected theme changes concrete colors while the semantic roles stay the
same:

<div class="terminal-preview" role="img" aria-label="The same semantic roles in light and dark themes"><code>Latte  <span style="background:#eff1f5;color:#4c4f69"> body </span> <span style="background:#eff1f5;color:#1e66f5;font-weight:800"> accent </span> <span style="background:#eff1f5;color:#40a02b"> success </span>
Mocha  <span style="background:#1e1e2e;color:#cdd6f4"> body </span> <span style="background:#1e1e2e;color:#74a8fc;font-weight:800"> accent </span> <span style="background:#1e1e2e;color:#89d88b"> success </span></code></div>

With no observation, `Auto { fallback: Dark }` selects `Dark`. Explicit
`Light` and `Dark` modes select their named scheme regardless of the observed
background and therefore need no query.

`Light` and `Dark` modes ignore observations. `Auto` classifies an observed
background and uses its required fallback when no observation is available.
If a prompt, TUI, or adapter already owns a terminal backend, query that same
backend and pass the selected Theme onward; do not open a second connection.
The background query, input, capability queries, and output must stay with the
surface that owns the session. See the
[caller-owned prompt example](/docs/prompts/form/#use-the-application-theme)
for that ownership pattern.
