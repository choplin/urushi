//! The underline decoration: one value carrying both its style and its color.

use crate::Color;

/// The shape a terminal draws an underline with.
///
/// These are the complete SGR `4:1`–`4:5` vocabulary; the escape sequence has no
/// further underline shapes to expose.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum UnderlineStyle {
    #[default]
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

/// A set of underline shapes selected for rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UnderlineStyleSet(u8);

impl UnderlineStyleSet {
    pub const SINGLE: Self = Self(1 << 0);
    pub const DOUBLE: Self = Self(1 << 1);
    pub const CURLY: Self = Self(1 << 2);
    pub const DOTTED: Self = Self(1 << 3);
    pub const DASHED: Self = Self(1 << 4);

    const ALL_BITS: u8 =
        Self::SINGLE.0 | Self::DOUBLE.0 | Self::CURLY.0 | Self::DOTTED.0 | Self::DASHED.0;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, style: UnderlineStyle) -> bool {
        let style = match style {
            UnderlineStyle::Single => Self::SINGLE,
            UnderlineStyle::Double => Self::DOUBLE,
            UnderlineStyle::Curly => Self::CURLY,
            UnderlineStyle::Dotted => Self::DOTTED,
            UnderlineStyle::Dashed => Self::DASHED,
        };
        self.0 & style.0 != 0
    }
}

impl std::ops::BitOr for UnderlineStyleSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl UnderlineStyle {
    /// The SGR parameter selecting this shape (e.g. `"4:3"`).
    ///
    /// A single underline is spelled `4` rather than the equivalent `4:1`,
    /// because a terminal that does not parse SGR subparameters still
    /// understands it. The two mean the same thing, so only one is ever
    /// emitted and the output stays canonical.
    pub(crate) const fn sgr_params(self) -> &'static str {
        match self {
            Self::Single => "4",
            Self::Double => "4:2",
            Self::Curly => "4:3",
            Self::Dotted => "4:4",
            Self::Dashed => "4:5",
        }
    }
}

/// An underline: the shape it is drawn with, and the color it is drawn in.
///
/// A `color` of `None` means the terminal draws the underline in the text's
/// foreground color, which is its default behavior.
///
/// This is one value rather than a modifier flag beside a separate color
/// property, so that one appearance cannot be spelled two ways; the reasoning
/// is in `docs/design/underline.md`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Underline {
    pub style: UnderlineStyle,
    pub color: Option<Color>,
}

impl Underline {
    /// An underline of `style`, drawn in the foreground color.
    pub const fn new(style: UnderlineStyle) -> Self {
        Self { style, color: None }
    }

    /// Returns this underline drawn in `color`.
    pub fn with_color(mut self, color: impl Into<Color>) -> Self {
        self.color = Some(color.into());
        self
    }
}

impl From<UnderlineStyle> for Underline {
    fn from(style: UnderlineStyle) -> Self {
        Self::new(style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_a_single_underline_in_the_foreground_color() {
        assert_eq!(
            Underline::default(),
            Underline {
                style: UnderlineStyle::Single,
                color: None,
            }
        );
        assert_eq!(
            Underline::new(UnderlineStyle::Curly).with_color(Color::RED),
            Underline {
                style: UnderlineStyle::Curly,
                color: Some(Color::RED),
            }
        );
    }

    #[test]
    fn every_shape_has_its_own_sgr_parameter() {
        let params = [
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
        ]
        .map(UnderlineStyle::sgr_params);

        assert_eq!(params, ["4", "4:2", "4:3", "4:4", "4:5"]);
    }
}
