//! Theme construction and light/dark selection.

use std::fmt;

use urushi_terminal::TerminalBackground;

use crate::{BlockStyle, List, Scrollbar, Table, TextStyle, Tree, View};

use super::{BlockThemeRole, ComponentTheme, SemanticTokens, TextThemeRole};

/// An explicit choice between a light and dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// The caller's policy for selecting a light or dark theme.
///
/// `Light` and `Dark` are explicit choices and need no terminal observation.
/// `Auto` classifies an observed terminal background, or uses its explicit
/// fallback when observation is unavailable.
///
/// ```
/// use std::io;
/// use urushi::{ColorScheme, ThemeMode};
/// use urushi_terminal::TerminalQuery;
///
/// # fn resolve_mode(
/// #     terminal: &mut impl TerminalQuery,
/// #     mode: ThemeMode,
/// # ) -> io::Result<ColorScheme> {
/// let background = match mode {
///     ThemeMode::Auto { .. } => terminal.terminal_background()?,
///     ThemeMode::Light | ThemeMode::Dark => None,
/// };
/// let scheme = mode.resolve(background);
/// # Ok(scheme)
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    Auto { fallback: ColorScheme },
}

impl ThemeMode {
    pub fn resolve(self, background: Option<TerminalBackground>) -> ColorScheme {
        match self {
            Self::Light => ColorScheme::Light,
            Self::Dark => ColorScheme::Dark,
            Self::Auto { fallback } => background.map_or(fallback, classify_background),
        }
    }
}

fn classify_background(background: TerminalBackground) -> ColorScheme {
    let red = linear_srgb(background.red());
    let green = linear_srgb(background.green());
    let blue = linear_srgb(background.blue());
    let luminance = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    if luminance < 0.5 {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

fn linear_srgb(channel: u16) -> f64 {
    let encoded = f64::from(channel) / f64::from(u16::MAX);
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// A theme with common semantic tokens and canonical component presentations.
///
/// A theme carries no application-specific slot. Applications extend it by
/// implementing [`TextThemeRole`] for their own role type and deriving the style
/// inside [`TextThemeRole::resolve`]; see the module documentation for the
/// conventions that follow from that.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    tokens: SemanticTokens,
    components: ComponentTheme,
}

impl Theme {
    pub fn from_tokens(tokens: SemanticTokens) -> Self {
        let components = ComponentTheme::from_tokens(&tokens);
        Self::new(tokens, components)
    }

    pub fn new(tokens: SemanticTokens, components: ComponentTheme) -> Self {
        Self { tokens, components }
    }

    pub fn tokens(&self) -> &SemanticTokens {
        &self.tokens
    }

    pub fn components(&self) -> &ComponentTheme {
        &self.components
    }

    /// Composes a list with this theme's canonical presentation.
    pub fn list<T>(&self, list: &List<T>) -> View
    where
        T: fmt::Display,
    {
        self.components.list().compose(list)
    }

    /// Composes a tree with this theme's canonical presentation.
    pub fn tree<T>(&self, tree: &Tree<T>) -> View
    where
        T: fmt::Display,
    {
        self.components.tree().compose(tree)
    }

    /// Composes a table with this theme's canonical presentation.
    pub fn table<T>(&self, table: &Table<T>) -> View
    where
        T: crate::TableRow,
    {
        self.components.table().compose(table)
    }

    /// Composes a scrollbar with this theme's canonical presentation.
    pub fn scrollbar(&self, scrollbar: &Scrollbar) -> View {
        self.components.scrollbar().compose(scrollbar)
    }

    /// Resolves one role against this theme.
    ///
    /// The style is returned by value because application roles derive their
    /// style rather than reading it from a stored table. Consumers that resolve
    /// roles inside a draw loop should resolve once into their own cache and
    /// borrow from it; [`Theme::components`] also exposes built-in styles as
    /// borrows.
    pub fn text_style<R>(&self, role: R) -> TextStyle
    where
        R: TextThemeRole,
    {
        role.resolve(self)
    }

    /// Resolves one geometry-bearing role against this theme.
    pub fn block_style<R>(&self, role: R) -> BlockStyle
    where
        R: BlockThemeRole,
    {
        role.resolve(self)
    }
}

/// A matched light and dark theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSet {
    light: Theme,
    dark: Theme,
}

impl ThemeSet {
    pub fn new(light: Theme, dark: Theme) -> Self {
        Self { light, dark }
    }

    pub fn light(&self) -> &Theme {
        &self.light
    }

    pub fn dark(&self) -> &Theme {
        &self.dark
    }

    pub fn select(&self, scheme: ColorScheme) -> &Theme {
        match scheme {
            ColorScheme::Light => self.light(),
            ColorScheme::Dark => self.dark(),
        }
    }
}
