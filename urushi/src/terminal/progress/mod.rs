//! Public progress lifecycle independent from its live rendering backend.

mod bar;
mod indicatif_backend;
mod spinner;
mod view;

pub use bar::ProgressBar;
pub use spinner::Spinner;

use crate::{ComponentRole, ComponentTheme};

use super::StderrTerminal;

fn themed_message(terminal: &StderrTerminal, styles: &ComponentTheme, message: &str) -> String {
    terminal
        .renderer
        .render(&crate::View::text(
            message,
            styles.text_style(ComponentRole::Body).clone(),
        ))
        .into_string()
}

#[cfg(test)]
mod tests {
    use crate::{AnsiPolicy, Color, ColorProfile, SemanticTokens, TerminalProfile, Theme};

    use super::*;
    use crate::OutputMode;

    const TOKENS: SemanticTokens = SemanticTokens {
        text: Color::Ansi(2),
        text_muted: Color::Ansi(8),
        background: Color::Ansi(0),
        surface: Color::Ansi(0),
        accent: Color::Ansi(6),
        accent_text: Color::Ansi(0),
        success: Color::Ansi(2),
        warning: Color::Ansi(3),
        error: Color::Ansi(1),
        border: Color::Ansi(8),
    };

    #[test]
    fn live_messages_use_the_same_body_role_as_stable_views() {
        let theme = Theme::from_tokens(TOKENS);
        let terminal = StderrTerminal::new(
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled),
            OutputMode::Live,
            80,
        );

        assert_eq!(
            themed_message(&terminal, theme.components(), "Working"),
            "\x1b[32mWorking\x1b[0m"
        );
    }

    #[test]
    fn live_messages_follow_disabled_ansi_policy() {
        let theme = Theme::from_tokens(TOKENS);
        let terminal = StderrTerminal::new(
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled),
            OutputMode::Live,
            80,
        );

        assert_eq!(
            themed_message(&terminal, theme.components(), "Working"),
            "Working"
        );
    }
}
