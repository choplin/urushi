use crate::{Color, Style};

use super::*;

const TOKENS: SemanticTokens = SemanticTokens {
    text: Color::Ansi(1),
    text_muted: Color::Ansi(2),
    background: Color::Ansi(3),
    surface: Color::Ansi(4),
    accent: Color::Ansi(5),
    accent_text: Color::Ansi(6),
    success: Color::Ansi(7),
    warning: Color::Ansi(8),
    error: Color::Ansi(9),
    border: Color::Ansi(10),
};

#[test]
fn component_styles_follow_the_token_mapping() {
    let components = ComponentStyles::from_tokens(&TOKENS);
    let expected = [
        (ComponentRole::Body, "\x1b[31mx\x1b[0m"),
        (ComponentRole::Muted, "\x1b[2;32mx\x1b[0m"),
        (ComponentRole::Accent, "\x1b[1;35mx\x1b[0m"),
        (ComponentRole::Success, "\x1b[37mx\x1b[0m"),
        (ComponentRole::Warning, "\x1b[90mx\x1b[0m"),
        (ComponentRole::Error, "\x1b[1;91mx\x1b[0m"),
        (ComponentRole::PromptQuestion, "\x1b[1;35mx\x1b[0m"),
        (ComponentRole::PromptAnswer, "\x1b[31mx\x1b[0m"),
        (ComponentRole::PromptPlaceholder, "\x1b[3;32mx\x1b[0m"),
        (ComponentRole::PromptCursor, "\x1b[1;35mx\x1b[0m"),
        (ComponentRole::PromptOption, "\x1b[31mx\x1b[0m"),
        (ComponentRole::PromptOptionSelected, "\x1b[1;36;45mx\x1b[0m"),
        (ComponentRole::PromptButton, "\x1b[31;44mx\x1b[0m"),
        (ComponentRole::PromptButtonFocused, "\x1b[1;36;45mx\x1b[0m"),
        (ComponentRole::PromptHelp, "\x1b[2;32mx\x1b[0m"),
        (ComponentRole::PromptError, "\x1b[91mx\x1b[0m"),
    ];
    for (role, rendered) in expected {
        assert_eq!(components.style(role).render("x"), rendered);
    }
    assert_eq!(
        components.style(ComponentRole::Panel).render("x"),
        "\x1b[92m╭───╮\x1b[0m\n\x1b[92m│\x1b[0m\x1b[31;44m x \x1b[0m\x1b[92m│\x1b[0m\n\x1b[92m╰───╯\x1b[0m"
    );
    assert_eq!(
        components.style(ComponentRole::PanelFocused).render("x"),
        "\x1b[35m╭───╮\x1b[0m\n\x1b[35m│\x1b[0m\x1b[31;44m x \x1b[0m\x1b[35m│\x1b[0m\n\x1b[35m╰───╯\x1b[0m"
    );
}

#[test]
fn custom_component_style_replaces_the_default() {
    let components = ComponentStyles::from_tokens(&TOKENS)
        .with_style(ComponentRole::Warning, Style::new().underline());
    assert_eq!(
        components.style(ComponentRole::Warning).render("note"),
        "\x1b[4mnote\x1b[0m"
    );
}

#[derive(Debug, Clone, PartialEq)]
struct AppTheme {
    report_title: Style,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppRole {
    ReportTitle,
}

impl ThemeRole<AppTheme> for AppRole {
    fn resolve(self, theme: &Theme<AppTheme>) -> &Style {
        match self {
            Self::ReportTitle => &theme.extension().report_title,
        }
    }
}

#[test]
fn typed_extension_roles_resolve_from_the_same_theme() {
    let theme = Theme::from_tokens(TOKENS).extend(|tokens, components| AppTheme {
        report_title: components
            .style(ComponentRole::Body)
            .clone()
            .foreground(tokens.accent)
            .bold(),
    });
    assert_eq!(
        theme.style(AppRole::ReportTitle).render("report"),
        "\x1b[1;35mreport\x1b[0m"
    );
    assert_eq!(
        theme.style(ComponentRole::Body).render("body"),
        "\x1b[31mbody\x1b[0m"
    );
}

#[test]
fn theme_set_selects_a_theme_without_changing_its_extension_type() {
    let light = Theme::from_tokens(TOKENS).extend(|tokens, _| tokens.accent);
    let dark = Theme::from_tokens(TOKENS).extend(|tokens, _| tokens.error);
    let themes = ThemeSet::new(light, dark);
    assert_eq!(*themes.light().extension(), Color::Ansi(5));
    assert_eq!(*themes.dark().extension(), Color::Ansi(9));
    assert_eq!(
        *themes.select(ColorScheme::Light).extension(),
        Color::Ansi(5)
    );
    assert_eq!(
        *themes.select(ColorScheme::Dark).extension(),
        Color::Ansi(9)
    );
}
