use std::fs::File;

use urushi::{
    Align, AnsiPolicy, BlockStyle, Border, Color, ColorProfile, ColorScheme, ComponentRole,
    ComponentStyles, PanelRole, SemanticTokens, TerminalProfile, Theme, ThemeSet, visible_width,
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
    let components = ComponentStyles::from_tokens(&tokens).with_panel_focused(
        BlockStyle::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .bold()
            .border(Border::ROUNDED)
            .border_foreground(tokens.accent)
            .padding((0, 1))
            // The width measures the outer box: two border columns, two
            // padding columns, and ten cells of content.
            .width(14)
            .align(Align::Center),
    );

    Theme::new(tokens, components)
}

fn themes() -> ThemeSet {
    ThemeSet::new(theme(light_tokens()), theme(dark_tokens()))
}

fn strip_csi(input: &str) -> String {
    let mut output = String::new();
    let mut characters = input.chars();

    while let Some(character) = characters.next() {
        if character == '\x1b' && characters.as_str().starts_with('[') {
            characters.next();
            for control in characters.by_ref() {
                if ('@'..='~').contains(&control) {
                    break;
                }
            }
        } else {
            output.push(character);
        }
    }

    output
}

#[test]
fn theme_set_resolves_roles_for_each_terminal_profile() {
    let themes = themes();
    let expected_panel = "╭────────────╮\n│    名前    │\n╰────────────╯";
    let profiles = [
        (
            TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled),
            "\x1b[1;38;2;255;255;255;48;2;95;135;175mselected\x1b[0m",
            "\x1b[38;2;95;135;175m╭",
        ),
        (
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled),
            "\x1b[1;97;48;5;67mselected\x1b[0m",
            "\x1b[38;5;67m╭",
        ),
        (
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled),
            "\x1b[1;97;100mselected\x1b[0m",
            "\x1b[90m╭",
        ),
        (
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled),
            "\x1b[1mselected\x1b[0m",
            "╭",
        ),
    ];

    for (profile, expected_selection, expected_panel_border) in profiles {
        let selection = profile
            .resolve_text_style(
                &themes
                    .select(ColorScheme::Light)
                    .text_style(ComponentRole::PromptOptionSelected),
            )
            .paint("selected");
        assert_eq!(selection, expected_selection);

        let panel = profile
            .resolve_block_style(
                &themes
                    .select(ColorScheme::Dark)
                    .block_style(PanelRole::PanelFocused),
            )
            .render("名前")
            .into_string();
        assert!(panel.starts_with(expected_panel_border));
        assert_eq!(strip_csi(&panel), expected_panel);
        assert!(panel.lines().all(|line| visible_width(line) == 14));
    }
}

#[test]
fn explicit_profile_is_a_deterministic_consumer_override() {
    let forced = TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled);

    assert_eq!(forced.color_profile(), ColorProfile::Ansi16);
    assert_eq!(forced.ansi_policy(), AnsiPolicy::Enabled);
    assert_eq!(
        forced
            .resolve_text_style(
                &themes()
                    .select(ColorScheme::Light)
                    .text_style(ComponentRole::PromptOptionSelected),
            )
            .paint("selected"),
        "\x1b[1;97;100mselected\x1b[0m"
    );
}

#[test]
fn non_tty_file_disables_ansi_without_changing_theme_layout() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let file = File::open(manifest).unwrap();
    let profile = TerminalProfile::detect_for(&file);

    assert_eq!(profile.color_profile(), ColorProfile::Monochrome);
    assert_eq!(profile.ansi_policy(), AnsiPolicy::Disabled);

    let rendered = profile
        .resolve_block_style(
            &themes()
                .select(ColorScheme::Dark)
                .block_style(PanelRole::PanelFocused),
        )
        .render("名前")
        .into_string();

    assert!(!rendered.contains("\x1b["));
    assert_eq!(rendered, "╭────────────╮\n│    名前    │\n╰────────────╯");
    assert!(rendered.lines().all(|line| visible_width(line) == 14));
}
