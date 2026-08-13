//! Theme construction, extension, and light/dark selection.

use crate::Style;

use super::{ComponentStyles, SemanticTokens, ThemeRole};

/// An explicit choice between a light and dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// A theme with common semantic tokens, component styles, and typed extension data.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme<E = ()> {
    tokens: SemanticTokens,
    components: ComponentStyles,
    extension: E,
}

impl Theme<()> {
    pub fn from_tokens(tokens: SemanticTokens) -> Self {
        let components = ComponentStyles::from_tokens(&tokens);
        Self::new(tokens, components, ())
    }

    pub fn extend<E>(self, build: impl FnOnce(&SemanticTokens, &ComponentStyles) -> E) -> Theme<E> {
        let extension = build(&self.tokens, &self.components);
        Theme::new(self.tokens, self.components, extension)
    }
}

impl<E> Theme<E> {
    pub fn new(tokens: SemanticTokens, components: ComponentStyles, extension: E) -> Self {
        Self {
            tokens,
            components,
            extension,
        }
    }

    pub fn tokens(&self) -> &SemanticTokens {
        &self.tokens
    }

    pub fn components(&self) -> &ComponentStyles {
        &self.components
    }

    pub fn extension(&self) -> &E {
        &self.extension
    }

    pub fn style<R>(&self, role: R) -> &Style
    where
        R: ThemeRole<E>,
    {
        role.resolve(self)
    }
}

/// A matched light and dark theme with the same extension type.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSet<E = ()> {
    light: Theme<E>,
    dark: Theme<E>,
}

impl<E> ThemeSet<E> {
    pub fn new(light: Theme<E>, dark: Theme<E>) -> Self {
        Self { light, dark }
    }

    pub fn light(&self) -> &Theme<E> {
        &self.light
    }

    pub fn dark(&self) -> &Theme<E> {
        &self.dark
    }

    pub fn select(&self, scheme: ColorScheme) -> &Theme<E> {
        match scheme {
            ColorScheme::Light => self.light(),
            ColorScheme::Dark => self.dark(),
        }
    }
}
