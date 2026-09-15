//! Canonical presentations and shared role styles for reusable components.

use crate::{BlockStyle, Border, ListPresentation, TablePresentation, TextStyle, TreePresentation};

use super::{ComponentRole, ListRole, SemanticTokens, TableRole, TreeRole};

const COMPONENT_ROLE_COUNT: usize = 15;

/// Theme-derived presentations and shared role styles for common components.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentTheme {
    text_styles: [TextStyle; COMPONENT_ROLE_COUNT],
    panel: BlockStyle,
    panel_focused: BlockStyle,
    list: ListPresentation,
    tree: TreePresentation,
    table: TablePresentation,
}

impl ComponentTheme {
    /// Builds the canonical component theme from semantic tokens.
    pub fn from_tokens(tokens: &SemanticTokens) -> Self {
        let panel = BlockStyle::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .border(Border::ROUNDED)
            .border_foreground(tokens.border)
            .padding((0, 1));

        let text_styles = [
            TextStyle::new().foreground(tokens.text),
            TextStyle::new().foreground(tokens.text_muted).dim(),
            TextStyle::new().foreground(tokens.accent).bold(),
            TextStyle::new().foreground(tokens.success),
            TextStyle::new().foreground(tokens.error).bold(),
            TextStyle::new().foreground(tokens.accent).bold(),
            TextStyle::new().foreground(tokens.text),
            TextStyle::new().foreground(tokens.text_muted).italic(),
            TextStyle::new().foreground(tokens.accent).bold(),
            TextStyle::new().foreground(tokens.text),
            TextStyle::new()
                .foreground(tokens.accent_text)
                .background(tokens.accent)
                .bold(),
            TextStyle::new()
                .foreground(tokens.text)
                .background(tokens.surface),
            TextStyle::new()
                .foreground(tokens.accent_text)
                .background(tokens.accent)
                .bold(),
            TextStyle::new().foreground(tokens.text_muted).dim(),
            TextStyle::new().foreground(tokens.error),
        ];
        Self {
            text_styles,
            panel: panel.clone(),
            panel_focused: panel.border_foreground(tokens.accent),
            list: ListPresentation::new(
                TextStyle::new().foreground(tokens.text),
                TextStyle::new().foreground(tokens.text_muted),
            ),
            tree: TreePresentation::new(
                TextStyle::new().foreground(tokens.text).bold(),
                TextStyle::new().foreground(tokens.text),
                TextStyle::new().foreground(tokens.text_muted),
            ),
            table: TablePresentation::new(
                BlockStyle::new().foreground(tokens.text).bold(),
                BlockStyle::new().foreground(tokens.text),
                TextStyle::new().foreground(tokens.border),
            ),
        }
    }

    pub fn get_text_style(&self, role: ComponentRole) -> &TextStyle {
        &self.text_styles[role.index()]
    }

    /// Returns the framed-surface block style.
    pub const fn get_panel(&self) -> &BlockStyle {
        &self.panel
    }

    /// Returns the focused framed-surface block style.
    pub const fn get_panel_focused(&self) -> &BlockStyle {
        &self.panel_focused
    }

    /// Returns the style assigned to one List-specific role.
    pub fn get_list_style(&self, role: ListRole) -> &TextStyle {
        self.list.get_style(role)
    }

    /// Returns the style assigned to one Tree-specific role.
    pub fn get_tree_style(&self, role: TreeRole) -> &TextStyle {
        self.tree.get_style(role)
    }

    /// Returns the block style assigned to one Table-specific cell role.
    pub fn get_table_style(&self, role: TableRole) -> &BlockStyle {
        self.table.get_style(role)
    }

    /// Returns the default presentation policy for lists.
    pub fn list(&self) -> &ListPresentation {
        &self.list
    }

    /// Returns the default presentation policy for trees.
    pub fn tree(&self) -> &TreePresentation {
        &self.tree
    }

    /// Returns the default presentation policy for tables.
    pub fn table(&self) -> &TablePresentation {
        &self.table
    }

    #[must_use]
    pub fn text_style(mut self, role: ComponentRole, style: TextStyle) -> Self {
        self.text_styles[role.index()] = style;
        self
    }

    /// Replaces the framed-surface block style.
    #[must_use]
    pub fn panel(mut self, panel: BlockStyle) -> Self {
        self.panel = panel;
        self
    }

    /// Replaces the focused framed-surface block style.
    #[must_use]
    pub fn panel_focused(mut self, panel: BlockStyle) -> Self {
        self.panel_focused = panel;
        self
    }

    /// Replaces the style assigned to one List-specific role.
    #[must_use]
    pub fn list_style(mut self, role: ListRole, style: TextStyle) -> Self {
        self.list = self.list.style(role, style);
        self
    }

    /// Replaces the style assigned to one Tree-specific role.
    #[must_use]
    pub fn tree_style(mut self, role: TreeRole, style: TextStyle) -> Self {
        self.tree = self.tree.style(role, style);
        self
    }

    /// Replaces the block style assigned to one Table-specific cell role.
    #[must_use]
    pub fn table_style(mut self, role: TableRole, style: BlockStyle) -> Self {
        self.table = self.table.style(role, style);
        self
    }
}
