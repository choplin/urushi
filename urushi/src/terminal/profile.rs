//! Terminal capability detection and deterministic style degradation.

use std::io::IsTerminal;

use crate::Style;

use super::palette::{quantize_to_ansi16, quantize_to_ansi256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorProfile {
    TrueColor,
    Ansi256,
    Ansi16,
    Monochrome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiPolicy {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProfile {
    color_profile: ColorProfile,
    ansi_policy: AnsiPolicy,
}

impl TerminalProfile {
    pub const fn new(color_profile: ColorProfile, ansi_policy: AnsiPolicy) -> Self {
        Self {
            color_profile,
            ansi_policy,
        }
    }

    pub fn detect_for(output: &impl IsTerminal) -> Self {
        detect(
            output.is_terminal(),
            std::env::var("NO_COLOR").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
        )
    }

    pub const fn color_profile(&self) -> ColorProfile {
        self.color_profile
    }
    pub const fn ansi_policy(&self) -> AnsiPolicy {
        self.ansi_policy
    }

    pub fn resolve_style(&self, style: &Style) -> Style {
        if self.ansi_policy == AnsiPolicy::Disabled {
            return style.clone().without_ansi();
        }
        match self.color_profile {
            ColorProfile::TrueColor => style.clone(),
            ColorProfile::Ansi256 => style.clone().map_colors(quantize_to_ansi256),
            ColorProfile::Ansi16 => style.clone().map_colors(quantize_to_ansi16),
            ColorProfile::Monochrome => style.clone().without_colors(),
        }
    }
}

fn detect(
    is_tty: bool,
    no_color: Option<&str>,
    term: Option<&str>,
    color_term: Option<&str>,
) -> TerminalProfile {
    if !is_tty {
        return TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
    }
    if no_color.is_some_and(|value| !value.is_empty()) {
        return TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled);
    }
    if term.is_some_and(|value| value.eq_ignore_ascii_case("dumb")) {
        return TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
    }
    if color_term.is_some_and(|value| {
        value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit")
    }) {
        return TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
    }
    if term.is_some_and(|value| value.to_ascii_lowercase().contains("256color")) {
        return TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled);
    }
    TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Align, Border, Color};

    #[test]
    fn detection_obeys_precedence_and_empty_no_color() {
        let cases = [
            (
                false,
                None,
                Some("xterm-256color"),
                Some("truecolor"),
                ColorProfile::Monochrome,
                AnsiPolicy::Disabled,
            ),
            (
                true,
                Some("1"),
                Some("xterm-256color"),
                Some("truecolor"),
                ColorProfile::Monochrome,
                AnsiPolicy::Enabled,
            ),
            (
                true,
                Some(""),
                Some("xterm"),
                Some("truecolor"),
                ColorProfile::TrueColor,
                AnsiPolicy::Enabled,
            ),
            (
                true,
                None,
                Some("dumb"),
                Some("truecolor"),
                ColorProfile::Monochrome,
                AnsiPolicy::Disabled,
            ),
            (
                true,
                None,
                Some("xterm-256color"),
                None,
                ColorProfile::Ansi256,
                AnsiPolicy::Enabled,
            ),
            (
                true,
                None,
                None,
                None,
                ColorProfile::Ansi16,
                AnsiPolicy::Enabled,
            ),
        ];
        for (is_tty, no_color, term, color_term, color_profile, ansi_policy) in cases {
            assert_eq!(
                detect(is_tty, no_color, term, color_term),
                TerminalProfile::new(color_profile, ansi_policy)
            );
        }
    }

    #[test]
    fn detection_does_not_normalize_environment_values() {
        assert_eq!(
            detect(true, None, Some(" DUMB"), None),
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
        );
        assert_eq!(
            detect(true, Some(" "), Some("dumb"), None),
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
        );
    }

    #[test]
    fn monochrome_and_disabled_preserve_layout() {
        let style = Style::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .border_foreground(Color::GREEN)
            .border_background(Color::YELLOW)
            .bold()
            .dim()
            .italic()
            .underline()
            .blink()
            .reverse()
            .strikethrough()
            .border(Border::ROUNDED)
            .padding((0, 1))
            .width(8)
            .align(Align::Center);
        let monochrome = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
            .resolve_style(&style)
            .render("日本");
        assert_eq!(
            monochrome,
            "╭────────╮\n│\x1b[1;2;3;4;5;7;9m  日本  \x1b[0m│\n╰────────╯"
        );
        let disabled = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled)
            .resolve_style(&style)
            .render("日本");
        assert_eq!(disabled, "╭────────╮\n│  日本  │\n╰────────╯");
    }
}
