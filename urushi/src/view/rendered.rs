//! Rendered output: text this crate has already laid out, or text it did not.

use std::fmt;

use crate::visible_width;

use super::layout::Size;

/// A rectangle of rendered terminal output, and the size it was measured at.
///
/// Whether a string is plain text or already-rendered ANSI cannot be recovered
/// from the string, so the property is carried by this type instead of guessed.
/// Every row is exactly `size().width()` cells wide, so composing rendered
/// blocks never re-measures escape sequences.
///
/// A `RenderedBlock` does not re-enter the view tree: content that participates
/// in layout is expressed as a [`View`](crate::View).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedBlock {
    text: String,
    size: Size,
}

impl RenderedBlock {
    /// Adopts a string produced elsewhere.
    ///
    /// The caller asserts it is rendered output, and this is the one place
    /// ANSI-aware measurement happens. Rows shorter than the widest are padded
    /// with spaces, so the result is a rectangle.
    pub fn from_ansi(text: impl Into<String>) -> Self {
        let text = text.into();
        let rows: Vec<&str> = text.split('\n').collect();
        let width = rows.iter().copied().map(visible_width).max().unwrap_or(0);
        let height = rows.len();
        let padded = rows
            .iter()
            .map(|row| {
                let gap = width.saturating_sub(visible_width(row));
                format!("{row}{}", " ".repeat(gap))
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
