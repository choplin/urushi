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

impl ThemeRole for ListRole {
    fn resolve(self, theme: &Theme) -> Style {
        theme.components().list_style(self).clone()
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

impl ThemeRole for TreeRole {
    fn resolve(self, theme: &Theme) -> Style {
        theme.components().tree_style(self).clone()
    }
}

/// A semantic style role used by the Table component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRole {
    Header,
    Cell,
    Border,
}

impl ThemeRole for TableRole {
    fn resolve(self, theme: &Theme) -> Style {
        theme.components().table_style(self).clone()
    }
}

/// A typed role that resolves a style from a [`Theme`].
///
/// This is the extension point of the theme system: an application defines its
/// own role type and derives the style here, so the result keeps following
/// theme overrides and light/dark selection. A role that needs a parameter
/// carries it in the role value, and a role that needs data the theme cannot
/// provide carries a reference to it.
pub trait ThemeRole: Copy {
    fn resolve(self, theme: &Theme) -> Style;
}

impl ThemeRole for ComponentRole {
    fn resolve(self, theme: &Theme) -> Style {
        theme.components().style(self).clone()
    }
}
