//! Rendered output: text this crate has already laid out, or text it did not.

use std::fmt;

use super::ansi;
use super::geometry::Size;

/// A rectangle of rendered terminal output, and the size it was measured at.
///
/// Whether a string is plain text or already-rendered ANSI cannot be recovered
/// from the string, so the property is carried by this type instead of guessed.
/// Every row is exactly `size().width()` cells wide, so composing rendered
/// blocks never re-measures escape sequences.
///
/// The text it holds may carry ANSI SGR and OSC sequences. It never carries
/// cursor movement: [`from_ansi`](Self::from_ansi) resolves `\r`, `\t` and
/// backspace to the cells they produce, and the crate's own rendering emits
/// none. That invariant is what makes the block a rectangle of cells rather
/// than a script for a terminal, and it is why a block can be placed at any
/// column of a join without its content sliding.
///
/// A `RenderedBlock` does not re-enter the view tree: content that participates
/// in layout is expressed as a [`View`](crate::View).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedBlock {
    text: String,
    size: Size,
}

impl RenderedBlock {
    /// Adopts a string produced elsewhere, asserted by the caller to be
    /// rendered output — text that may carry ANSI escape sequences and cursor
    /// movement.
    ///
    /// This is the one place in the crate where ANSI-aware measurement
    /// happens. Everywhere else measures plain text, because whether a string
    /// is one or the other cannot be recovered from the string, so the
    /// property is declared here instead of guessed on every call.
    ///
    /// Each row is resolved to the cells a terminal would show before it is
    /// measured: `\r`, `\t` and backspace move the cursor, so text written
    /// later can land on top of text written earlier, and counting the
    /// operations would not give the width of the result. Rows shorter than
    /// the widest are then padded with spaces. The block that comes out is a
    /// rectangle of cells containing no cursor movement, which is what lets
    /// [`join_horizontal`](crate::join_horizontal) and
    /// [`join_vertical`](crate::join_vertical) compose blocks without
    /// measuring them again.
    ///
    /// Empty text is an empty block: zero cells wide and zero rows tall.
    pub fn from_ansi(text: impl Into<String>) -> Self {
        let text = text.into();
        let resolved: Vec<(std::borrow::Cow<'_, str>, usize)> = ansi::rows(&text)
            .into_iter()
            .map(ansi::resolve_row)
            .collect();
        let width = resolved.iter().map(|(_, width)| *width).max().unwrap_or(0);
        let height = resolved.len();
        let padded = resolved
            .iter()
            .map(|(row, row_width)| {
                format!("{row}{}", " ".repeat(width.saturating_sub(*row_width)))
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self {
            text: padded,
            size: Size::new(width, height),
        }
    }

    /// Adopts text this crate laid out, with the size the layout pass decided.
    pub(crate) fn measured(text: String, size: Size) -> Self {
        Self { text, size }
    }

    /// Returns the size this block was measured at.
    pub const fn size(&self) -> Size {
        self.size
    }

    /// Returns the rendered text, without a trailing newline.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Consumes the block, returning its rendered text.
    pub fn into_string(self) -> String {
        self.text
    }

    pub(super) fn rows(&self) -> Vec<&str> {
        if self.size.height() == 0 {
            return Vec::new();
        }
        self.text.split('\n').collect()
    }

    pub(super) fn empty() -> Self {
        Self {
            text: String::new(),
            size: Size::ZERO,
        }
    }
}

impl fmt::Display for RenderedBlock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.text)
    }
}

impl AsRef<str> for RenderedBlock {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(text: &str) -> (usize, usize) {
        let block = RenderedBlock::from_ansi(text);
        (block.size().width(), block.size().height())
    }

    #[test]
    fn measures_rows_and_columns() {
        assert_eq!(size(""), (0, 0));
        assert_eq!(size("abc"), (3, 1));
        assert_eq!(size("abc\nde"), (3, 2));
        assert_eq!(
            size("abc\n"),
            (3, 2),
            "a trailing newline leaves an empty row"
        );
        assert_eq!(
            size("a\r\nb"),
            (1, 2),
            "the carriage return of a CRLF is not a cell"
        );
    }

    #[test]
    fn measures_cells_not_characters() {
        assert_eq!(size("日本語"), (6, 1));
        assert_eq!(size("👩‍💻x"), (3, 1));
        assert_eq!(size("e\u{301}x"), (2, 1));
    }

    #[test]
    fn escape_sequences_occupy_no_cells() {
        assert_eq!(size("\x1b[1;38;5;212mabc\x1b[0m"), (3, 1));
        assert_eq!(size("\x1b[31ma\x1b[1mb\x1b[0mc\x1b[0m"), (3, 1));
        assert_eq!(
            size("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"),
            (4, 1)
        );
    }

    #[test]
    fn cursor_movement_is_resolved_before_the_row_is_measured() {
        assert_eq!(size("\rProgress"), (8, 1));
        assert_eq!(size("ab\rc"), (2, 1));
        assert_eq!(size("a\tb"), (9, 1));
        assert_eq!(size("abc\u{8}\u{8}X"), (3, 1));
        assert_eq!(size("日\rx"), (1, 1), "half a wide grapheme cannot survive");
    }

    #[test]
    fn every_row_is_padded_to_the_block_width() {
        let block = RenderedBlock::from_ansi("abc\nd");
        assert_eq!(block.as_str(), "abc\nd  ");
        for row in block.rows() {
            assert_eq!(RenderedBlock::from_ansi(row).size().width(), 3);
        }
    }

    #[test]
    fn resolution_keeps_the_last_style_written_to_a_cell() {
        let block = RenderedBlock::from_ansi("\x1b[31mab\rc\x1b[0m");
        assert_eq!(block.as_str(), "\x1b[31mcb\x1b[0m");
        assert_eq!(block.size().width(), 2);
    }
}
