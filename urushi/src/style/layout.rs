//! Alignment, sizing, and box-side values used by logical styles.

use std::borrow::Cow;

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
    Fill(u16),
}

impl From<u16> for Length {
    fn from(value: u16) -> Self {
        Self::Cells(value)
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
