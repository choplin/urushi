//! Typed roles that map semantic meaning to styles.

use crate::{BlockStyle, TextStyle};

use super::Theme;

/// A reusable text component understood by all urushi consumers.
///
/// Every `ComponentRole` resolves to a [`TextStyle`]. Roles whose value is a
/// rectangle — a panel — are [`PanelRole`] values instead, because geometry
/// lives on [`BlockStyle`].
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
        }
    }
}

/// A framed surface: a rectangle, not a run of text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelRole {
    Panel,
    PanelFocused,
}

impl BlockThemeRole for PanelRole {
    fn resolve(self, theme: &Theme) -> BlockStyle {
        match self {
            Self::Panel => theme.components().panel().clone(),
            Self::PanelFocused => theme.components().panel_focused().clone(),
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

impl TextThemeRole for ListRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
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

impl TextThemeRole for TreeRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        theme.components().tree_style(self).clone()
    }
}

/// A semantic cell role used by the Table component.
///
/// A table cell is a block: it aligns its content inside a column width, which
/// is geometry. The glyph style of the table's rules is a plain [`TextStyle`],
/// reachable through [`TableStyle::border_glyph_style`](crate::TableStyle::border_glyph_style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRole {
    Header,
    Cell,
}

impl BlockThemeRole for TableRole {
    fn resolve(self, theme: &Theme) -> BlockStyle {
        theme.components().table_style(self).clone()
    }
}

/// A typed role that resolves a [`TextStyle`] from a [`Theme`].
///
/// This is the extension point of the theme system: an application defines its
/// own role type and derives the style here, so the result keeps following
/// theme overrides and light/dark selection. A role that needs a parameter
/// carries it in the role value, and a role that needs data the theme cannot
/// provide carries a reference to it.
pub trait TextThemeRole: Copy {
    fn resolve(self, theme: &Theme) -> TextStyle;
}

/// A typed role that resolves a [`BlockStyle`] from a [`Theme`].
///
/// The geometry-bearing counterpart of [`TextThemeRole`], for roles whose value is
/// a rectangle rather than a run of text.
pub trait BlockThemeRole: Copy {
    fn resolve(self, theme: &Theme) -> BlockStyle;
}

impl TextThemeRole for ComponentRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        theme.components().text_style(self).clone()
    }
}
