//! Conversion of logical text styles to ratatui styles.

use ratatui::style::{Color as RatatuiColor, Modifier, Style as InnerStyle};

use crate::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatatuiStyle {
    content: InnerStyle,
    border: InnerStyle,
}

impl RatatuiStyle {
    pub const fn into_inner(self) -> InnerStyle {
        self.content
    }
    pub const fn border_style(self) -> InnerStyle {
        self.border
    }
}

impl From<&Style> for RatatuiStyle {
    fn from(value: &Style) -> Self {
        let parts = value.stylable_parts();
        let mut style = InnerStyle::new();
        if let Some(color) = parts.foreground {
            style = style.fg(convert_color(color));
        }
        if let Some(color) = parts.background {
            style = style.bg(convert_color(color));
        }
        let mut modifiers = Modifier::empty();
        for (enabled, modifier) in [
            (parts.bold, Modifier::BOLD),
            (parts.dim, Modifier::DIM),
            (parts.italic, Modifier::ITALIC),
            (parts.underline, Modifier::UNDERLINED),
            (parts.blink, Modifier::SLOW_BLINK),
            (parts.reverse, Modifier::REVERSED),
            (parts.strikethrough, Modifier::CROSSED_OUT),
        ] {
            if enabled {
                modifiers.insert(modifier);
            }
        }
        let parts = value.box_parts();
        let mut border = InnerStyle::new();
        if let Some(color) = parts.border_foreground {
            border = border.fg(convert_color(color));
        }
        if let Some(color) = parts.border_background {
            border = border.bg(convert_color(color));
        }
        Self {
            content: style.add_modifier(modifiers),
            border,
        }
    }
}

impl From<RatatuiStyle> for InnerStyle {
    fn from(value: RatatuiStyle) -> Self {
        value.into_inner()
    }
}

pub(super) const fn convert_color(color: Color) -> RatatuiColor {
    match color {
        Color::Ansi(0) => RatatuiColor::Black,
        Color::Ansi(1) => RatatuiColor::Red,
        Color::Ansi(2) => RatatuiColor::Green,
        Color::Ansi(3) => RatatuiColor::Yellow,
        Color::Ansi(4) => RatatuiColor::Blue,
        Color::Ansi(5) => RatatuiColor::Magenta,
        Color::Ansi(6) => RatatuiColor::Cyan,
        Color::Ansi(7) => RatatuiColor::Gray,
        Color::Ansi(8) => RatatuiColor::DarkGray,
        Color::Ansi(9) => RatatuiColor::LightRed,
        Color::Ansi(10) => RatatuiColor::LightGreen,
        Color::Ansi(11) => RatatuiColor::LightYellow,
        Color::Ansi(12) => RatatuiColor::LightBlue,
        Color::Ansi(13) => RatatuiColor::LightMagenta,
        Color::Ansi(14) => RatatuiColor::LightCyan,
        Color::Ansi(15) => RatatuiColor::White,
        Color::Ansi(index) | Color::Ansi256(index) => RatatuiColor::Indexed(index),
        Color::Rgb(red, green, blue) => RatatuiColor::Rgb(red, green, blue),
    }
}
