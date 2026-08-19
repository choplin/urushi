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

impl InlinePresentation {
    /// Abandon the region whose extent can no longer be established.
    ///
    /// A resize may reflow content and move the saved origin, and a write that
    /// fails while the origin is being re-anchored leaves the region mid-growth
    /// with nothing to restore to. Both leave the extent unknown, and the
    /// design abandons such a region rather than erasing rows whose position
    /// was inferred: residue is ugly and bounded, erasure is invisible and
    /// unbounded.
    ///
    /// `drawn` survives. It records that this prompt put something on screen at
    /// some point, which stays true however many regions have been abandoned
    /// since; see `docs/design/prompt-region.md`.
    pub(crate) fn lose_region(&mut self) {
        self.anchored = false;
        self.reserved_rows = 0;
        self.owned_rows = 0;
        self.rows.clear();
    }
}
