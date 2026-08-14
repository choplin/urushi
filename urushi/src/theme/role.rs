//! Typed roles that map semantic meaning to styles.

use crate::Style;

use super::Theme;

/// A reusable visual component understood by all urushi consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentRole {
    Body,
    Muted,
    Accent,
    Success,
    Warning,
    Error,
    PromptQuestion,
    PromptAnswer,
    PromptPlaceholder,
    PromptCursor,
    PromptOption,
    PromptOptionSelected,
    PromptButton,
    PromptButtonFocused,
    PromptHelp,
    PromptError,
    Panel,
    PanelFocused,
}

impl ComponentRole {
    pub(super) const fn index(self) -> usize {
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
            Self::PromptButton => 12,
            Self::PromptButtonFocused => 13,
            Self::PromptHelp => 14,
            Self::PromptError => 15,
            Self::Panel => 16,
            Self::PanelFocused => 17,
        }
    }
}

/// A semantic style role used by the List component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListRole {
    Item,
    Enumerator,
    Indenter,
}

impl<E> ThemeRole<E> for ListRole {
    #[allow(
        clippy::needless_lifetimes,
        reason = "Matches the public ThemeRole contract signature."
    )]
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style {
        theme.components().list_style(self)
    }
}

/// A semantic style role used by the Tree component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeRole {
    Root,
    Item,
    Enumerator,
    Indenter,
}

impl<E> ThemeRole<E> for TreeRole {
    #[allow(
        clippy::needless_lifetimes,
        reason = "Matches the public ThemeRole contract signature."
    )]
    fn resolve<'a>(self, theme: &'a Theme<E>) -> &'a Style {
        theme.components().tree_style(self)
    }
}

/// A typed role that resolves a style from a [`Theme`].
pub trait ThemeRole<E = ()>: Copy {
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
