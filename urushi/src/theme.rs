//! Semantic theme roles for terminal consumers.
//!
//! A [`Theme`] owns the meaning of its colors and the [`Style`] values used by
//! reusable components. Consumers select a [`ColorScheme`], resolve a role,
//! and render the returned style with their own output adapter. Themes do not
//! inspect terminals or own output state.
//!
//! ```
//! use urushi::{Color, ColorScheme, ComponentRole, SemanticTokens, Theme, ThemeSet};
//!
//! let tokens = SemanticTokens {
//!     text: Color::BLACK,
//!     text_muted: Color::BRIGHT_BLACK,
//!     background: Color::WHITE,
//!     surface: Color::BRIGHT_WHITE,
//!     accent: Color::BLUE,
//!     accent_text: Color::WHITE,
//!     success: Color::GREEN,
//!     warning: Color::YELLOW,
//!     error: Color::RED,
//!     border: Color::BRIGHT_BLACK,
//! };
//! let themes = ThemeSet::new(Theme::from_tokens(tokens), Theme::from_tokens(tokens));
//!
//! let style = themes.select(ColorScheme::Dark).style(ComponentRole::Accent);
//! assert_eq!(style.render("saved"), "\x1b[1;34msaved\x1b[0m");
//! ```

use crate::{Border, Color, Style};

const COMPONENT_ROLE_COUNT: usize = 16;

/// An explicit choice between a light and dark theme.
///
/// A color scheme is chosen by the caller; this type deliberately has neither
/// an automatic variant nor a default value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Select the light theme.
    Light,
    /// Select the dark theme.
    Dark,
}

/// Colors named for their display meaning rather than a rendering technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticTokens {
    /// Normal foreground color for body text and unselected options.
    pub text: Color,
    /// Foreground color for secondary information.
    pub text_muted: Color,
    /// Base background for output areas that explicitly render one.
    pub background: Color,
    /// Background for surfaces separated from the base background.
    pub surface: Color,
    /// Color for current position, selection, and primary actions.
    pub accent: Color,
    /// Foreground color used over an accent background.
    pub accent_text: Color,
    /// Color for successful results.
    pub success: Color,
    /// Color for recoverable warnings.
    pub warning: Color,
    /// Color for failures and validation errors.
    pub error: Color,
    /// Color for ordinary borders and dividers.
    pub border: Color,
}

/// A reusable visual component understood by all urushi consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentRole {
    /// Normal body text.
    Body,
    /// Secondary text.
    Muted,
    /// Accented text.
    Accent,
    /// Successful result text.
    Success,
    /// Warning text.
    Warning,
    /// Error text.
    Error,
    /// A prompt question.
    PromptQuestion,
    /// A prompt answer.
    PromptAnswer,
    /// Placeholder text in a prompt.
    PromptPlaceholder,
    /// The prompt cursor.
    PromptCursor,
    /// An unselected prompt option.
    PromptOption,
    /// A selected prompt option.
    PromptOptionSelected,
    /// Prompt help text.
    PromptHelp,
    /// A prompt validation error.
    PromptError,
    /// A normal bordered panel.
    Panel,
    /// A bordered panel with focused border treatment.
    PanelFocused,
}

impl ComponentRole {
    const fn index(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Muted => 1,
            Self::Accent => 2,
            Self::Success => 3,
            Self::Warning => 4,
            Self::Error => 5,
            Self::PromptQuestion => 6,
            Self::PromptAnswer => 7,
            Self::PromptPlaceholder => 8,
            Self::PromptCursor => 9,
            Self::PromptOption => 10,
            Self::PromptOptionSelected => 11,
            Self::PromptHelp => 12,
            Self::PromptError => 13,
            Self::Panel => 14,
            Self::PanelFocused => 15,
        }
    }
}

/// Styles for common components, indexed by [`ComponentRole`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentStyles {
    styles: [Style; COMPONENT_ROLE_COUNT],
}

impl ComponentStyles {
    /// Builds the standard component styles from semantic tokens.
    pub fn from_tokens(tokens: &SemanticTokens) -> Self {
        let panel = Style::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .border(Border::ROUNDED)
            .border_foreground(tokens.border)
            .padding((0, 1));

        Self {
            styles: [
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted).dim(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.success),
                Style::new().foreground(tokens.warning),
                Style::new().foreground(tokens.error).bold(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted).italic(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.text),
                Style::new()
                    .foreground(tokens.accent_text)
                    .background(tokens.accent)
                    .bold(),
                Style::new().foreground(tokens.text_muted).dim(),
                Style::new().foreground(tokens.error),
                panel.clone(),
                panel.border_foreground(tokens.accent),
            ],
        }
    }

    /// Returns the style for `role`.
    pub fn style(&self, role: ComponentRole) -> &Style {
        &self.styles[role.index()]
    }

    /// Replaces the style for `role`.
    #[must_use]
    pub fn with_style(mut self, role: ComponentRole, style: Style) -> Self {
        self.styles[role.index()] = style;
        self
    }
}

