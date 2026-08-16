//! Conversion of logical text styles to ratatui styles.

use ratatui::style::{Color as RatatuiColor, Modifier as RatatuiModifier, Style as InnerStyle};

use urushi::{BlockStyle, Color, Modifier, TextStyle};

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

impl From<&TextStyle> for RatatuiStyle {
    /// Converts a text style. A [`TextStyle`] carries no geometry, so the border
    /// style stays empty.
    fn from(value: &TextStyle) -> Self {
        let mut style = InnerStyle::new();
        if let Some(color) = value.foreground_color() {
            style = style.fg(convert_color(color));
        }
        if let Some(color) = value.background_color() {
            style = style.bg(convert_color(color));
        }
        Self {
            content: style.add_modifier(convert_modifier(value.modifiers())),
            border: InnerStyle::new(),
        }
    }
}

impl From<&BlockStyle> for RatatuiStyle {
    /// Converts a block style: its fill becomes the content style and its
    /// border colors the border style.
    fn from(value: &BlockStyle) -> Self {
        let content = Self::from(value.text()).content;
        let mut border = InnerStyle::new();
        if let Some(color) = value.border_foreground_color() {
            border = border.fg(convert_color(color));
        }
        if let Some(color) = value.border_background_color() {
            border = border.bg(convert_color(color));
        }
        Self { content, border }
    }
}

const fn convert_modifier(modifier: Modifier) -> RatatuiModifier {
    let mut converted = RatatuiModifier::empty();
    if modifier.contains(Modifier::BOLD) {
        converted = converted.union(RatatuiModifier::BOLD);
    }
    if modifier.contains(Modifier::DIM) {
        converted = converted.union(RatatuiModifier::DIM);
    }
    if modifier.contains(Modifier::ITALIC) {
        converted = converted.union(RatatuiModifier::ITALIC);
    }
    if modifier.contains(Modifier::UNDERLINED) {
        converted = converted.union(RatatuiModifier::UNDERLINED);
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        converted = converted.union(RatatuiModifier::SLOW_BLINK);
    }
    if modifier.contains(Modifier::REVERSED) {
        converted = converted.union(RatatuiModifier::REVERSED);
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        converted = converted.union(RatatuiModifier::CROSSED_OUT);
    }
    converted
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_active_modifiers() {
        let converted = RatatuiStyle::from(
            &TextStyle::new()
                .add(Modifier::BOLD | Modifier::ITALIC)
                .remove(Modifier::ITALIC),
        )
        .into_inner();

        assert_eq!(converted.add_modifier, RatatuiModifier::BOLD);
        assert!(converted.sub_modifier.is_empty());
    }
}
