//! Terminal capability detection and deterministic style degradation.

use std::io::IsTerminal;

use crate::{BlockStyle, TextStyle};

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

    /// Degrades one text style to this terminal's capabilities.
    ///
    /// The result is canonical: degradation is the last stage that can change a
    /// color, and it is what makes two logical colors equal — a truecolor red
    /// and an indexed red both become `Ansi(1)` on a sixteen-color terminal —
    /// so folding before it would miss exactly the duplicates it creates.
    pub fn resolve_text_style(&self, style: &TextStyle) -> TextStyle {
        if self.ansi_policy == AnsiPolicy::Disabled {
            return TextStyle::new();
        }
        match self.color_profile {
            ColorProfile::TrueColor => style.clone(),
            ColorProfile::Ansi256 => style.clone().map_colors(quantize_to_ansi256),
            ColorProfile::Ansi16 => style.clone().map_colors(quantize_to_ansi16),
            ColorProfile::Monochrome => style.clone().without_colors(),
        }
        .canonical()
    }

    /// Degrades one block style to this terminal's capabilities, preserving its
    /// box model.
    pub fn resolve_block_style(&self, style: &BlockStyle) -> BlockStyle {
        if self.ansi_policy == AnsiPolicy::Disabled {
            return style.clone().without_ansi();
        }
        match self.color_profile {
            ColorProfile::TrueColor => style.clone(),
            ColorProfile::Ansi256 => style.clone().map_colors(quantize_to_ansi256),
            ColorProfile::Ansi16 => style.clone().map_colors(quantize_to_ansi16),
            ColorProfile::Monochrome => style.clone().without_colors(),
        }
        .canonical()
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
    use crate::{Align, Border, Color, Length, Sides, Underline, UnderlineStyle, VerticalAlign};

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
        let style = BlockStyle::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .border_foreground(Color::GREEN)
            .border_background(Color::YELLOW)
            .bold()
            .dim()
            .italic()
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::MAGENTA)
            .blink()
            .reverse()
            .hide()
            .strikethrough()
            .border(Border::ROUNDED)
            .padding((0, 1))
            // Both dimensions measure the outer box, so the border rows and
            // columns are inside them: a 10x3 box holds one 6-cell content row.
            .width(10)
            .height(3)
            .align(Align::Center)
            .align_vertical(VerticalAlign::Bottom);
        let monochrome = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
            .resolve_block_style(&style)
            .render("日本")
            .into_string();
        assert_eq!(
            monochrome,
            "╭────────╮\n│\x1b[1;2;3;4:3;5;7;8;9m  日本  \x1b[0m│\n╰────────╯"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
                .resolve_block_style(&style)
                .height_length(),
            Some(Length::Cells(3))
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
                .resolve_block_style(&style)
                .vertical_alignment(),
            VerticalAlign::Bottom
        );
        let disabled = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled)
            .resolve_block_style(&style)
            .render("日本")
            .into_string();
        assert_eq!(disabled, "╭────────╮\n│  日本  │\n╰────────╯");
        assert_eq!(
            TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled)
                .resolve_block_style(&style)
                .height_length(),
            Some(Length::Cells(3))
        );
    }

    #[test]
    fn degradation_maps_and_drops_the_underline_color_with_the_others() {
        let style = TextStyle::new()
            .foreground(Color::Rgb(0, 0, 255))
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::Rgb(255, 0, 0));

        // A quantized underline color must be quantized too: leaving a
        // truecolor underline on a sixteen-color terminal emits a sequence the
        // rest of the style no longer matches.
        let quantized = TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
            .resolve_text_style(&style);
        assert_eq!(
            quantized.underline_value(),
            Some(Underline {
                style: UnderlineStyle::Curly,
                color: Some(Color::BRIGHT_RED),
            })
        );
        assert_eq!(quantized.paint("t"), "\x1b[4:3;94;58;5;9mt\x1b[0m");

        // Monochrome keeps the underline, which is a shape, and drops its
        // color, which is not.
        let monochrome = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled)
            .resolve_text_style(&style);
        assert_eq!(
            monochrome.underline_value(),
            Some(Underline::new(UnderlineStyle::Curly))
        );
        assert_eq!(monochrome.paint("t"), "\x1b[4:3mt\x1b[0m");
    }

    #[test]
    fn degradation_folds_the_underline_color_it_makes_equal_to_the_foreground() {
        // Two distinct logical colors collapse onto `Ansi(1)` here, so the
        // duplicate only exists after degradation — folding earlier misses it.
        let style = TextStyle::new()
            .foreground(Color::Rgb(255, 0, 0))
            .underline_color(Color::Ansi256(196));

        let resolved = TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
            .resolve_text_style(&style);

        assert_eq!(resolved.underline_value(), Some(Underline::default()));
        assert_eq!(resolved.paint("t"), "\x1b[4;91mt\x1b[0m");
    }

    #[test]
    fn color_resolution_preserves_box_layout_values() {
        let style = BlockStyle::new()
            .foreground(Color::Rgb(1, 2, 3))
            .border(Border::ROUNDED)
            .border_top(false)
            .border_right(true)
            .border_bottom(true)
            .border_left(false)
            .padding((1, 2, 3, 4))
            .margin((4, 3, 2, 1))
            .width(12)
            .height(9)
            .max_width(10)
            .max_height(7)
            .align(Align::Right)
            .align_vertical(VerticalAlign::Center);

        for profile in [
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled),
            TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled),
            TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled),
        ] {
            let resolved = profile.resolve_block_style(&style);
            assert_eq!(resolved.border_kind(), Some(Border::ROUNDED));
            assert!(!resolved.is_border_top_enabled());
            assert!(resolved.is_border_right_enabled());
            assert!(resolved.is_border_bottom_enabled());
            assert!(!resolved.is_border_left_enabled());
            assert_eq!(resolved.padding_sides(), Sides::from((1, 2, 3, 4)));
            assert_eq!(resolved.margin_sides(), Sides::from((4, 3, 2, 1)));
            assert_eq!(resolved.width_length(), Some(Length::Cells(12)));
            assert_eq!(resolved.height_length(), Some(Length::Cells(9)));
            assert_eq!(resolved.maximum_width(), Some(10));
            assert_eq!(resolved.maximum_height(), Some(7));
            assert_eq!(resolved.horizontal_alignment(), Align::Right);
            assert_eq!(resolved.vertical_alignment(), VerticalAlign::Center);
        }
    }
}
