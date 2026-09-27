//! Backend-independent terminal style primitives.

/// A color addressable by terminal text commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// Standard 4-bit ANSI color (0–15). Values 8–15 are bright variants.
    Ansi(u8),
    /// An entry in the xterm 256-color palette.
    Ansi256(u8),
    /// A 24-bit color.
    Rgb(u8, u8, u8),
}

impl Color {
    pub const BLACK: Self = Self::Ansi(0);
    pub const RED: Self = Self::Ansi(1);
    pub const GREEN: Self = Self::Ansi(2);
    pub const YELLOW: Self = Self::Ansi(3);
    pub const BLUE: Self = Self::Ansi(4);
    pub const MAGENTA: Self = Self::Ansi(5);
    pub const CYAN: Self = Self::Ansi(6);
    pub const WHITE: Self = Self::Ansi(7);
    pub const BRIGHT_BLACK: Self = Self::Ansi(8);
    pub const BRIGHT_RED: Self = Self::Ansi(9);
    pub const BRIGHT_GREEN: Self = Self::Ansi(10);
    pub const BRIGHT_YELLOW: Self = Self::Ansi(11);
    pub const BRIGHT_BLUE: Self = Self::Ansi(12);
    pub const BRIGHT_MAGENTA: Self = Self::Ansi(13);
    pub const BRIGHT_CYAN: Self = Self::Ansi(14);
    pub const BRIGHT_WHITE: Self = Self::Ansi(15);

    /// Parses `#rgb`, `#rrggbb`, or a decimal palette index from 0 to 255.
    pub fn parse(value: &str) -> Option<Self> {
        if let Some(hex) = value.strip_prefix('#') {
            return match hex.len() {
                3 => {
                    let mut digits = hex
                        .chars()
                        .map(|character| character.to_digit(16).map(|digit| (digit * 17) as u8));
                    Some(Self::Rgb(digits.next()??, digits.next()??, digits.next()??))
                }
                6 => Some(Self::Rgb(
                    u8::from_str_radix(&hex[0..2], 16).ok()?,
                    u8::from_str_radix(&hex[2..4], 16).ok()?,
                    u8::from_str_radix(&hex[4..6], 16).ok()?,
                )),
                _ => None,
            };
        }

        value.parse::<u8>().ok().map(Self::from)
    }
}

impl From<u8> for Color {
    fn from(index: u8) -> Self {
        if index < 16 {
            Self::Ansi(index)
        } else {
            Self::Ansi256(index)
        }
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((red, green, blue): (u8, u8, u8)) -> Self {
        Self::Rgb(red, green, blue)
    }
}

/// One independently selectable terminal text attribute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextAttribute {
    Bold,
    Dim,
    Italic,
    SlowBlink,
    RapidBlink,
    Reversed,
    Hidden,
    CrossedOut,
    Fraktur,
    Framed,
    Encircled,
    Overlined,
}

impl TextAttribute {
    const COUNT: usize = 12;

    const fn bit(self) -> u16 {
        1 << self as u16
    }

    const fn from_index(index: u32) -> Self {
        match index {
            0 => Self::Bold,
            1 => Self::Dim,
            2 => Self::Italic,
            3 => Self::SlowBlink,
            4 => Self::RapidBlink,
            5 => Self::Reversed,
            6 => Self::Hidden,
            7 => Self::CrossedOut,
            8 => Self::Fraktur,
            9 => Self::Framed,
            10 => Self::Encircled,
            11 => Self::Overlined,
            _ => unreachable!(),
        }
    }
}

/// A set of terminal text attributes with no duplicates or ordering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextAttributes(u16);

impl TextAttributes {
    const ALL_BITS: u16 = (1 << TextAttribute::COUNT) - 1;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    pub const fn from_attribute(attribute: TextAttribute) -> Self {
        Self(attribute.bit())
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & (Self::ALL_BITS ^ other.0))
    }

    pub const fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub const fn contains(self, attribute: TextAttribute) -> bool {
        self.0 & attribute.bit() != 0
    }

    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn iter(self) -> TextAttributeIter {
        TextAttributeIter { remaining: self.0 }
    }
}

impl From<TextAttribute> for TextAttributes {
    fn from(attribute: TextAttribute) -> Self {
        Self::from_attribute(attribute)
    }
}

impl std::ops::BitOr for TextAttribute {
    type Output = TextAttributes;

    fn bitor(self, rhs: Self) -> Self::Output {
        TextAttributes::from_attribute(self).union(TextAttributes::from_attribute(rhs))
    }
}

impl std::ops::BitOr<TextAttribute> for TextAttributes {
    type Output = Self;

    fn bitor(self, rhs: TextAttribute) -> Self::Output {
        self.union(rhs.into())
    }
}

impl std::ops::BitOr for TextAttributes {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Iterator over the attributes present in a [`TextAttributes`] value.
pub struct TextAttributeIter {
    remaining: u16,
}

impl Iterator for TextAttributeIter {
    type Item = TextAttribute;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        let index = self.remaining.trailing_zeros();
        self.remaining &= self.remaining - 1;
        Some(TextAttribute::from_index(index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.remaining.count_ones() as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for TextAttributeIter {}
impl std::iter::FusedIterator for TextAttributeIter {}

impl IntoIterator for TextAttributes {
    type Item = TextAttribute;
    type IntoIter = TextAttributeIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// The shape of an underline drawn by the terminal.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UnderlineStyle {
    #[default]
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// An underline and its optional color independent of the foreground.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Underline {
    style: UnderlineStyle,
    color: Option<Color>,
}

impl Underline {
    pub const fn new(style: UnderlineStyle) -> Self {
        Self { style, color: None }
    }

    pub const fn style(mut self, style: UnderlineStyle) -> Self {
        self.style = style;
        self
    }

    pub const fn reset_style(mut self) -> Self {
        self.style = UnderlineStyle::Single;
        self
    }

    pub fn color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub const fn reset_color(mut self) -> Self {
        self.color = None;
        self
    }

    pub const fn get_style(self) -> UnderlineStyle {
        self.style
    }

    pub const fn get_color(self) -> Option<Color> {
        self.color
    }
}

impl From<UnderlineStyle> for Underline {
    fn from(style: UnderlineStyle) -> Self {
        Self::new(style)
    }
}

/// The complete physical text style applied to terminal output.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TerminalStyle {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub attributes: TextAttributes,
    pub underline: Option<Underline>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_colors_without_a_rendering_dependency() {
        assert_eq!(Color::parse("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(Color::parse("#ff88cc"), Some(Color::Rgb(255, 136, 204)));
        assert_eq!(Color::parse("1"), Some(Color::Ansi(1)));
        assert_eq!(Color::parse("212"), Some(Color::Ansi256(212)));
        assert_eq!(Color::parse("256"), None);
    }

    #[test]
    fn attribute_sets_support_style_composition() {
        let emphasis = TextAttribute::Bold | TextAttribute::Italic;

        assert_eq!(
            emphasis.difference(TextAttribute::Italic.into()),
            TextAttribute::Bold.into()
        );
        assert_eq!(
            emphasis.intersection(TextAttribute::Italic | TextAttribute::Dim),
            TextAttribute::Italic.into()
        );
        assert_eq!(
            emphasis.iter().collect::<Vec<_>>(),
            [TextAttribute::Bold, TextAttribute::Italic]
        );
    }
}
