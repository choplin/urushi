//! Terminal capability detection and deterministic style degradation.

use std::io::IsTerminal;

use crate::{Color, Style};

/// The color fidelity available to a terminal renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorProfile {
    /// Preserve 24-bit RGB colors.
    TrueColor,
    /// Convert RGB colors to the canonical xterm 256-color palette.
    Ansi256,
    /// Convert every color to the canonical xterm 16-color palette.
    Ansi16,
    /// Remove every color while retaining modifiers when ANSI is enabled.
    Monochrome,
}

/// Whether urushi may emit ANSI SGR sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnsiPolicy {
    /// Preserve text modifiers after color resolution.
    Enabled,
    /// Remove colors and text modifiers so rendering cannot emit SGR.
    Disabled,
}

/// The terminal capabilities used to resolve a logical [`Style`].
///
/// Construct an explicit override with [`TerminalProfile::new`], or detect
/// the capabilities of the exact writer that will receive output with
/// [`TerminalProfile::detect_for`]. A terminal profile never selects a theme
/// or retains the writer used for detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalProfile {
    color_profile: ColorProfile,
    ansi_policy: AnsiPolicy,
}

impl TerminalProfile {
    /// Constructs an explicit terminal profile without inspecting the writer
    /// or process environment.
    pub const fn new(color_profile: ColorProfile, ansi_policy: AnsiPolicy) -> Self {
        Self {
            color_profile,
            ansi_policy,
        }
    }

    /// Detects capabilities for the writer that will receive ANSI output.
    ///
    /// Detection reads the writer's TTY state and the current `NO_COLOR`,
    /// `TERM`, and `COLORTERM` environment variables each time it is called.
    /// A non-TTY writer and `TERM=dumb` disable ANSI entirely. A non-empty
    /// `NO_COLOR` value removes colors but retains text modifiers.
    pub fn detect_for(output: &impl IsTerminal) -> Self {
        detect(
            output.is_terminal(),
            std::env::var("NO_COLOR").ok().as_deref(),
            std::env::var("TERM").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
        )
    }

    /// Returns the detected or explicitly selected color profile.
    pub const fn color_profile(&self) -> ColorProfile {
        self.color_profile
    }

    /// Returns whether resolved styles may emit ANSI SGR sequences.
    pub const fn ansi_policy(&self) -> AnsiPolicy {
        self.ansi_policy
    }

    /// Resolves a logical style for this terminal's color and ANSI support.
    ///
    /// Color resolution changes foreground, background, and border colors
    /// only. Padding, margins, borders, glyphs, width, alignment, and visible
    /// content are preserved. [`AnsiPolicy::Disabled`] additionally removes
    /// all text modifiers, so rendering the returned style emits no SGR.
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

fn quantize_to_ansi256(color: Color) -> Color {
    match color {
        Color::Ansi(_) | Color::Ansi256(_) => color,
        Color::Rgb(r, g, b) => color_from_index(nearest_palette_index((r, g, b), 256)),
    }
}

fn quantize_to_ansi16(color: Color) -> Color {
    let rgb = color_to_rgb(color);
    Color::Ansi(nearest_palette_index(rgb, 16))
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Ansi(index) | Color::Ansi256(index) => xterm_rgb(index),
    }
}

fn color_from_index(index: u8) -> Color {
    if index < 16 {
        Color::Ansi(index)
    } else {
        Color::Ansi256(index)
    }
}

fn nearest_palette_index(rgb: (u8, u8, u8), palette_size: u16) -> u8 {
    let mut nearest = 0;
    let mut nearest_distance = squared_distance(rgb, xterm_rgb(0));

    for index in 1..palette_size {
        let index = index as u8;
        let distance = squared_distance(rgb, xterm_rgb(index));
        if distance < nearest_distance {
            nearest = index;
            nearest_distance = distance;
        }
    }
    nearest
}

fn squared_distance((r, g, b): (u8, u8, u8), (cr, cg, cb): (u8, u8, u8)) -> u32 {
    let dr = i32::from(r) - i32::from(cr);
    let dg = i32::from(g) - i32::from(cg);
    let db = i32::from(b) - i32::from(cb);
    (dr * dr + dg * dg + db * db) as u32
}

fn xterm_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (128, 0, 0),
        (0, 128, 0),
        (128, 128, 0),
        (0, 0, 128),
        (128, 0, 128),
        (0, 128, 128),
        (192, 192, 192),
        (128, 128, 128),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (0, 0, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

    match index {
        0..=15 => ANSI[usize::from(index)],
        16..=231 => {
            let cube = index - 16;
            let r = cube / 36;
            let g = (cube % 36) / 6;
            let b = cube % 6;
            (
                CUBE_LEVELS[usize::from(r)],
                CUBE_LEVELS[usize::from(g)],
                CUBE_LEVELS[usize::from(b)],
            )
        }
        232..=255 => {
            let gray = 8 + 10 * (index - 232);
            (gray, gray, gray)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Align, Border};

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
                Some("xterm"),
                Some("TRUECOLOR"),
                ColorProfile::TrueColor,
                AnsiPolicy::Enabled,
            ),
            (
                true,
                None,
                Some("xterm"),
                Some("24bit"),
                ColorProfile::TrueColor,
                AnsiPolicy::Enabled,
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
                Some("screen-256color"),
                Some("unknown"),
                ColorProfile::Ansi256,
                AnsiPolicy::Enabled,
            ),
            (
                true,
                None,
                Some("xterm-color"),
                None,
                ColorProfile::Ansi16,
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
    fn detection_does_not_trim_or_case_fold_no_color() {
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
    fn profiles_quantize_xterm_colors_deterministically() {
        let rgb = Style::new().foreground(Color::Rgb(95, 135, 175));
        assert_eq!(
            TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled)
                .resolve_style(&rgb)
                .render("x"),
            "\x1b[38;2;95;135;175mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled)
                .resolve_style(&rgb)
                .render("x"),
            "\x1b[38;5;67mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Rgb(0, 0, 0)))
                .render("x"),
            "\x1b[30mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Rgb(128, 128, 128)))
                .render("x"),
            "\x1b[90mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi256, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Ansi256(212)))
                .render("x"),
            "\x1b[38;5;212mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Rgb(255, 0, 0)))
                .render("x"),
            "\x1b[91mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Ansi256(16)))
                .render("x"),
            "\x1b[30mx\x1b[0m"
        );
        assert_eq!(
            TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled)
                .resolve_style(&Style::new().foreground(Color::Ansi(16)))
                .render("x"),
            "\x1b[30mx\x1b[0m"
        );
    }

    #[test]
    fn monochrome_and_disabled_preserve_layout_as_specified() {
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
        assert!(!disabled.contains("\x1b["));
    }
}
