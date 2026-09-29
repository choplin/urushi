//! Synchronous full-screen presentation over terminal command primitives.
//!
//! [`Screen`] owns frame history and transactional presentation. It borrows a
//! draw-scoped [`Frame`] to callers, lowers changed cells through
//! [`urushi_terminal::CommandWriter`], and advances its committed baseline only
//! after output and flushing succeed. Physical terminal connection, input,
//! queries, raw mode, and session restoration remain responsibilities of
//! [`urushi_terminal::TerminalBackend`] and higher-level callers.

mod output;
mod screen;

use urushi_terminal::{Position, TerminalSize};

pub(crate) use output::Cell;
#[cfg(feature = "runtime")]
pub(crate) use screen::RenderFrame;
pub use screen::{Frame, Screen};

/// A rectangular region in terminal cell coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    origin: Position,
    size: TerminalSize,
}

impl Rect {
    /// Creates a rectangle from its origin and size.
    pub const fn new(origin: Position, size: TerminalSize) -> Self {
        Self { origin, size }
    }

    /// Creates a rectangle at the terminal origin.
    pub const fn from_size(size: TerminalSize) -> Self {
        Self::new(Position::new(0, 0), size)
    }

    /// Returns the rectangle's top-left cell.
    pub const fn origin(self) -> Position {
        self.origin
    }

    /// Returns the rectangle's cell dimensions.
    pub const fn size(self) -> TerminalSize {
        self.size
    }
}
