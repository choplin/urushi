//! Standard style mapping for reusable component roles.

use crate::{BlockStyle, Border, ListPresentation, TablePresentation, TextStyle, TreePresentation};

use super::{ComponentRole, ListRole, SemanticTokens, TableRole, TreeRole};

const COMPONENT_ROLE_COUNT: usize = 16;

/// Styles for common components, indexed by [`ComponentRole`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentStyles {
    styles: [TextStyle; COMPONENT_ROLE_COUNT],
    panel: BlockStyle,
    panel_focused: BlockStyle,
    list: ListPresentation,
    tree: TreePresentation,
    table: TablePresentation,
}

impl ComponentStyles {
    /// Builds the standard component styles from semantic tokens.
    pub fn from_tokens(tokens: &SemanticTokens) -> Self {
        let panel = BlockStyle::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .border(Border::ROUNDED)
            .border_foreground(tokens.border)
            .padding((0, 1));

        Self {
            styles: [
                TextStyle::new().foreground(tokens.text),
                TextStyle::new().foreground(tokens.text_muted).dim(),
                TextStyle::new().foreground(tokens.accent).bold(),
                TextStyle::new().foreground(tokens.success),
                TextStyle::new().foreground(tokens.warning),
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
            ],
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

    pub fn text_style(&self, role: ComponentRole) -> &TextStyle {
        &self.styles[role.index()]
    }

    /// Returns the framed-surface block style.
    pub const fn panel(&self) -> &BlockStyle {
        &self.panel
    }

    /// Returns the focused framed-surface block style.
    pub const fn panel_focused(&self) -> &BlockStyle {
        &self.panel_focused
    }

    /// Returns the style assigned to one List-specific role.
    pub fn list_style(&self, role: ListRole) -> &TextStyle {
        self.list.style(role)
    }

    /// Returns the style assigned to one Tree-specific role.
    pub fn tree_style(&self, role: TreeRole) -> &TextStyle {
        self.tree.style(role)
    }

    /// Returns the block style assigned to one Table-specific cell role.
    pub fn table_style(&self, role: TableRole) -> &BlockStyle {
        self.table.style(role)
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
    pub fn with_text_style(mut self, role: ComponentRole, style: TextStyle) -> Self {
        self.styles[role.index()] = style;
        self
    }

    /// Replaces the framed-surface block style.
    #[must_use]
    pub fn with_panel(mut self, panel: BlockStyle) -> Self {
        self.panel = panel;
        self
    }

    /// Replaces the focused framed-surface block style.
    #[must_use]
    pub fn with_panel_focused(mut self, panel: BlockStyle) -> Self {
        self.panel_focused = panel;
        self
    }

    /// Replaces the style assigned to one List-specific role.
    #[must_use]
    pub fn with_list_style(mut self, role: ListRole, style: TextStyle) -> Self {
        self.list = self.list.with_style(role, style);
        self
    }

    /// Replaces the style assigned to one Tree-specific role.
    #[must_use]
    pub fn with_tree_style(mut self, role: TreeRole, style: TextStyle) -> Self {
        self.tree = self.tree.with_style(role, style);
        self
    }

    /// Replaces the block style assigned to one Table-specific cell role.
    #[must_use]
    pub fn with_table_style(mut self, role: TableRole, style: BlockStyle) -> Self {
        self.table = self.table.with_style(role, style);
        self
    }

    /// Replaces the complete default list presentation policy.
    #[must_use]
    pub fn with_list(mut self, list: ListPresentation) -> Self {
        self.list = list;
        self
    }

    /// Replaces the complete default tree presentation policy.
    #[must_use]
    pub fn with_tree(mut self, tree: TreePresentation) -> Self {
        self.tree = tree;
        self
    }

    /// Replaces the complete default table presentation policy.
    #[must_use]
    pub fn with_table(mut self, table: TablePresentation) -> Self {
        self.table = table;
        self
    }
}
