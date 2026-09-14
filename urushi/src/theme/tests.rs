use crate::{BlockStyle, Color, TablePresentation, TextStyle};

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
fn component_theme_follows_the_token_mapping() {
    let components = ComponentTheme::from_tokens(&TOKENS);
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
    for (role, painted) in expected {
        assert_eq!(components.text_style(role).paint("x"), painted);
    }
    assert_eq!(
        components.panel().render("x").as_str(),
        "\x1b[92m╭───╮\x1b[0m\n\x1b[92m│\x1b[0m\x1b[31;44m x \x1b[0m\x1b[92m│\x1b[0m\n\x1b[92m╰───╯\x1b[0m"
    );
    assert_eq!(
        components.panel_focused().render("x").as_str(),
        "\x1b[35m╭───╮\x1b[0m\n\x1b[35m│\x1b[0m\x1b[31;44m x \x1b[0m\x1b[35m│\x1b[0m\n\x1b[35m╰───╯\x1b[0m"
    );
    assert_eq!(
        components.list_style(ListRole::Item).paint("x"),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.list_style(ListRole::Enumerator).paint("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Root).paint("x"),
        "\x1b[1;31mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Item).paint("x"),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Connector).paint("x"),
        "\x1b[32mx\x1b[0m"
    );
    assert_eq!(
        components
            .table_style(TableRole::Header)
            .render("x")
            .as_str(),
        "\x1b[1;31mx\x1b[0m"
    );
    assert_eq!(
        components.table_style(TableRole::Cell).render("x").as_str(),
        "\x1b[31mx\x1b[0m"
    );
    assert_eq!(
        components.table().border_glyph_style().paint("x"),
        "\x1b[92mx\x1b[0m"
    );
}

#[test]
fn custom_component_style_replaces_the_default() {
    let components = ComponentTheme::from_tokens(&TOKENS)
        .with_text_style(ComponentRole::Warning, TextStyle::new().underline());
    assert_eq!(
        components.text_style(ComponentRole::Warning).paint("note"),
        "\x1b[4mnote\x1b[0m"
    );
}

#[test]
fn custom_tree_style_replaces_the_default() {
    let components = ComponentTheme::from_tokens(&TOKENS)
        .with_tree_style(TreeRole::Connector, TextStyle::new().underline());
    assert_eq!(
        components.tree_style(TreeRole::Connector).paint("branch"),
        "\x1b[4mbranch\x1b[0m"
    );
}

#[test]
fn list_roles_are_independent_from_tree_roles() {
    let components = ComponentTheme::from_tokens(&TOKENS)
        .with_list_style(ListRole::Enumerator, TextStyle::new().underline());

    assert_eq!(
        components.list_style(ListRole::Enumerator).paint("marker"),
        "\x1b[4mmarker\x1b[0m"
    );
    assert_eq!(
        components.tree_style(TreeRole::Connector).paint("branch"),
        "\x1b[32mbranch\x1b[0m"
    );
}

#[test]
fn list_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.text_style(ListRole::Item),
        theme.components().list_style(ListRole::Item)
    );
}

#[test]
fn theme_list_shortcut_delegates_to_the_canonical_presentation() {
    let theme = Theme::from_tokens(TOKENS);
    let list = crate::List::new().item("item");

    assert_eq!(theme.list(&list), theme.components().list().compose(&list));
}

#[test]
fn tree_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.text_style(TreeRole::Root),
        theme.components().tree_style(TreeRole::Root)
    );
}

#[test]
fn theme_tree_shortcut_delegates_to_the_canonical_presentation() {
    let theme = Theme::from_tokens(TOKENS);
    let tree = crate::Tree::new().child("item");

    assert_eq!(theme.tree(&tree), theme.components().tree().compose(&tree));
}

#[test]
fn theme_and_component_theme_keep_value_equality_after_cloning() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(theme.clone(), theme);
    assert_eq!(theme.components().clone(), *theme.components());
}

#[test]
fn custom_table_style_replaces_the_default() {
    let components = ComponentTheme::from_tokens(&TOKENS)
        .with_table_style(TableRole::Header, BlockStyle::new().underline());

    assert_eq!(
        components
            .table_style(TableRole::Header)
            .render("head")
            .as_str(),
        "\x1b[4mhead\x1b[0m"
    );
    assert_eq!(
        components
            .table_style(TableRole::Cell)
            .render("cell")
            .as_str(),
        "\x1b[31mcell\x1b[0m"
    );
}

#[test]
fn table_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.block_style(TableRole::Header),
        theme.components().table_style(TableRole::Header)
    );
}

#[test]
fn panel_roles_resolve_through_the_theme_contract() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        &theme.block_style(PanelRole::Panel),
        theme.components().panel()
    );
    assert_eq!(
        &theme.block_style(PanelRole::PanelFocused),
        theme.components().panel_focused()
    );
}

#[test]
fn replacing_the_table_policy_keeps_the_other_component_defaults() {
    let replaced = ComponentTheme::from_tokens(&TOKENS).with_table(TablePresentation::new(
        BlockStyle::new().underline(),
        BlockStyle::new(),
        TextStyle::new(),
    ));

    assert_eq!(
        replaced
            .table_style(TableRole::Header)
            .render("head")
            .as_str(),
        "\x1b[4mhead\x1b[0m"
    );
    assert_eq!(
        replaced.list_style(ListRole::Item).paint("item"),
        "\x1b[31mitem\x1b[0m"
    );
}

/// An application role derived from a semantic token and a built-in role,
/// standing in for the typed extension slot the theme no longer carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppRole {
    ReportTitle,
}

impl TextThemeRole for AppRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        match self {
            Self::ReportTitle => theme
                .text_style(ComponentRole::Body)
                .foreground(theme.tokens().accent)
                .bold(),
        }
    }
}

/// An application role parameterized by a value that no stored table could
/// have precomputed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Heading(u8);

impl TextThemeRole for Heading {
    fn resolve(self, theme: &Theme) -> TextStyle {
        let style = theme.text_style(ComponentRole::Body);
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
        theme.text_style(AppRole::ReportTitle).paint("report"),
        "\x1b[1;35mreport\x1b[0m"
    );
    assert_eq!(
        theme.text_style(ComponentRole::Body).paint("body"),
        "\x1b[31mbody\x1b[0m"
    );
}

#[test]
fn application_roles_follow_an_overridden_built_in_role() {
    let components = ComponentTheme::from_tokens(&TOKENS)
        .with_text_style(ComponentRole::Body, TextStyle::new().italic());
    let theme = Theme::new(TOKENS, components);

    // The override drops the token foreground the built-in Body style carried,
    // so the application role now derives from the italic override instead.
    assert_eq!(
        theme.text_style(AppRole::ReportTitle).paint("report"),
        "\x1b[1;3;35mreport\x1b[0m"
    );
}

#[test]
fn parameterized_application_roles_resolve_per_value() {
    let theme = Theme::from_tokens(TOKENS);

    assert_eq!(
        theme.text_style(Heading(1)).paint("h1"),
        "\x1b[1;35mh1\x1b[0m"
    );
    assert_eq!(
        theme.text_style(Heading(2)).paint("h2"),
        "\x1b[32mh2\x1b[0m"
    );
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
        themes
            .select(ColorScheme::Light)
            .text_style(ComponentRole::Body),
        themes.light().text_style(ComponentRole::Body)
    );
    assert_eq!(
        themes
            .select(ColorScheme::Dark)
            .text_style(ComponentRole::Body)
            .paint("x"),
        "\x1b[91mx\x1b[0m"
    );
}
