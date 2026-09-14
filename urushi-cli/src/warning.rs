//! Titled warning messages with a rail-aligned body.

use urushi::{Align, BlockStyle, Border, TextStyle, VerticalAlign, View};

use crate::CliRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    title: String,
    message: String,
}

impl Warning {
    /// Creates a warning from a plain-text title and message.
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Presentation policy used to compose a [`Warning`] into a [`View`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarningPresentation {
    muted: TextStyle,
    warning: TextStyle,
    body: TextStyle,
}

impl WarningPresentation {
    /// Creates the canonical warning presentation from its three text roles.
    pub fn new(muted: TextStyle, warning: TextStyle, body: TextStyle) -> Self {
        Self {
            muted,
            warning,
            body,
        }
    }

    /// Composes warning data without binding it to an available width.
    pub fn compose(&self, warning: &Warning) -> View {
        let rail = Border {
            left: '│',
            ..Border::HIDDEN
        };
        let body = View::block(
            BlockStyle::new()
                .border(rail)
                .border_top(false)
                .border_right(false)
                .border_bottom(false)
                .border_text_style(self.muted.clone()),
            View::row(
                VerticalAlign::Top,
                [
                    View::block(
                        BlockStyle::from_text_style(self.muted.clone()).width(2),
                        View::text("  ", self.muted.clone()),
                    ),
                    View::text(warning.message.clone(), self.body.clone()),
                ],
            ),
        );

        View::column(
            Align::Left,
            [
                View::text("│", self.muted.clone()),
                View::row(
                    VerticalAlign::Top,
                    [
                        View::text("▲", self.warning.clone()),
                        View::text("  ", self.muted.clone()),
                        View::text(warning.title.clone(), self.warning.clone()),
                    ],
                ),
                body,
            ],
        )
    }

    pub(crate) fn set_style(&mut self, role: CliRole, style: TextStyle) {
        match role {
            CliRole::Muted => self.muted = style,
            CliRole::Warning => self.warning = style,
            CliRole::Body => self.body = style,
            CliRole::Accent => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain_at, style_at_width};
    use urushi::{Available, Color, SemanticTokens, Theme, resolve};

    fn cli_theme() -> crate::CliTheme {
        crate::CliTheme::from_theme(&Theme::from_tokens(SemanticTokens {
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
        }))
    }

    #[test]
    fn preserves_the_canonical_appearance() {
        let view = cli_theme().warning(&Warning::new("Caution", "Careful"));

        assert_eq!(plain_at(&view, 80), "│\n▲  Caution\n│  Careful");
    }

    #[test]
    fn the_same_view_reflows_and_extends_its_border() {
        let view = cli_theme().warning(&Warning::new(
            "Caution",
            "the quick brown fox jumps over the lazy dog",
        ));

        assert_eq!(
            plain_at(&view, 24),
            "│\n▲  Caution\n│  the quick brown fox\n│  jumps over the lazy\n│  dog"
        );
        assert_eq!(
            plain_at(&view, 14),
            "│\n▲  Caution\n│  the quick\n│  brown fox\n│  jumps over\n│  the lazy\n│  dog"
        );
    }

    #[test]
    fn border_title_and_body_keep_their_complete_roles() {
        let muted = TextStyle::new().foreground(Color::BLUE).dim();
        let warning = TextStyle::new().foreground(Color::YELLOW).bold();
        let body = TextStyle::new().foreground(Color::WHITE);
        let view = WarningPresentation::new(muted.clone(), warning.clone(), body.clone())
            .compose(&Warning::new("Caution", "Careful"));
        let resolved = resolve(&view, Available::columns(20)).unwrap();

        assert_eq!(resolved.rows()[0][0].style(), &muted);
        assert_eq!(resolved.rows()[1][0].style(), &warning);
        assert_eq!(resolved.rows()[2][0].style(), &muted);
        assert_eq!(resolved.rows()[2][1].style(), &muted);
        assert_eq!(resolved.rows()[2][2].style(), &muted);
        assert_eq!(style_at_width(&view, 20, 2, 3), body);
    }
}
