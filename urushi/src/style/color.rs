//! Terminal color types and their ANSI SGR encoding.

/// A terminal color.
///
/// Construct one directly, from a named constant, or by parsing a string:
///
/// ```
/// use urushi::Color;
///
/// let a = Color::RED;
/// let b = Color::Ansi256(212);
/// let c = Color::Rgb(0xfa, 0xfa, 0xfa);
/// let d = Color::parse("#ff88cc").unwrap();
/// let e = Color::parse("212").unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Color {
    /// Standard 4-bit ANSI color (0–15). Values 8–15 are the bright variants.
    Ansi(u8),
    /// 8-bit indexed color from the 256-color palette.
    Ansi256(u8),
    /// 24-bit true color.
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

    /// Parses a color from a hex string (`"#fac"` or `"#ffaacc"`) or a
    /// decimal palette index (`"0"`–`"255"`).
    ///
    /// Indexes below 16 become [`Color::Ansi`]; the rest become
    /// [`Color::Ansi256`].
    pub fn parse(s: &str) -> Option<Self> {
        if let Some(hex) = s.strip_prefix('#') {
            return match hex.len() {
                3 => {
                    let mut it = hex.chars().map(|c| c.to_digit(16).map(|d| (d * 17) as u8));
                    let r = it.next()??;
                    let g = it.next()??;
                    let b = it.next()??;
                    Some(Self::Rgb(r, g, b))
                }
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some(Self::Rgb(r, g, b))
                }
                _ => None,
            };
        }
        let n = s.parse::<u8>().ok()?;
        Some(if n < 16 {
            Self::Ansi(n)
        } else {
            Self::Ansi256(n)
        })
    }

    /// Returns the SGR parameter string selecting this color for the
    /// foreground or background layer (e.g. `"38;5;212"`).
    pub(crate) fn sgr_params(self, bg: bool) -> String {
        match self {
            Self::Ansi(n) if n < 8 => (u16::from(n) + if bg { 40 } else { 30 }).to_string(),
            Self::Ansi(n) if n < 16 => (u16::from(n - 8) + if bg { 100 } else { 90 }).to_string(),
            // Out-of-range Ansi values fall back to the indexed form.
            Self::Ansi(n) | Self::Ansi256(n) => {
                format!("{};5;{n}", if bg { 48 } else { 38 })
            }
            Self::Rgb(r, g, b) => format!("{};2;{r};{g};{b}", if bg { 48 } else { 38 }),
        }
    }
}

impl From<u8> for Color {
    /// Converts a palette index, mirroring [`Color::parse`] for numbers.
    fn from(n: u8) -> Self {
        if n < 16 {
            Self::Ansi(n)
        } else {
            Self::Ansi256(n)
        }
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from((r, g, b): (u8, u8, u8)) -> Self {
        Self::Rgb(r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex() {
        assert_eq!(Color::parse("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(Color::parse("#ff88cc"), Some(Color::Rgb(0xff, 0x88, 0xcc)));
        assert_eq!(Color::parse("#ffff"), None);
        assert_eq!(Color::parse("#gggggg"), None);
    }

    #[test]
    fn parse_index() {
        assert_eq!(Color::parse("1"), Some(Color::Ansi(1)));
        assert_eq!(Color::parse("212"), Some(Color::Ansi256(212)));
        assert_eq!(Color::parse("256"), None);
    }

    #[test]
    fn sgr_encoding() {
        assert_eq!(Color::RED.sgr_params(false), "31");
        assert_eq!(Color::BRIGHT_RED.sgr_params(false), "91");
        assert_eq!(Color::RED.sgr_params(true), "41");
        assert_eq!(Color::Ansi256(212).sgr_params(false), "38;5;212");
        assert_eq!(Color::Rgb(1, 2, 3).sgr_params(true), "48;2;1;2;3");
    }
}
