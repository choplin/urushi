//! Alignment, sizing, and box-side values used by logical styles.

use std::borrow::Cow;
use std::fmt;
use std::num::NonZeroU16;

/// A box dimension: an absolute size, or a share of the remaining area.
///
/// Every length measures the box the terminal shows — content plus padding
/// plus enabled border edges — with margin outside it. `u16` converts into
/// [`Length::Cells`], so `width(20)` stays concise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Length {
    /// An absolute number of terminal cells.
    Cells(u16),
    /// A weighted share of the area remaining to the box's siblings.
    Fill(NonZeroU16),
}

impl Length {
    /// Creates a weighted share of the area remaining to the box's siblings.
    ///
    /// Use [`Length::try_fill`] when `weight` comes from input that may be
    /// zero.
    ///
    /// # Panics
    ///
    /// Panics if `weight` is zero.
    pub const fn fill(weight: u16) -> Self {
        match Self::try_fill(weight) {
            Ok(length) => length,
            Err(_) => panic!("fill weight must be greater than zero"),
        }
    }

    /// Tries to create a weighted share of the remaining area.
    pub const fn try_fill(weight: u16) -> Result<Self, InvalidFillWeight> {
        match NonZeroU16::new(weight) {
            Some(weight) => Ok(Self::Fill(weight)),
            None => Err(InvalidFillWeight),
        }
    }
}

/// A fill weight was zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidFillWeight;

impl fmt::Display for InvalidFillWeight {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fill weight must be greater than zero")
    }
}

impl std::error::Error for InvalidFillWeight {}

impl From<u16> for Length {
    fn from(value: u16) -> Self {
        Self::Cells(value)
    }
}

#[cfg(test)]
mod length_tests {
    use super::{InvalidFillWeight, Length};

    #[test]
    fn fill_and_try_fill_construct_the_same_positive_weight() {
        assert_eq!(Length::fill(1), Length::try_fill(1).unwrap());
        assert_eq!(Length::fill(2), Length::try_fill(2).unwrap());
    }

    #[test]
    fn try_fill_rejects_zero() {
        assert_eq!(Length::try_fill(0), Err(InvalidFillWeight));
    }

    #[test]
    #[should_panic(expected = "fill weight must be greater than zero")]
    fn fill_panics_on_zero() {
        Length::fill(0);
    }
}

/// How content that does not fit its box is absorbed.
///
/// The frame always closes at the resolved size; overflow is absorbed by the
/// content. This governs the width axis. Height always clips inside the frame.
///
/// Clipping carries the marker that stands for what was cut, because which
/// glyph a terminal can show is the application's knowledge, not the
/// library's: `…` on a capable terminal, `...` where [`Border::ASCII`] would
/// be chosen for the same reason, and `""` for a silent cut.
///
/// ```
/// use urushi::Overflow;
///
/// let quiet = Overflow::clip();
/// let marked = Overflow::ellipsis();
/// let ascii = Overflow::clip_with("...");
/// ```
///
/// [`Border::ASCII`]: crate::Border::ASCII
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Overflow {
    /// Reflow the content to the content width.
    #[default]
    Wrap,
    /// Cut inside the frame, ending the cut line with this marker. The frame
    /// stays closed; an empty marker cuts silently.
    Clip(Cow<'static, str>),
}

impl Overflow {
    /// Cuts without marking the cut.
    pub const fn clip() -> Self {
        Self::Clip(Cow::Borrowed(""))
    }

    /// Cuts, ending the line with a horizontal ellipsis.
    pub const fn ellipsis() -> Self {
        Self::Clip(Cow::Borrowed("…"))
    }

    /// Cuts, ending the line with `marker`.
    ///
    /// The marker occupies cells of its own: the content keeps the box's width
    /// less the marker's display width. A marker that cannot fit the box is
    /// dropped, leaving a silent cut rather than an open frame.
    pub fn clip_with(marker: impl Into<Cow<'static, str>>) -> Self {
        Self::Clip(marker.into())
    }

    /// Returns the marker this policy ends a cut line with, if it cuts at all.
    pub fn clip_marker(&self) -> Option<&str> {
        match self {
            Self::Wrap => None,
            Self::Clip(marker) => Some(marker),
        }
    }
}

/// Horizontal alignment of content within a styled block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Vertical alignment of content within a styled block or of blocks joined
/// side by side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VerticalAlign {
    #[default]
    Top,
    Center,
    Bottom,
}

/// Spacing values for the four sides of a box.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sides {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl From<u16> for Sides {
    fn from(value: u16) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }
}

impl From<(u16, u16)> for Sides {
    fn from((vertical, horizontal): (u16, u16)) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

impl From<(u16, u16, u16, u16)> for Sides {
    fn from((top, right, bottom, left): (u16, u16, u16, u16)) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}
