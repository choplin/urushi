//! Lowering from Urushi text styles to backend-independent terminal styles.

use urushi_terminal::TerminalStyle;

use crate::{Hyperlink, TextStyle};

/// A text style lowered to the terminal representation without discarding its hyperlink.
///
/// This conversion does not negotiate terminal capabilities. Callers rendering to a
/// particular terminal should first resolve the [`TextStyle`] with
/// [`RenderSettings`](crate::RenderSettings). Hyperlinks remain separate because they
/// are scoped OSC 8 commands rather than SGR cell attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerminalTextStyle<'a> {
    style: TerminalStyle,
    hyperlink: Option<&'a Hyperlink>,
}

impl<'a> TerminalTextStyle<'a> {
    /// Returns the physical color, attribute, and underline state.
    pub const fn style(self) -> TerminalStyle {
        self.style
    }

    /// Returns the hyperlink that must surround the styled text, if any.
    pub const fn hyperlink(self) -> Option<&'a Hyperlink> {
        self.hyperlink
    }
}

impl<'a> From<&'a TextStyle> for TerminalTextStyle<'a> {
    fn from(value: &'a TextStyle) -> Self {
        Self {
            style: TerminalStyle {
                foreground: value.get_foreground(),
                background: value.get_background(),
                attributes: value.get_attributes(),
                underline: value.get_underline(),
            },
            hyperlink: value.get_hyperlink(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, TextAttribute, Underline, UnderlineStyle};

    #[test]
    fn lowers_every_part_without_losing_the_hyperlink() {
        let source = TextStyle::new()
            .foreground(Color::Ansi(9))
            .background(Color::Ansi256(200))
            .add_attributes(TextAttribute::Bold | TextAttribute::Italic)
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::Rgb(1, 2, 3))
            .hyperlink(Hyperlink::new("https://example.com").parameter("id", "reference"));

        let lowered = TerminalTextStyle::from(&source);

        assert_eq!(
            lowered.style(),
            TerminalStyle {
                foreground: Some(Color::Ansi(9)),
                background: Some(Color::Ansi256(200)),
                attributes: TextAttribute::Bold | TextAttribute::Italic,
                underline: Some(Underline::new(UnderlineStyle::Curly).color(Color::Rgb(1, 2, 3)),),
            }
        );
        assert_eq!(
            lowered.hyperlink().map(Hyperlink::uri),
            Some("https://example.com")
        );
    }
}
