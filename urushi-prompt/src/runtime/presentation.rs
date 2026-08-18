//! State the inline renderer carries between redraws.
//!
//! The prompt owns a region of rows below the terminal cursor position it
//! saved on first draw. [`InlinePresentation`] is everything the renderer must
//! remember to keep claiming, redrawing, and finally releasing that region.

use super::frame::FramedRow;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct InlinePresentation {
    /// Whether a valid origin for the owned region is saved in the terminal.
    pub anchored: bool,
    /// Rows materialized below the origin. Never decreases while a prompt runs.
    pub reserved_rows: u16,
    /// Rows cleanup must erase. After a completed frame it is that frame's
    /// height; after a failed one it is the high-water mark of the rows the
    /// commands that succeeded actually touched.
    pub owned_rows: u16,
    /// The rows as last drawn, used to skip unchanged rows on redraw.
    pub rows: Vec<FramedRow>,
    /// Whether this prompt has put anything on screen. Set by the first
    /// successful write and never cleared: every other field describes the
    /// region currently tracked, while this one describes the screen.
    /// Region loss, which resets everything else and leaves this alone, is the
    /// decision it exists to gate; see `docs/design/prompt-region.md`.
    pub drawn: bool,
}
