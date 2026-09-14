//! Theme construction and light/dark selection.

use crate::{BlockStyle, List, Table, TextStyle, Tree, View};

use super::{BlockThemeRole, ComponentTheme, SemanticTokens, TextThemeRole};

/// An explicit choice between a light and dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
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
    pub fn list(&self, list: &List) -> View {
        self.components.list().compose(list)
    }

    /// Composes a tree with this theme's canonical presentation.
    pub fn tree(&self, tree: &Tree) -> View {
        self.components.tree().compose(tree)
    }

    /// Composes a table with this theme's canonical presentation.
    pub fn table(&self, table: &Table) -> View {
        self.components.table().compose(table)
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
