//! State the inline renderer carries between redraws.
//!
//! The prompt owns a region of rows below the terminal cursor position it
//! saved on first draw. [`InlinePresentation`] is everything the renderer must
//! remember to keep claiming, redrawing, and finally releasing that region.

use super::frame::FramedRow;

#[derive(Default)]
pub(crate) struct InlinePresentation {
    /// Whether the origin of the owned region has been saved in the terminal.
    pub origin_saved: bool,
    /// Rows scrolled into existence below the origin so far.
    pub reserved_rows: u16,
    /// Rows the prompt currently claims; error cleanup erases exactly these.
    pub previous_rows: u16,
    /// The rows as last drawn, used to skip unchanged rows on redraw.
    pub previous_lines: Vec<FramedRow>,
}
