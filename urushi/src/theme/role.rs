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
            Self::Error => 4,
            Self::PromptQuestion => 5,
            Self::PromptAnswer => 6,
            Self::PromptPlaceholder => 7,
            Self::PromptCursor => 8,
            Self::PromptOption => 9,
            Self::PromptOptionSelected => 10,
            Self::PromptButton => 11,
            Self::PromptButtonFocused => 12,
            Self::PromptHelp => 13,
            Self::PromptError => 14,
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
            Self::Panel => theme.components().get_panel().clone(),
            Self::PanelFocused => theme.components().get_panel_focused().clone(),
        }
    }
}

/// A semantic style role used by the List component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListRole {
    Item,
    Enumerator,
}

impl TextThemeRole for ListRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        theme.components().get_list_style(self).clone()
    }
}

/// A semantic style role used by the Tree component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeRole {
    Root,
    Item,
    Connector,
}

impl TextThemeRole for TreeRole {
    fn resolve(self, theme: &Theme) -> TextStyle {
        theme.components().get_tree_style(self).clone()
    }
}

/// A semantic cell role used by the Table component.
///
/// A table cell is a block: it aligns its content inside a column width, which
/// is geometry. The glyph style of the table's rules is a plain [`TextStyle`],
/// reachable through
/// [`TablePresentation::get_border_style`](crate::TablePresentation::get_border_style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableRole {
    Header,
    Cell,
}

impl BlockThemeRole for TableRole {
    fn resolve(self, theme: &Theme) -> BlockStyle {
        theme.components().get_table_style(self).clone()
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
        theme.components().get_text_style(self).clone()
    }
}
