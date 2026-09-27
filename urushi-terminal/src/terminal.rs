//! Geometry shared by terminal commands, events, and queries.

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
