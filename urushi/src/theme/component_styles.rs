//! Standard style mapping for reusable component roles.

use crate::{Border, Style};

use super::{ComponentRole, SemanticTokens};

const COMPONENT_ROLE_COUNT: usize = 18;

/// Styles for common components, indexed by [`ComponentRole`].
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentStyles {
    styles: [Style; COMPONENT_ROLE_COUNT],
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
        }
    }

    pub fn style(&self, role: ComponentRole) -> &Style {
        &self.styles[role.index()]
    }

    #[must_use]
    pub fn with_style(mut self, role: ComponentRole, style: Style) -> Self {
        self.styles[role.index()] = style;
        self
    }
}
