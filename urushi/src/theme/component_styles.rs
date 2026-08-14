//! Standard style mapping for reusable component roles.

use crate::{Border, ListStyle, Style, TableStyle, TreeStyle};

use super::{ComponentRole, ListRole, SemanticTokens, TableRole, TreeRole};

const COMPONENT_ROLE_COUNT: usize = 18;

/// Styles for common components, indexed by [`ComponentRole`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentStyles {
    styles: [Style; COMPONENT_ROLE_COUNT],
    list: ListStyle,
    tree: TreeStyle,
    table: TableStyle,
}

impl ComponentStyles {
    /// Builds the standard component styles from semantic tokens.
    pub fn from_tokens(tokens: &SemanticTokens) -> Self {
        let panel = Style::new()
            .foreground(tokens.text)
            .background(tokens.surface)
            .border(Border::ROUNDED)
            .border_foreground(tokens.border)
            .padding((0, 1));

        Self {
            styles: [
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted).dim(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.success),
                Style::new().foreground(tokens.warning),
                Style::new().foreground(tokens.error).bold(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted).italic(),
                Style::new().foreground(tokens.accent).bold(),
                Style::new().foreground(tokens.text),
                Style::new()
                    .foreground(tokens.accent_text)
                    .background(tokens.accent)
                    .bold(),
                Style::new()
                    .foreground(tokens.text)
                    .background(tokens.surface),
                Style::new()
                    .foreground(tokens.accent_text)
                    .background(tokens.accent)
                    .bold(),
                Style::new().foreground(tokens.text_muted).dim(),
                Style::new().foreground(tokens.error),
                panel.clone(),
                panel.border_foreground(tokens.accent),
            ],
            list: ListStyle::new(
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted),
                Style::new().foreground(tokens.text_muted),
            ),
            tree: TreeStyle::new(
                Style::new().foreground(tokens.text).bold(),
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.text_muted),
                Style::new().foreground(tokens.text_muted),
            ),
            table: TableStyle::new(
                Style::new().foreground(tokens.text).bold(),
                Style::new().foreground(tokens.text),
                Style::new().foreground(tokens.border),
            ),
        }
    }

    pub fn style(&self, role: ComponentRole) -> &Style {
        &self.styles[role.index()]
    }

    /// Returns the style assigned to one List-specific role.
    pub fn list_style(&self, role: ListRole) -> &Style {
        self.list.style(role)
    }

    /// Returns the style assigned to one Tree-specific role.
    pub fn tree_style(&self, role: TreeRole) -> &Style {
        self.tree.style(role)
    }

    /// Returns the style assigned to one Table-specific role.
    pub fn table_style(&self, role: TableRole) -> &Style {
        self.table.style(role)
    }

    /// Returns the default presentation policy for lists.
    pub fn list(&self) -> &ListStyle {
        &self.list
    }

    /// Returns the default presentation policy for trees.
    pub fn tree(&self) -> &TreeStyle {
        &self.tree
    }

    /// Returns the default presentation policy for tables.
    pub fn table(&self) -> &TableStyle {
        &self.table
    }

    #[must_use]
    pub fn with_style(mut self, role: ComponentRole, style: Style) -> Self {
        self.styles[role.index()] = style;
        self
    }

    /// Replaces the style assigned to one List-specific role.
    #[must_use]
    pub fn with_list_style(mut self, role: ListRole, style: Style) -> Self {
        self.list = self.list.with_style(role, style);
        self
    }

    /// Replaces the style assigned to one Tree-specific role.
    #[must_use]
    pub fn with_tree_style(mut self, role: TreeRole, style: Style) -> Self {
        self.tree = self.tree.with_style(role, style);
        self
    }

    /// Replaces the style assigned to one Table-specific role.
    #[must_use]
    pub fn with_table_style(mut self, role: TableRole, style: Style) -> Self {
        self.table = self.table.with_style(role, style);
        self
    }

    /// Replaces the complete default list presentation policy.
    #[must_use]
    pub fn with_list(mut self, list: ListStyle) -> Self {
        self.list = list;
        self
    }

    /// Replaces the complete default tree presentation policy.
    #[must_use]
    pub fn with_tree(mut self, tree: TreeStyle) -> Self {
        self.tree = tree;
        self
    }

    /// Replaces the complete default table presentation policy.
    #[must_use]
    pub fn with_table(mut self, table: TableStyle) -> Self {
        self.table = table;
        self
    }
}
