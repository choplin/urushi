//! CLI-specific theme roles and canonical presentations.

use urushi::{ComponentRole, TextStyle, Theme, View};

use crate::{Summary, SummaryPresentation, Warning, WarningPresentation};

const CLI_ROLE_COUNT: usize = 4;

/// A semantic text role in the canonical CLI presentation language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliRole {
    Body,
    Muted,
    Accent,
    Warning,
}

impl CliRole {
    const fn index(self) -> usize {
        match self {
            Self::Body => 0,
            Self::Muted => 1,
            Self::Accent => 2,
            Self::Warning => 3,
        }
    }
}

/// Theme-derived styles and canonical presentations for CLI output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliTheme {
    styles: [TextStyle; CLI_ROLE_COUNT],
    summary: SummaryPresentation,
    warning: WarningPresentation,
}

impl CliTheme {
    /// Derives the canonical CLI presentation language from a core theme.
    pub fn from_theme(theme: &Theme) -> Self {
        let styles = [
            theme.text_style(ComponentRole::Body),
            theme.text_style(ComponentRole::Muted),
            theme.text_style(ComponentRole::Accent),
            TextStyle::new().foreground(theme.tokens().warning),
        ];
        let summary = SummaryPresentation::new(
            styles[CliRole::Muted.index()].clone(),
            styles[CliRole::Accent.index()].clone(),
            styles[CliRole::Body.index()].clone(),
        );
        let warning = WarningPresentation::new(
            styles[CliRole::Muted.index()].clone(),
            styles[CliRole::Warning.index()].clone(),
            styles[CliRole::Body.index()].clone(),
        );
        Self {
            styles,
            summary,
            warning,
        }
    }

    /// Returns the style assigned to one CLI role.
    pub fn style(&self, role: CliRole) -> &TextStyle {
        &self.styles[role.index()]
    }

    /// Returns the canonical summary presentation.
    pub fn summary_presentation(&self) -> &SummaryPresentation {
        &self.summary
    }

    /// Returns the canonical warning presentation.
    pub fn warning_presentation(&self) -> &WarningPresentation {
        &self.warning
    }

    /// Composes a summary with the canonical presentation.
    pub fn summary(&self, summary: &Summary) -> View {
        self.summary.compose(summary)
    }

    /// Composes a warning with the canonical presentation.
    pub fn warning(&self, warning: &Warning) -> View {
        self.warning.compose(warning)
    }

    /// Replaces one CLI role and updates the canonical presentations that use it.
    #[must_use]
    pub fn with_style(mut self, role: CliRole, style: TextStyle) -> Self {
        self.styles[role.index()] = style.clone();
        self.summary.set_style(role, style.clone());
        self.warning.set_style(role, style);
        self
    }

    /// Replaces the complete canonical summary presentation.
    #[must_use]
    pub fn with_summary(mut self, summary: SummaryPresentation) -> Self {
        self.summary = summary;
        self
    }

    /// Replaces the complete canonical warning presentation.
    #[must_use]
    pub fn with_warning(mut self, warning: WarningPresentation) -> Self {
        self.warning = warning;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urushi::{Color, RenderSettings, SemanticTokens, StyledText, render_text};

    fn render_style(style: &TextStyle, text: &str) -> String {
        let settings = RenderSettings::all();
        render_text(&StyledText::new(text, style.clone()), &settings)
    }

    fn theme() -> Theme {
        Theme::from_tokens(SemanticTokens {
            text: Color::Ansi(1),
            text_muted: Color::Ansi(2),
            background: Color::Ansi(3),
            surface: Color::Ansi(4),
            accent: Color::Ansi(5),
            accent_text: Color::Ansi(6),
            success: Color::Ansi(7),
            warning: Color::Ansi(8),
            error: Color::Ansi(9),
            border: Color::Ansi(10),
        })
    }

    #[test]
    fn derives_cli_roles_from_the_core_theme() {
        let cli = CliTheme::from_theme(&theme());

        assert_eq!(
            render_style(cli.style(CliRole::Body), "x"),
            "\x1b[31mx\x1b[0m"
        );
        assert_eq!(
            render_style(cli.style(CliRole::Muted), "x"),
            "\x1b[2;32mx\x1b[0m"
        );
        assert_eq!(
            render_style(cli.style(CliRole::Accent), "x"),
            "\x1b[1;35mx\x1b[0m"
        );
        assert_eq!(
            render_style(cli.style(CliRole::Warning), "x"),
            "\x1b[90mx\x1b[0m"
        );
    }

    #[test]
    fn shortcuts_delegate_to_replaceable_presentations() {
        let theme = theme();
        let cli = CliTheme::from_theme(&theme);
        let summary = Summary::new("Done");
        let warning = Warning::new("Caution", "Careful");

        assert_eq!(
            cli.summary(&summary),
            cli.summary_presentation().compose(&summary)
        );
        assert_eq!(
            cli.warning(&warning),
            cli.warning_presentation().compose(&warning)
        );
    }

    #[test]
    fn roles_and_complete_presentations_are_replaceable() {
        let theme = theme();
        let base = CliTheme::from_theme(&theme);
        let changed_role = base
            .clone()
            .with_style(CliRole::Muted, TextStyle::new().underline());
        assert_eq!(
            changed_role.style(CliRole::Muted),
            &TextStyle::new().underline()
        );
        assert_ne!(
            changed_role.summary(&Summary::new("Done")),
            base.summary(&Summary::new("Done"))
        );

        let summary =
            SummaryPresentation::new(TextStyle::new().bold(), TextStyle::new(), TextStyle::new());
        let warning =
            WarningPresentation::new(TextStyle::new(), TextStyle::new().bold(), TextStyle::new());
        let replaced = base
            .with_summary(summary.clone())
            .with_warning(warning.clone());

        assert_eq!(replaced.summary_presentation(), &summary);
        assert_eq!(replaced.warning_presentation(), &warning);
    }
}
