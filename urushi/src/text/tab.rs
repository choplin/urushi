use std::fmt;

use super::PrintableText;

pub(crate) static DEFAULT_TAB_POLICY: TabPolicy = TabPolicy::spaces(4);

/// A fixed-width replacement for one tab during text layout.
///
/// A policy without a marker produces `width` spaces. A policy with a marker
/// writes that printable text first and pads its right side with spaces until
/// the replacement occupies `width` cells. Literal tabs are preserved only by
/// the non-layout text renderer; resolved views never contain them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabPolicy {
    width: u16,
    marker: Option<String>,
}

impl Default for TabPolicy {
    fn default() -> Self {
        Self::spaces(4)
    }
}

impl TabPolicy {
    /// Replaces each tab with `width` spaces.
    pub const fn spaces(width: u16) -> Self {
        Self {
            width,
            marker: None,
        }
    }

    /// Replaces each tab with `marker`, padded on the right to `width` cells.
    ///
    /// An empty marker is canonicalized to no marker. Other markers must be
    /// printable text on one line, have no zero-width graphemes, and fit
    /// inside the requested width.
    pub fn with_marker(width: u16, marker: impl Into<String>) -> Result<Self, InvalidTabMarker> {
        let marker = marker.into();
        if marker.is_empty() {
            return Ok(Self::spaces(width));
        }
        if marker.chars().any(char::is_control) {
            return Err(InvalidTabMarker::ControlCharacter);
        }
        let printable = PrintableText::new(&marker);
        if printable.graphemes().any(|grapheme| grapheme.width() == 0) {
            return Err(InvalidTabMarker::ZeroWidthGrapheme);
        }
        let marker_width = printable.width();
        if marker_width > usize::from(width) {
            return Err(InvalidTabMarker::TooWide {
                width,
                marker_width,
            });
        }
        Ok(Self {
            width,
            marker: Some(marker),
        })
    }

    /// Returns the fixed cell width of one replacement.
    pub const fn width(&self) -> u16 {
        self.width
    }

    /// Returns the optional visible marker written before the padding.
    pub fn marker(&self) -> Option<&str> {
        self.marker.as_deref()
    }
}

/// Why a visible tab marker cannot participate in layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidTabMarker {
    /// The marker contains a terminal control character.
    ControlCharacter,
    /// The marker contains a grapheme with no independently visible cells.
    ZeroWidthGrapheme,
    /// The marker occupies more cells than the configured replacement width.
    TooWide { width: u16, marker_width: usize },
}

impl fmt::Display for InvalidTabMarker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ControlCharacter => {
                formatter.write_str("a tab marker must not contain control characters")
            }
            Self::ZeroWidthGrapheme => {
                formatter.write_str("every tab marker grapheme must occupy at least one cell")
            }
            Self::TooWide {
                width,
                marker_width,
            } => write!(
                formatter,
                "tab marker occupies {marker_width} cells, exceeding its width of {width}"
            ),
        }
    }
}

impl std::error::Error for InvalidTabMarker {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_marker_is_canonicalized_to_spaces() {
        assert_eq!(TabPolicy::with_marker(3, "").unwrap(), TabPolicy::spaces(3));
    }

    #[test]
    fn marker_must_be_printable_and_fit_its_width() {
        assert_eq!(
            TabPolicy::with_marker(4, "→\t"),
            Err(InvalidTabMarker::ControlCharacter)
        );
        assert_eq!(
            TabPolicy::with_marker(1, "日本"),
            Err(InvalidTabMarker::TooWide {
                width: 1,
                marker_width: 4,
            })
        );
        assert_eq!(
            TabPolicy::with_marker(1, "\u{301}"),
            Err(InvalidTabMarker::ZeroWidthGrapheme)
        );
        assert_eq!(
            TabPolicy::with_marker(1, "\u{301}x"),
            Err(InvalidTabMarker::ZeroWidthGrapheme)
        );
    }
}
