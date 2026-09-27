use urushi::{
    Align, Available, BlockStyle, Border, Color, ColorLevel, ColorScheme, ComponentRole,
    ComponentTheme, PanelRole, RenderSettings, SemanticTokens, TextAttributes, Theme, ThemeSet,
    UnderlineStyleSet, View, render, resolve,
};

fn light_tokens() -> SemanticTokens {
    SemanticTokens {
        text: Color::Rgb(20, 30, 40),
        text_muted: Color::Rgb(90, 100, 110),
        background: Color::Rgb(255, 255, 255),
        surface: Color::Rgb(245, 245, 245),
        accent: Color::Rgb(95, 135, 175),
        accent_text: Color::Rgb(255, 255, 255),
        success: Color::Rgb(0, 128, 0),
        warning: Color::Rgb(180, 120, 0),
        error: Color::Rgb(200, 0, 0),
        border: Color::Rgb(130, 130, 130),
    }
}

fn dark_tokens() -> SemanticTokens {
    SemanticTokens {
        text: Color::Rgb(230, 230, 230),
        text_muted: Color::Rgb(150, 150, 150),
        background: Color::Rgb(20, 24, 28),
        surface: Color::Rgb(32, 36, 44),
        accent: Color::Rgb(95, 135, 175),
        accent_text: Color::Rgb(255, 255, 255),
        success: Color::Rgb(80, 200, 120),
        warning: Color::Rgb(240, 190, 70),
        error: Color::Rgb(255, 100, 100),
        border: Color::Rgb(110, 120, 130),
    }
}

fn theme(tokens: SemanticTokens) -> Theme {
    let components = ComponentTheme::from_tokens(&tokens).panel_focused(
        BlockStyle::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .bold()
            .border(Border::ROUNDED)
            .border_foreground(tokens.accent)
            .padding((0, 1))
            .width(14)
            .align(Align::Center),
    );
    Theme::new(tokens, components)
}

fn themes() -> ThemeSet {
    ThemeSet::new(theme(light_tokens()), theme(dark_tokens()))
}

fn settings(color_level: ColorLevel) -> RenderSettings {
    RenderSettings::default()
        .color_level(color_level)
        .attributes(TextAttributes::all())
        .underline_styles(UnderlineStyleSet::all())
        .underline_colors(true)
        .hyperlinks(true)
}

fn render_text(view: View, settings: &RenderSettings) -> String {
    render(&resolve(&view, Available::NONE).unwrap(), settings)
}

fn panel_view(style: BlockStyle, content: &str) -> View {
    let text = style.text_style().clone();
    View::block(style, View::text(content, text))
}

#[test]
fn render_settings_degrade_theme_roles_at_the_output_boundary() {
    let themes = themes();
    let role = themes
        .select(ColorScheme::Light)
        .text_style(ComponentRole::PromptOptionSelected);
    let expected = [
        (
            ColorLevel::TrueColor,
            "\x1b[1;38;2;255;255;255;48;2;95;135;175mselected\x1b[0m",
        ),
        (ColorLevel::Ansi256, "\x1b[1;97;48;5;67mselected\x1b[0m"),
        (ColorLevel::Ansi16, "\x1b[1;97;100mselected\x1b[0m"),
        (ColorLevel::None, "\x1b[1mselected\x1b[0m"),
    ];

    for (colors, expected) in expected {
        assert_eq!(
            render_text(View::text("selected", role.clone()), &settings(colors)),
            expected
        );
    }
}

#[test]
fn panel_layout_is_independent_from_render_settings() {
    let style = themes()
        .select(ColorScheme::Dark)
        .block_style(PanelRole::PanelFocused);
    let view = panel_view(style, "名前");
    let resolved = resolve(&view, Available::NONE).unwrap();

    assert_eq!(resolved.size().width(), 14);
    assert_eq!(
        render(&resolved, &RenderSettings::default()),
        "╭────────────╮\n│    名前    │\n╰────────────╯"
    );
    assert!(render(&resolved, &settings(ColorLevel::Ansi16)).contains("\x1b["));
}
