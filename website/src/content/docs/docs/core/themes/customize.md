---
title: Define tokens and custom roles
description: Build a Theme from semantic colors and extend its roles in application code.
---

## Define semantic tokens

`Theme::from_tokens` creates a Theme and derives its canonical component
presentations. Use `Theme::new` when the application also supplies a complete
custom `ComponentTheme`.

The token fields are semantic inputs, not component-specific colors:

<pre class="terminal-preview" aria-label="Semantic token color swatches"><code>text       <span style="color:#e8e6df">ordinary content</span>   muted   <span class="ansi-dim">secondary content</span>
accent     <span class="ansi-cyan ansi-bold">selected action</span>    success <span class="ansi-green">completed</span>
warning    <span class="ansi-yellow">attention</span>          error   <span class="ansi-red">failed</span>
background <span style="background:#10100f"> cells </span>             surface <span style="background:#30302c"> panel </span></code></pre>

## Add application roles

Define role enums for meanings that belong to the application. Implement
`TextThemeRole` for inline text and `BlockThemeRole` for rectangles. Each role
receives the active Theme, so it continues to follow token overrides and
light/dark selection.

This complete example defines one role of each kind and renders both:

```rust
use std::io;

use urushi::{
    Align, BlockStyle, BlockThemeRole, Border, Color, ComponentRole,
    SemanticTokens, TextStyle, TextThemeRole, Theme, View,
};

#[derive(Clone, Copy)]
enum AppTextRole {
    Command,
}

impl TextThemeRole for AppTextRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        match self {
            Self::Command => theme
                .text_style(ComponentRole::Accent)
                .background(theme.tokens().surface)
                .bold(),
        }
    }
}

#[derive(Clone, Copy)]
enum AppBlockRole {
    Result,
}

impl BlockThemeRole for AppBlockRole {
    fn resolve(self, theme: &Theme) -> BlockStyle {
        match self {
            Self::Result => BlockStyle::from_text_style(
                theme.text_style(ComponentRole::Success),
            )
            .border(Border::ROUNDED)
            .border_foreground(theme.tokens().border)
            .padding((0, 1)),
        }
    }
}

fn main() -> io::Result<()> {
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

    let view = View::column(
        Align::Left,
        [
            View::text("cargo test", theme.text_style(AppTextRole::Command)),
            View::block(
                theme.block_style(AppBlockRole::Result),
                View::text(
                    "12 tests passed",
                    theme.text_style(ComponentRole::Success),
                ),
            ),
        ],
    );

    urushi::println_view(&view)
}
```

<p class="terminal-preview-label">Rendered output</p>
<pre class="terminal-preview" aria-label="Custom text and block theme roles"><code><span class="ansi-cyan ansi-bold" style="background:#10100f">cargo test</span>
<span style="color:#808080">╭─────────────────╮</span>
<span style="color:#808080">│</span> <span class="ansi-green">12 tests passed</span> <span style="color:#808080">│</span>
<span style="color:#808080">╰─────────────────╯</span></code></pre>

The command uses the theme's accent and surface tokens. The result block uses
the success and border tokens. Changing the selected Theme changes the concrete
colors without changing either application role.

Derive custom roles from `theme.tokens()` or existing component roles so the
visual language remains connected. For one component instance, prefer cloning
and changing its presentation over constructing another Theme.
