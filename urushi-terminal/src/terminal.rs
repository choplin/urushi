//! Terminal-owned presentation state and its draw-scoped access.

use std::io;

use crate::TerminalSize;

/// One cell position relative to a terminal surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Position {
    column: usize,
    row: usize,
}

impl Position {
    pub const fn new(column: usize, row: usize) -> Self {
        Self { column, row }
    }

    pub const fn column(self) -> usize {
        self.column
    }

    pub const fn row(self) -> usize {
        self.row
    }
}

/// One rectangular region of a terminal surface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    origin: Position,
    size: TerminalSize,
}

impl Rect {
    pub const fn new(origin: Position, size: TerminalSize) -> Self {
        Self { origin, size }
    }

    pub const fn from_size(size: TerminalSize) -> Self {
        Self::new(Position::new(0, 0), size)
    }

    pub const fn origin(self) -> Position {
        self.origin
    }

    pub const fn size(self) -> TerminalSize {
        self.size
    }
}

/// A draw-scoped handle to one terminal's working presentation state.
pub trait Frame {
    /// The cell value this frame accepts.
    type Cell: ?Sized;

    fn area(&self) -> Rect;

    fn put(&mut self, column: usize, row: usize, cell: &Self::Cell);

    fn set_cursor(&mut self, at: Option<Position>);
}

/// Presentation state and commit behavior seen by a terminal owner.
pub trait Terminal {
    /// The cell value accepted by frames drawn through this terminal.
    type Cell: ?Sized;

    type Frame<'a>: Frame<Cell = Self::Cell>
    where
        Self: 'a;

    fn size(&self) -> TerminalSize;

    fn resize(&mut self, size: TerminalSize) -> io::Result<()>;

    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()>;
}