/// A theme with common semantic tokens, component styles, and typed extension data.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme<E = ()> {
    tokens: SemanticTokens,
    components: ComponentStyles,
    extension: E,
}

impl Theme<()> {
    /// Builds a common theme using the standard component mapping.
    pub fn from_tokens(tokens: SemanticTokens) -> Self {
        let components = ComponentStyles::from_tokens(&tokens);
        Self::new(tokens, components, ())
    }

    /// Adds typed, application-specific theme data.
    ///
    /// The builder receives the common tokens and components so application
    /// styles can derive from the same definitions without a string registry.
    pub fn extend<E>(self, build: impl FnOnce(&SemanticTokens, &ComponentStyles) -> E) -> Theme<E> {
        let extension = build(&self.tokens, &self.components);
        Theme::new(self.tokens, self.components, extension)
    }
}

impl<E> Theme<E> {
    /// Builds a theme from explicit common styles and extension data.
    pub fn new(tokens: SemanticTokens, components: ComponentStyles, extension: E) -> Self {
        Self {
            tokens,
            components,
            extension,
        }
    }

    /// Returns this theme's semantic tokens.
    pub fn tokens(&self) -> &SemanticTokens {
        &self.tokens
    }

    /// Returns this theme's common component styles.
    pub fn components(&self) -> &ComponentStyles {
        &self.components
    }

    /// Returns this theme's typed application extension.
    pub fn extension(&self) -> &E {
        &self.extension
    }

    /// Resolves a common or application-specific role to its logical style.
    pub fn style<R>(&self, role: R) -> &Style
    where
        R: ThemeRole<E>,
    {
        role.resolve(self)
    }
}

/// A typed role that resolves a style from a [`Theme`].
///
/// Applications implement this trait for their own role enum and extension
/// type, keeping their theme vocabulary type checked and local to the app.
pub trait ThemeRole<E = ()>: Copy {
    /// Resolves this role from `theme`.
    #[allow(
        clippy::needless_lifetimes,
        reason = "The public contract spells out the returned Style borrow explicitly."
    )]
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style;
}

impl<E> ThemeRole<E> for ComponentRole {
    #[allow(
        clippy::needless_lifetimes,
        reason = "Matches the public ThemeRole contract signature."
    )]
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style {
        theme.components().style(self)
    }
}

/// A matched light and dark theme with the same extension type.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSet<E = ()> {
    light: Theme<E>,
    dark: Theme<E>,
}

impl<E> ThemeSet<E> {
    /// Pairs the light and dark variants of a theme.
    pub fn new(light: Theme<E>, dark: Theme<E>) -> Self {
        Self { light, dark }
    }

    /// Returns the light theme.
    pub fn light(&self) -> &Theme<E> {
        &self.light
    }

    /// Returns the dark theme.
    pub fn dark(&self) -> &Theme<E> {
        &self.dark
    }

    /// Returns the theme selected by an explicit color scheme.
    pub fn select(&self, scheme: ColorScheme) -> &Theme<E> {
        match scheme {
            ColorScheme::Light => self.light(),
            ColorScheme::Dark => self.dark(),
        }
    }
}

#[cfg(test)]
mod tests {
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

        assert_eq!(
            components.style(ComponentRole::Body).render("x"),
            "\x1b[31mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::Muted).render("x"),
            "\x1b[2;32mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::Accent).render("x"),
            "\x1b[1;35mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::Success).render("x"),
            "\x1b[37mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::Warning).render("x"),
            "\x1b[90mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::Error).render("x"),
            "\x1b[1;91mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptQuestion).render("x"),
            "\x1b[1;35mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptAnswer).render("x"),
            "\x1b[31mx\x1b[0m"
        );
        assert_eq!(
            components
                .style(ComponentRole::PromptPlaceholder)
                .render("x"),
            "\x1b[3;32mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptCursor).render("x"),
            "\x1b[1;35mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptOption).render("x"),
            "\x1b[31mx\x1b[0m"
        );
        assert_eq!(
            components
                .style(ComponentRole::PromptOptionSelected)
                .render("x"),
            "\x1b[1;36;45mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptHelp).render("x"),
            "\x1b[2;32mx\x1b[0m"
        );
        assert_eq!(
            components.style(ComponentRole::PromptError).render("x"),
            "\x1b[91mx\x1b[0m"
        );

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
}
