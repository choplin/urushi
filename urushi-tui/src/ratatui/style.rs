//! Conversion of logical text styles to ratatui styles.

use ::ratatui::style::{Color as RatatuiColor, Modifier as RatatuiModifier, Style as InnerStyle};

use urushi::{Color, Modifier, TextStyle};

/// One logical [`TextStyle`] as a Ratatui style.
///
/// This is the whole style boundary of the adapter. Geometry never reaches it:
/// a [`ResolvedView`](urushi::ResolvedView) carries only `TextStyle` per
/// grapheme, and a block's border colors arrive as the border graphemes' own
/// text style, so the adapter never distinguishes a border from its content.
///
/// The wrapper exists because `Style` and `TextStyle` are both foreign to this
/// crate, so `impl From<&TextStyle> for ratatui::Style` is not allowed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatatuiStyle {
    content: InnerStyle,
}

impl RatatuiStyle {
    pub const fn into_inner(self) -> InnerStyle {
        self.content
    }
}

impl From<&TextStyle> for RatatuiStyle {
    fn from(value: &TextStyle) -> Self {
        let mut style = InnerStyle::new();
        if let Some(color) = value.foreground_color() {
            style = style.fg(convert_color(color));
        }
        if let Some(color) = value.background_color() {
            style = style.bg(convert_color(color));
        }
        // Ratatui has no underline shape, and its underline color lives behind
        // a feature that pulls in a backend this adapter does not depend on, so
        // every underline degrades to the plain `UNDERLINED` modifier. The
        // degradation is deterministic: two styles differing only in underline
        // shape or color reach Ratatui as the same style.
        if value.underline_value().is_some() {
            style = style.add_modifier(RatatuiModifier::UNDERLINED);
        }
        Self {
            content: style.add_modifier(convert_modifier(value.modifiers())),
        }
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
    if modifier.contains(Modifier::SLOW_BLINK) {
        converted = converted.union(RatatuiModifier::SLOW_BLINK);
    }
    if modifier.contains(Modifier::REVERSED) {
        converted = converted.union(RatatuiModifier::REVERSED);
    }
    if modifier.contains(Modifier::HIDDEN) {
        converted = converted.union(RatatuiModifier::HIDDEN);
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
    use urushi::UnderlineStyle;

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

    #[test]
    fn every_underline_degrades_to_the_plain_underlined_modifier() {
        // Ratatui has no underline shape, so the five shapes and any underline
        // color arrive as one modifier. Fixing the degradation here is what
        // keeps it from silently becoming something else.
        let styles = [
            TextStyle::new().underline(),
            TextStyle::new().underline_style(UnderlineStyle::Double),
            TextStyle::new().underline_style(UnderlineStyle::Curly),
            TextStyle::new().underline_style(UnderlineStyle::Dotted),
            TextStyle::new().underline_style(UnderlineStyle::Dashed),
            TextStyle::new().underline_color(Color::RED),
        ];

        for style in styles {
            let converted = RatatuiStyle::from(&style).into_inner();

            assert_eq!(converted.add_modifier, RatatuiModifier::UNDERLINED);
            assert_eq!(converted.fg, None);
        }

        assert!(
            RatatuiStyle::from(&TextStyle::new())
                .into_inner()
                .add_modifier
                .is_empty()
        );
    }
}
