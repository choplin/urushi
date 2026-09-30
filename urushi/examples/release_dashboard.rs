//! Renders a representative release-readiness fixture.
//!
//! The example demonstrates semantic styling and composition; it does not
//! inspect the current workspace or report live CI results.

use urushi::{
    Align, Available, BlockStyle, Border, ComponentRole, PanelRole, RenderSettings, TextStyle,
    Theme, ThemePreset, VerticalAlign, View, render, resolve,
};

fn theme() -> Theme {
    ThemePreset::get("TokyoNight")
        .expect("the built-in TokyoNight theme is available")
        .theme()
}

fn text(theme: &Theme, value: impl Into<String>, role: ComponentRole) -> View {
    View::block(
        BlockStyle::new().background(theme.tokens().background),
        View::text(
            value,
            theme.text_style(role).background(theme.tokens().background),
        ),
    )
}

fn surface_text(theme: &Theme, value: impl Into<String>, role: ComponentRole) -> View {
    View::block(
        BlockStyle::new().background(theme.tokens().surface),
        View::text(
            value,
            theme.text_style(role).background(theme.tokens().surface),
        ),
    )
}

fn canvas_space(theme: &Theme, value: &'static str) -> View {
    View::block(
        BlockStyle::new().background(theme.tokens().background),
        View::text(
            value,
            TextStyle::new().background(theme.tokens().background),
        ),
    )
}

fn canvas_backdrop(theme: &Theme, content: View) -> View {
    View::block(
        BlockStyle::new().background(theme.tokens().background),
        content,
    )
}

fn badge(theme: &Theme, label: &str, role: ComponentRole) -> View {
    let style = theme
        .text_style(role)
        .background(theme.tokens().background)
        .bold();
    View::block(
        BlockStyle::from_text_style(style.clone())
            .border(Border::ROUNDED)
            .border_foreground(style.get_foreground().unwrap_or(theme.tokens().accent))
            .border_background(theme.tokens().background)
            .padding((0, 1)),
        View::text(label, style),
    )
}

fn check(theme: &Theme, label: &str, detail: &str) -> View {
    View::block(
        theme
            .block_style(PanelRole::Panel)
            .background(theme.tokens().surface)
            .border_background(theme.tokens().surface)
            .width(23)
            .padding((0, 1)),
        View::column(
            Align::Left,
            [
                surface_text(theme, format!("✓ {label}"), ComponentRole::Success),
                surface_text(theme, detail, ComponentRole::Muted),
            ],
        ),
    )
}

fn metric(theme: &Theme, value: &str, label: &str) -> View {
    View::block(
        BlockStyle::new()
            .background(theme.tokens().background)
            .width(15)
            .align(Align::Center),
        View::column(
            Align::Center,
            [
                text(theme, value, ComponentRole::Accent),
                text(theme, label, ComponentRole::Muted),
            ],
        ),
    )
}

fn release_dashboard(theme: &Theme) -> View {
    let header = canvas_backdrop(
        theme,
        View::column(
            Align::Left,
            [
                View::row(
                    VerticalAlign::Center,
                    [
                        badge(theme, "URUSHI", ComponentRole::Accent),
                        canvas_space(theme, "  "),
                        View::block(
                            BlockStyle::new()
                                .background(theme.tokens().background)
                                .width(50),
                            text(theme, "RELEASE READINESS", ComponentRole::Body),
                        ),
                        badge(theme, "READY", ComponentRole::Success),
                    ],
                ),
                text(
                    theme,
                    "One visual language, ready for every terminal surface.",
                    ComponentRole::Muted,
                ),
            ],
        ),
    );

    let checks = canvas_backdrop(
        theme,
        View::row(
            VerticalAlign::Top,
            [
                check(theme, "Compile", "workspace crates"),
                canvas_space(theme, " "),
                check(theme, "Test", "all feature sets"),
                canvas_space(theme, " "),
                check(theme, "Package", "release archives"),
            ],
        ),
    );

    let metrics = canvas_backdrop(
        theme,
        View::row(
            VerticalAlign::Top,
            [
                metric(theme, "9", "crates"),
                canvas_space(theme, "  "),
                metric(theme, "463", "themes"),
                canvas_space(theme, "  "),
                metric(theme, "CJK", "wide text"),
                canvas_space(theme, "  "),
                metric(theme, "1.90+", "Rust"),
            ],
        ),
    );

    let footer = canvas_backdrop(
        theme,
        View::row(
            VerticalAlign::Center,
            [
                text(theme, "●", ComponentRole::Success),
                text(theme, " All release checks passed", ComponentRole::Body),
                View::block(
                    BlockStyle::new()
                        .background(theme.tokens().background)
                        .width(36)
                        .align(Align::Right),
                    text(theme, "v0.1.0 · 2026-09-30", ComponentRole::Muted),
                ),
            ],
        ),
    );

    View::block(
        theme
            .block_style(PanelRole::PanelFocused)
            .background(theme.tokens().background)
            .border_background(theme.tokens().background)
            .padding((1, 2)),
        View::column(
            Align::Left,
            [
                header,
                canvas_space(theme, ""),
                checks,
                canvas_space(theme, ""),
                metrics,
                canvas_space(theme, ""),
                footer,
            ],
        ),
    )
}

fn main() -> std::io::Result<()> {
    let resolved = resolve(&release_dashboard(&theme()), Available::columns(100))
        .map_err(std::io::Error::other)?;
    println!("{}", render(&resolved, &RenderSettings::all()));
    Ok(())
}
