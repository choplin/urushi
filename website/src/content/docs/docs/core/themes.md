---
title: Themes and semantic roles
description: Define semantic color tokens once and use them across static output, prompts, and Ratatui views.
---

A `Theme` names colors by meaning rather than by component or renderer. Built-in
component presentations derive their styles from these semantic tokens.

## Define semantic tokens

```rust
use urushi::{Color, SemanticTokens, Theme};

let theme = Theme::from_tokens(SemanticTokens {
    text: Color::BRIGHT_WHITE,
    text_muted: Color::BRIGHT_BLACK,
    background: Color::BLACK,
    surface: Color::BLACK,
    accent: Color::CYAN,
    accent_text: Color::BLACK,
    success: Color::GREEN,
    warning: Color::YELLOW,
    error: Color::RED,
    border: Color::BRIGHT_BLACK,
});
```

The theme constructs canonical presentations for built-in lists, tables, trees,
scrollbars, panels, and component roles.

The terminal structure stays the same across theme variants; semantic roles
change its appearance. For example, the CLI summary maps the glyph and title
to `accent`, the rail to `text_muted`, and the field label to `text_muted`:

```rust
use urushi_cli::{CliTheme, Summary};

let view = CliTheme::from_theme(&theme).summary(
    &Summary::new("Build complete")
        .field("Output", "target/release/app"),
);

urushi::println_view(&view)?;
# Ok::<(), std::io::Error>(())
```

```text title="Theme applied to a summary"
│
◇  Build complete
│  Output  target/release/app
```

## Resolve a semantic role

```rust
use urushi::{ComponentRole, PanelRole};

let body = theme.text_style(ComponentRole::Body);
let focused_panel = theme.block_style(PanelRole::PanelFocused);
```

Components should request roles instead of selecting concrete terminal colors.
That keeps their meaning stable when a light theme, dark theme, or reduced
terminal capability changes the final representation.

## Provide light and dark variants

`ThemeSet` carries matched light and dark themes. `ThemeMode` makes the
selection policy explicit: force one variant, or classify an observed terminal
background with a required fallback. Neither `Theme` nor `ThemeSet` performs
terminal I/O.

```rust
use urushi::{ColorScheme, TerminalBackground, ThemeMode, ThemeSet};

let themes = ThemeSet::new(light_theme, dark_theme);
let mode = ThemeMode::Auto {
    fallback: ColorScheme::Dark,
};
let observed = Some(TerminalBackground::new(
    u16::MAX,
    u16::MAX,
    u16::MAX,
));
let scheme = mode.resolve(observed);
let theme = themes.select(scheme);

assert_eq!(scheme, ColorScheme::Light);
```

The white observation selects the light theme. If the observation were `None`,
the same policy would select the dark fallback. A terminal application obtains
the optional observation explicitly with
`TerminalQuery::terminal_background()`; explicit `Light` and `Dark` modes do
not need a query.

## Share one theme across surfaces

- Pass `&Theme` to `Form::run` for prompts.
- Query and select first, then pass the same connection to
  `Form::run_with_terminal` when a prompt uses `ThemeMode::Auto`.
- Derive `CliTheme::from_theme(&theme)` for CLI presentations such as summaries and warnings.
- Resolve `PanelRole` or other roles before drawing through Ratatui.

The surfaces retain different control flow while using the same semantic visual
language.

## Extend themes in application code

Applications can define their own role enums and implement `TextThemeRole` or
`BlockThemeRole`. Resolve those roles from `Theme::tokens()` and existing
component roles rather than maintaining a disconnected palette.
