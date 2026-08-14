//! Theme construction and light/dark selection.

use crate::Style;

use super::{ComponentStyles, SemanticTokens, ThemeRole};

/// An explicit choice between a light and dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// A theme with common semantic tokens and component styles.
///
/// A theme carries no application-specific slot. Applications extend it by
/// implementing [`ThemeRole`] for their own role type and deriving the style
/// inside [`ThemeRole::resolve`]; see the module documentation for the
/// conventions that follow from that.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    tokens: SemanticTokens,
    components: ComponentStyles,
}

impl Theme {
    pub fn from_tokens(tokens: SemanticTokens) -> Self {
        let components = ComponentStyles::from_tokens(&tokens);
        Self::new(tokens, components)
    }

    pub fn new(tokens: SemanticTokens, components: ComponentStyles) -> Self {
        Self { tokens, components }
    }

    pub fn tokens(&self) -> &SemanticTokens {
        &self.tokens
    }

    pub fn components(&self) -> &ComponentStyles {
        &self.components
    }

    /// Resolves one role against this theme.
    ///
    /// The style is returned by value because application roles derive their
    /// style rather than reading it from a stored table. Consumers that resolve
    /// roles inside a draw loop should resolve once into their own cache and
    /// borrow from it; [`Theme::components`] also exposes built-in styles as
    /// borrows.
    pub fn style<R>(&self, role: R) -> Style
    where
        R: ThemeRole,
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
