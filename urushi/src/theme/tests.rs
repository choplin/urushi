use crate::{Color, Style, TableStyle};

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
    assert_eq!(
        components.list_style(ListRole::Item).render("x"),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.list_style(ListRole::Enumerator).render("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components.list_style(ListRole::Indenter).render("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Root).render("x"),
        "\x1b[1;31mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Item).render("x"),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Enumerator).render("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Indenter).render("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components.table_style(TableRole::Header).render("x"),
        "\x1b[1;31mx\x1b[0m"
    );
    assert_eq!(
        components.table_style(TableRole::Cell).render("x"),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.table_style(TableRole::Border).render("x"),
        "\x1b[92mx\x1b[0m"
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

#[test]
fn custom_tree_style_replaces_the_default() {
    let components = ComponentStyles::from_tokens(&TOKENS)
        .with_tree_style(TreeRole::Enumerator, Style::new().underline());
    assert_eq!(
        components.tree_style(TreeRole::Enumerator).render("branch"),
        "\x1b[4mbranch\x1b[0m"
    );
}

#[test]
fn list_roles_are_independent_from_tree_roles() {
    let components = ComponentStyles::from_tokens(&TOKENS)
        .with_list_style(ListRole::Enumerator, Style::new().underline());

    assert_eq!(
        components.list_style(ListRole::Enumerator).render("marker"),
        "\x1b[4mmarker\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Enumerator).render("branch"),
        "\x1b[32mbranch\x1b[0m"
    );
}

#[test]
fn list_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.style(ListRole::Item),
        theme.components().list_style(ListRole::Item)
    );
}

#[test]
fn tree_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.style(TreeRole::Root),
        theme.components().tree_style(TreeRole::Root)
    );
}

#[test]
fn custom_table_style_replaces_the_default() {
    let components = ComponentStyles::from_tokens(&TOKENS)
        .with_table_style(TableRole::Border, Style::new().underline());

    assert_eq!(
        components.table_style(TableRole::Border).render("│"),
        "\x1b[4m│\x1b[0m"
    );
    assert_eq!(
        components.table_style(TableRole::Cell).render("cell"),
        "\x1b[31mcell\x1b[0m"
    );
}

#[test]
fn table_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.style(TableRole::Header),
        theme.components().table_style(TableRole::Header)
    );
}

#[test]
fn replacing_the_table_policy_keeps_the_other_component_defaults() {
    let replaced = ComponentStyles::from_tokens(&TOKENS).with_table(TableStyle::new(
        Style::new().underline(),
        Style::new(),
        Style::new(),
    ));

    assert_eq!(
        replaced.table_style(TableRole::Header).render("head"),
        "\x1b[4mhead\x1b[0m"
    );
    assert_eq!(
        replaced.list_style(ListRole::Item).render("item"),
        "\x1b[31mitem\x1b[0m"
    );
}

/// An application role derived from a semantic token and a built-in role,
/// standing in for the typed extension slot the theme no longer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppRole {
    ReportTitle,
}

impl ThemeRole for AppRole {
    fn resolve(self, theme: &Theme) -> Style {
        match self {
            Self::ReportTitle => theme
                .style(ComponentRole::Body)
                .foreground(theme.tokens().accent)
                .bold(),
        }
    }
}

/// An application role parameterized by a value that no stored table could
/// have precomputed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Heading(u8);

impl ThemeRole for Heading {
    fn resolve(self, theme: &Theme) -> Style {
        let style = theme.style(ComponentRole::Body);
        match self.0 {
            1 => style.foreground(theme.tokens().accent).bold(),
            _ => style.foreground(theme.tokens().text_muted),
        }
    }
}

#[test]
fn application_roles_resolve_from_a_token_and_a_built_in_role() {
    let theme = Theme::from_tokens(TOKENS);

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
fn application_roles_follow_an_overridden_built_in_role() {
    let components = ComponentStyles::from_tokens(&TOKENS)
        .with_style(ComponentRole::Body, Style::new().italic());
    let theme = Theme::new(TOKENS, components);

    // The override drops the token foreground the built-in Body style carried,
    // so the application role now derives from the italic override instead.
    assert_eq!(
        theme.style(AppRole::ReportTitle).render("report"),
        "\x1b[1;3;35mreport\x1b[0m"
    );
}

#[test]
fn parameterized_application_roles_resolve_per_value() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(theme.style(Heading(1)).render("h1"), "\x1b[1;35mh1\x1b[0m");
    assert_eq!(theme.style(Heading(2)).render("h2"), "\x1b[32mh2\x1b[0m");
}

#[test]
fn theme_set_selects_between_a_light_and_dark_theme() {
    let light = Theme::from_tokens(TOKENS);
    let dark = Theme::from_tokens(SemanticTokens {
        text: Color::Ansi(9),
        ..TOKENS
    });
    let themes = ThemeSet::new(light, dark);

    assert_eq!(
        themes.select(ColorScheme::Light).style(ComponentRole::Body),
        themes.light().style(ComponentRole::Body)
    );
    assert_eq!(
        themes
            .select(ColorScheme::Dark)
            .style(ComponentRole::Body)
            .render("x"),
        "\x1b[91mx\x1b[0m"
    );
}
