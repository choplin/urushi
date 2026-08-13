//! Titled warning messages with a rail-aligned body.

use crate::{ComponentRole, ComponentStyles, Line, View, text::wrap_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    title: String,
    message: String,
}

impl Warning {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn view(&self, styles: &ComponentStyles, width: usize) -> View {
        let muted = styles.style(ComponentRole::Muted).clone();
        let warning = styles.style(ComponentRole::Warning).clone();
        let body = styles.style(ComponentRole::Body).clone();
        let mut view = View::line(Line::styled("│", muted.clone())).push(
            Line::new()
                .span("▲", warning.clone())
                .span("  ", muted.clone())
                .span(self.title.clone(), warning),
        );
        for line in wrap_text(&self.message, width.saturating_sub(3).max(1)) {
            view = view.push(
                Line::new()
                    .span("│", muted.clone())
                    .span("  ", muted.clone())
                    .span(line, body.clone()),
            );
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, SemanticTokens, Theme};

    #[test]
    fn returns_a_renderer_neutral_view() {
        let theme = Theme::from_tokens(SemanticTokens {
            text: Color::Ansi(7),
            text_muted: Color::Ansi(8),
            background: Color::Ansi(0),
            surface: Color::Ansi(0),
            accent: Color::Ansi(6),
            accent_text: Color::Ansi(0),
            success: Color::Ansi(2),
            warning: Color::Ansi(3),
            error: Color::Ansi(1),
            border: Color::Ansi(8),
        });
        let view = Warning::new("Caution", "Careful").view(theme.components(), 80);
        assert_eq!(view.lines()[1].spans()[2].text(), "Caution");
        assert_eq!(view.lines()[2].spans()[2].text(), "Careful");
    }
}
