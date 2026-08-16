//! Titled warning messages with a rail-aligned body.

use crate::text::{PrintableLines, wrap_text};
use crate::{Align, ComponentRole, ComponentStyles, VerticalAlign, View};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    title: String,
    message: String,
}

impl Warning {
    /// Creates a warning from its title and message.
    ///
    /// `title` and `message` are plain text. Escape sequences and cursor movement in it break that contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Adopt
    /// already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    ///
    /// Style the component through its [`ComponentStyles`](crate::ComponentStyles)
    /// rather than by pre-rendering its content.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn view(&self, styles: &ComponentStyles, width: usize) -> View {
        let muted = styles.text_style(ComponentRole::Muted).clone();
        let warning = styles.text_style(ComponentRole::Warning).clone();
        let body = styles.text_style(ComponentRole::Body).clone();
        let mut rows = vec![
            View::text("│", muted.clone()),
            View::row(
                VerticalAlign::Top,
                [
                    View::text("▲", warning.clone()),
                    View::text("  ", muted.clone()),
                    View::text(self.title.clone(), warning),
                ],
            ),
        ];
        for line in wrap_text(
            PrintableLines::new(&self.message),
            width.saturating_sub(3).max(1),
        ) {
            rows.push(View::row(
                VerticalAlign::Top,
                [
                    View::text("│", muted.clone()),
                    View::text("  ", muted.clone()),
                    View::text(line, body.clone()),
                ],
            ));
        }
        View::column(Align::Left, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::plain;
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
        assert_eq!(plain(&view), "│\n▲  Caution\n│  Careful");
    }
}
