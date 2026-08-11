//! Optional interoperability with `ratatui`.

use ratatui::style::{Color as RatatuiColor, Modifier, Style as InnerStyle};

use crate::{Color, Style};

/// The stylable subset of an urushi [`Style`] converted for ratatui.
///
/// Foreground, background, and text modifiers are preserved. Box-model
/// properties (margin, border, padding, width, and alignment) and border
/// colors are intentionally omitted; use urushi's ratatui widget adapter when
/// those properties need to be rendered.
///
/// This urushi-owned wrapper provides an explicit, loss-aware conversion API
/// without requiring an orphan-rule-invalid foreign trait implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatatuiStyle(InnerStyle);

impl RatatuiStyle {
    /// Returns the underlying ratatui style.
    pub const fn into_inner(self) -> InnerStyle {
        self.0
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

        Self(style.add_modifier(modifiers))
    }
}

impl From<RatatuiStyle> for InnerStyle {
    fn from(value: RatatuiStyle) -> Self {
        value.into_inner()
    }
}

const fn convert_color(color: Color) -> RatatuiColor {
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
    fn converts_colors_and_every_modifier() {
        let converted = RatatuiStyle::from(
            &Style::new()
                .foreground(Color::Rgb(1, 2, 3))
                .background(Color::Ansi256(212))
                .bold()
                .dim()
                .italic()
                .underline()
                .blink()
                .reverse()
                .strikethrough(),
        )
        .into_inner();

        assert_eq!(converted.fg, Some(RatatuiColor::Rgb(1, 2, 3)));
        assert_eq!(converted.bg, Some(RatatuiColor::Indexed(212)));
        assert_eq!(
            converted.add_modifier,
            Modifier::BOLD
                | Modifier::DIM
                | Modifier::ITALIC
                | Modifier::UNDERLINED
                | Modifier::SLOW_BLINK
                | Modifier::REVERSED
                | Modifier::CROSSED_OUT
        );
    }

    #[test]
    fn maps_sixteen_color_palette_to_named_colors() {
        let expected = [
            RatatuiColor::Black,
            RatatuiColor::Red,
            RatatuiColor::Green,
            RatatuiColor::Yellow,
            RatatuiColor::Blue,
            RatatuiColor::Magenta,
            RatatuiColor::Cyan,
            RatatuiColor::Gray,
            RatatuiColor::DarkGray,
            RatatuiColor::LightRed,
            RatatuiColor::LightGreen,
            RatatuiColor::LightYellow,
            RatatuiColor::LightBlue,
            RatatuiColor::LightMagenta,
            RatatuiColor::LightCyan,
            RatatuiColor::White,
        ];

        for (index, expected) in expected.into_iter().enumerate() {
            let converted =
                RatatuiStyle::from(&Style::new().foreground(Color::Ansi(index as u8))).into_inner();
            assert_eq!(converted.fg, Some(expected));
        }
    }

    #[test]
    fn box_model_properties_are_not_in_the_subset() {
        let plain = RatatuiStyle::from(&Style::new()).into_inner();
        let boxed = RatatuiStyle::from(
            &Style::new()
                .padding(1)
                .margin(1)
                .border(crate::Border::ROUNDED)
                .border_foreground(Color::RED)
                .width(20)
                .align(crate::Align::Center),
        )
        .into_inner();

        assert_eq!(boxed, plain);
    }
}
