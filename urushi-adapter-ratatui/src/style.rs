//! Conversion of logical text styles to Ratatui styles.

use ::ratatui::style::{Color as RatatuiColor, Modifier as RatatuiModifier, Style as InnerStyle};

use urushi::{Color, TerminalTextStyle, TextAttribute, TextAttributes, TextStyle};

/// One logical [`TextStyle`] as a Ratatui style.
///
/// This is the whole style boundary of the adapter. Geometry never reaches it:
/// a [`ResolvedView`](urushi::ResolvedView) carries only `TextStyle` per
/// grapheme, and a block's border colors arrive as the border graphemes' own
/// text style, so the adapter never distinguishes a border from its content.
/// OSC 8 hyperlinks are intentionally discarded: Ratatui's cell style has no
/// field that can retain a link target or its parameters.
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
        let terminal = TerminalTextStyle::from(value).style();
        let mut style = InnerStyle::new();
        if let Some(color) = ratatui_color(terminal.foreground) {
            style = style.fg(color);
        }
        if let Some(color) = ratatui_color(terminal.background) {
            style = style.bg(color);
        }
        // Ratatui has no underline shape, and its underline color lives behind
        // a feature that pulls in a backend this adapter does not depend on, so
        // every underline degrades to the plain `UNDERLINED` modifier. The
        // degradation is deterministic: two styles differing only in underline
        // shape or color reach Ratatui as the same style.
        if terminal.underline.is_some() {
            style = style.add_modifier(RatatuiModifier::UNDERLINED);
        }
        Self {
            content: style.add_modifier(ratatui_attributes(terminal.attributes)),
        }
    }
}

fn ratatui_attributes(attributes: TextAttributes) -> RatatuiModifier {
    let mut converted = RatatuiModifier::empty();
    for attribute in attributes {
        converted = converted.union(match attribute {
            TextAttribute::Bold => RatatuiModifier::BOLD,
            TextAttribute::Dim => RatatuiModifier::DIM,
            TextAttribute::Italic => RatatuiModifier::ITALIC,
            TextAttribute::SlowBlink => RatatuiModifier::SLOW_BLINK,
            TextAttribute::RapidBlink => RatatuiModifier::RAPID_BLINK,
            TextAttribute::Reversed => RatatuiModifier::REVERSED,
            TextAttribute::Hidden => RatatuiModifier::HIDDEN,
            TextAttribute::CrossedOut => RatatuiModifier::CROSSED_OUT,
            TextAttribute::Fraktur
            | TextAttribute::Framed
            | TextAttribute::Encircled
            | TextAttribute::Overlined => RatatuiModifier::empty(),
        });
    }
    converted
}

impl From<RatatuiStyle> for InnerStyle {
    fn from(value: RatatuiStyle) -> Self {
        value.into_inner()
    }
}

const fn ratatui_color(color: Option<Color>) -> Option<RatatuiColor> {
    match color {
        None => None,
        Some(Color::Ansi(0)) => Some(RatatuiColor::Black),
        Some(Color::Ansi(1)) => Some(RatatuiColor::Red),
        Some(Color::Ansi(2)) => Some(RatatuiColor::Green),
        Some(Color::Ansi(3)) => Some(RatatuiColor::Yellow),
        Some(Color::Ansi(4)) => Some(RatatuiColor::Blue),
        Some(Color::Ansi(5)) => Some(RatatuiColor::Magenta),
        Some(Color::Ansi(6)) => Some(RatatuiColor::Cyan),
        Some(Color::Ansi(7)) => Some(RatatuiColor::Gray),
        Some(Color::Ansi(8)) => Some(RatatuiColor::DarkGray),
        Some(Color::Ansi(9)) => Some(RatatuiColor::LightRed),
        Some(Color::Ansi(10)) => Some(RatatuiColor::LightGreen),
        Some(Color::Ansi(11)) => Some(RatatuiColor::LightYellow),
        Some(Color::Ansi(12)) => Some(RatatuiColor::LightBlue),
        Some(Color::Ansi(13)) => Some(RatatuiColor::LightMagenta),
        Some(Color::Ansi(14)) => Some(RatatuiColor::LightCyan),
        Some(Color::Ansi(15)) => Some(RatatuiColor::White),
        Some(Color::Ansi(index) | Color::Ansi256(index)) => Some(RatatuiColor::Indexed(index)),
        Some(Color::Rgb(red, green, blue)) => Some(RatatuiColor::Rgb(red, green, blue)),
    }
}

#[cfg(test)]
mod tests {
    use urushi::{Color, TextAttribute, UnderlineStyle};

    use super::*;

    #[test]
    fn converts_active_attributes() {
        let converted = RatatuiStyle::from(
            &TextStyle::new()
                .add_attributes(TextAttribute::Bold | TextAttribute::Italic)
                .remove_attribute(TextAttribute::Italic),
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
            TextStyle::new().underlined(),
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

    #[test]
    fn hyperlink_is_intentionally_lost_at_the_cell_style_boundary() {
        assert_eq!(
            RatatuiStyle::from(&TextStyle::new().hyperlink("https://example.com")),
            RatatuiStyle::from(&TextStyle::new())
        );
    }
}
