//! Urushi-owned cell storage and shared traversal of resolved cell rectangles.

use std::fmt;

use compact_str::CompactString;
use urushi::{ResolvedView, StyledGrapheme, TerminalTextStyle};
use urushi_terminal::{Position, TerminalSize, TerminalStyle};

use crate::terminal::Cell as OutputCell;

#[derive(Clone, Debug, PartialEq, Eq)]
enum CellKind {
    Empty,
    Start(StoredCell),
    Continuation { owner: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredCell {
    symbol: CompactString,
    width: usize,
    style: TerminalStyle,
}

impl StoredCell {
    fn is_default_blank(&self) -> bool {
        self.symbol == " " && self.width == 1 && self.style == TerminalStyle::default()
    }
}

/// A fixed-size row-major cell buffer.
///
/// Every non-empty cell is either the start of one complete grapheme or a
/// continuation that points at its start. The representation keeps terminal
/// style and text independent of any backend buffer type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Buffer {
    size: TerminalSize,
    cells: Vec<CellKind>,
}

impl Buffer {
    pub(crate) fn new(size: TerminalSize) -> Result<Self, BufferSizeError> {
        let cells = empty_cells(size)?;
        Ok(Self { size, cells })
    }

    pub(crate) const fn size(&self) -> TerminalSize {
        self.size
    }

    /// Writes one resolved grapheme and replaces every previous owner it overlaps.
    pub(crate) fn write(
        &mut self,
        column: usize,
        row: usize,
        grapheme: &StyledGrapheme,
    ) -> Result<(), CellWriteError> {
        self.write_parts(
            column,
            row,
            grapheme.symbol(),
            grapheme.width(),
            TerminalTextStyle::from(grapheme.style()).style(),
        )
    }

    fn write_parts(
        &mut self,
        column: usize,
        row: usize,
        symbol: &str,
        width: usize,
        style: TerminalStyle,
    ) -> Result<(), CellWriteError> {
        if width == 0 {
            return Err(CellWriteError::ZeroWidth);
        }
        let Some(end) = column.checked_add(width) else {
            return Err(CellWriteError::OutOfBounds {
                position: Position::new(column, row),
                width,
                size: self.size,
            });
        };
        if row >= self.size.rows() || end > self.size.columns() {
            return Err(CellWriteError::OutOfBounds {
                position: Position::new(column, row),
                width,
                size: self.size,
            });
        }

        let start = row * self.size.columns() + column;
        for index in start..start + width {
            if let Some(owner) = self.owner_at(index) {
                self.clear_owner(owner);
            }
        }

        self.cells[start] = CellKind::Start(StoredCell {
            symbol: symbol.into(),
            width,
            style,
        });
        for index in start + 1..start + width {
            self.cells[index] = CellKind::Continuation { owner: start };
        }
        Ok(())
    }

    pub(crate) fn reset(&mut self) {
        self.cells.fill(CellKind::Empty);
    }

    /// Replaces the storage with an empty buffer of `size`.
    pub(crate) fn resize(&mut self, size: TerminalSize) -> Result<(), BufferSizeError> {
        let cells = empty_cells(size)?;
        self.cells = cells;
        self.size = size;
        Ok(())
    }

    /// Lazily compares this committed buffer with a working buffer.
    pub(crate) fn diff<'a>(
        &'a self,
        working: &'a Self,
    ) -> Result<BufferDiff<'a>, BufferBoundsMismatch> {
        if self.size != working.size {
            return Err(BufferBoundsMismatch {
                committed: self.size,
                working: working.size,
            });
        }
        Ok(BufferDiff {
            committed: self,
            working,
            next: 0,
        })
    }

    fn owner_at(&self, index: usize) -> Option<usize> {
        match self.cells[index] {
            CellKind::Empty => None,
            CellKind::Start(_) => Some(index),
            CellKind::Continuation { owner } => Some(owner),
        }
    }

    fn clear_owner(&mut self, owner: usize) {
        let CellKind::Start(cell) = &self.cells[owner] else {
            debug_assert!(false, "a continuation must point at a start cell");
            return;
        };
        let width = cell.width;
        for index in owner..owner + width {
            self.cells[index] = CellKind::Empty;
        }
    }
}

fn buffer_len(size: TerminalSize) -> Result<usize, BufferSizeError> {
    size.columns()
        .checked_mul(size.rows())
        .ok_or(BufferSizeError { size })
}

fn empty_cells(size: TerminalSize) -> Result<Vec<CellKind>, BufferSizeError> {
    let len = buffer_len(size)?;
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(len)
        .map_err(|_| BufferSizeError { size })?;
    cells.resize(len, CellKind::Empty);
    Ok(cells)
}

/// The requested dimensions cannot be represented by one row-major buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BufferSizeError {
    size: TerminalSize,
}

impl fmt::Display for BufferSizeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot allocate buffer dimensions {}x{}",
            self.size.columns(),
            self.size.rows()
        )
    }
}

impl std::error::Error for BufferSizeError {}

/// A grapheme cannot be represented at the requested buffer position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CellWriteError {
    ZeroWidth,
    OutOfBounds {
        position: Position,
        width: usize,
        size: TerminalSize,
    },
}

impl fmt::Display for CellWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWidth => formatter.write_str("a zero-width grapheme owns no buffer cell"),
            Self::OutOfBounds {
                position,
                width,
                size,
            } => write!(
                formatter,
                "grapheme of width {width} at ({}, {}) exceeds buffer {}x{}",
                position.column(),
                position.row(),
                size.columns(),
                size.rows()
            ),
        }
    }
}

impl std::error::Error for CellWriteError {}

/// Incremental diffing requires buffers with identical bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BufferBoundsMismatch {
    committed: TerminalSize,
    working: TerminalSize,
}

impl fmt::Display for BufferBoundsMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot diff buffers with bounds {}x{} and {}x{}",
            self.committed.columns(),
            self.committed.rows(),
            self.working.columns(),
            self.working.rows()
        )
    }
}

impl std::error::Error for BufferBoundsMismatch {}

/// A lazy row-major sequence of cells that must be sent to the terminal.
pub(crate) struct BufferDiff<'a> {
    committed: &'a Buffer,
    working: &'a Buffer,
    next: usize,
}

impl<'a> Iterator for BufferDiff<'a> {
    type Item = (Position, OutputCell<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        while self.next < self.working.cells.len() {
            let index = self.next;
            self.next += 1;
            let changed = match &self.working.cells[index] {
                CellKind::Continuation { .. } => None,
                CellKind::Empty => {
                    (!visibly_blank(&self.committed.cells[index])).then_some(OutputCell {
                        symbol: " ",
                        style: TerminalStyle::default(),
                    })
                }
                CellKind::Start(working) => (!visibly_equal(&self.committed.cells[index], working))
                    .then_some(OutputCell {
                        symbol: working.symbol.as_str(),
                        style: working.style,
                    }),
            };
            if let Some(cell) = changed {
                let columns = self.working.size.columns();
                return Some((Position::new(index % columns, index / columns), cell));
            }
        }
        None
    }
}

fn visibly_blank(cell: &CellKind) -> bool {
    match cell {
        CellKind::Empty => true,
        CellKind::Start(cell) => cell.is_default_blank(),
        CellKind::Continuation { .. } => false,
    }
}

fn visibly_equal(committed: &CellKind, working: &StoredCell) -> bool {
    match committed {
        CellKind::Empty => working.is_default_blank(),
        CellKind::Start(committed) => committed == working,
        CellKind::Continuation { .. } => false,
    }
}

/// Visits every leading grapheme cell in row-major order.
///
/// Backend-specific coordinates and clipping stay with the caller.
pub(crate) fn visit_resolved(
    resolved: &ResolvedView,
    mut visit: impl FnMut(usize, usize, &StyledGrapheme),
) {
    for (row, graphemes) in resolved.rows().iter().enumerate() {
        let mut column = 0;
        for grapheme in graphemes {
            visit(column, row, grapheme);
            column = column.saturating_add(grapheme.width());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use urushi::{Available, Color, TextAttribute, TextStyle, View, resolve};

    fn buffer(columns: usize, rows: usize) -> Buffer {
        Buffer::new(TerminalSize::new(columns, rows)).expect("test buffer size is valid")
    }

    fn write(buffer: &mut Buffer, column: usize, row: usize, symbol: &str, width: usize) {
        buffer
            .write_parts(column, row, symbol, width, TerminalStyle::default())
            .expect("test cell fits")
    }

    fn changes(committed: &Buffer, working: &Buffer) -> Vec<(Position, String, TerminalStyle)> {
        committed
            .diff(working)
            .expect("test buffers have equal bounds")
            .map(|(position, cell)| (position, cell.symbol.to_owned(), cell.style))
            .collect()
    }

    #[test]
    fn write_owns_every_cell_of_a_wide_grapheme() {
        let mut target = buffer(4, 1);

        write(&mut target, 1, 0, "👩‍💻", 2);

        assert_eq!(
            target.cells,
            [
                CellKind::Empty,
                CellKind::Start(StoredCell {
                    symbol: "👩‍💻".into(),
                    width: 2,
                    style: TerminalStyle::default(),
                }),
                CellKind::Continuation { owner: 1 },
                CellKind::Empty,
            ]
        );
    }

    #[test]
    fn resolved_grapheme_is_stored_without_a_backend_cell_conversion() {
        let style = TextStyle::new()
            .foreground(Color::BRIGHT_RED)
            .add_attributes(TextAttribute::Bold | TextAttribute::Italic);
        let view = View::text("👩‍💻", style);
        let resolved = resolve(&view, Available::size(2, 1)).expect("view resolves");
        let grapheme = &resolved.rows()[0][0];
        let mut target = buffer(2, 1);

        target
            .write(0, 0, grapheme)
            .expect("resolved grapheme fits");

        assert!(matches!(
            &target.cells[0],
            CellKind::Start(StoredCell {
                symbol,
                width: 2,
                style: TerminalStyle {
                    foreground: Some(Color::BRIGHT_RED),
                    attributes,
                    ..
                },
            }) if symbol == "👩‍💻"
                && attributes.contains(TextAttribute::Bold)
                && attributes.contains(TextAttribute::Italic)
        ));
        assert_eq!(target.cells[1], CellKind::Continuation { owner: 0 });
    }

    #[test]
    fn checked_write_rejects_zero_width_and_out_of_bounds_without_mutating() {
        let original = buffer(2, 1);
        let mut target = original.clone();

        assert_eq!(
            target.write_parts(0, 0, "\u{301}", 0, TerminalStyle::default()),
            Err(CellWriteError::ZeroWidth)
        );
        assert!(matches!(
            target.write_parts(1, 0, "界", 2, TerminalStyle::default()),
            Err(CellWriteError::OutOfBounds { .. })
        ));
        assert_eq!(target, original);
    }

    #[test]
    fn overlapping_write_clears_the_complete_previous_owner() {
        let mut target = buffer(3, 1);
        write(&mut target, 0, 0, "界", 2);

        write(&mut target, 1, 0, "x", 1);

        assert_eq!(target.cells[0], CellKind::Empty);
        assert!(matches!(&target.cells[1], CellKind::Start(cell) if cell.symbol == "x"));
    }

    #[test]
    fn overlap_clears_a_previous_owner_past_the_new_grapheme() {
        let mut target = buffer(3, 1);
        write(&mut target, 1, 0, "界", 2);

        write(&mut target, 0, 0, "好", 2);

        assert!(matches!(&target.cells[0], CellKind::Start(cell) if cell.symbol == "好"));
        assert_eq!(target.cells[1], CellKind::Continuation { owner: 0 });
        assert_eq!(target.cells[2], CellKind::Empty);
    }

    #[test]
    fn reset_and_resize_replace_all_cell_ownership() {
        let mut target = buffer(2, 1);
        write(&mut target, 0, 0, "界", 2);

        target.reset();
        assert_eq!(target.cells, [CellKind::Empty, CellKind::Empty]);

        target
            .resize(TerminalSize::new(1, 2))
            .expect("test resize fits");
        assert_eq!(target.size(), TerminalSize::new(1, 2));
        assert_eq!(target.cells, [CellKind::Empty, CellKind::Empty]);
    }

    #[test]
    fn default_blank_and_empty_are_visibly_equal_in_both_directions() {
        let empty = buffer(1, 1);
        let mut blank = buffer(1, 1);
        write(&mut blank, 0, 0, " ", 1);

        assert!(changes(&empty, &blank).is_empty());
        assert!(changes(&blank, &empty).is_empty());
    }

    #[test]
    fn styled_blank_is_not_visibly_equal_to_empty() {
        let empty = buffer(1, 1);
        let mut styled = buffer(1, 1);
        let style = TerminalStyle {
            background: Some(Color::BLUE),
            attributes: TextAttribute::Bold.into(),
            ..TerminalStyle::default()
        };
        styled
            .write_parts(0, 0, " ", 1, style)
            .expect("styled blank fits");

        assert_eq!(
            changes(&empty, &styled),
            [(Position::new(0, 0), " ".to_owned(), style)]
        );
        assert_eq!(
            changes(&styled, &empty),
            [(
                Position::new(0, 0),
                " ".to_owned(),
                TerminalStyle::default()
            )]
        );
    }

    #[test]
    fn shrinking_a_wide_grapheme_clears_its_old_continuation() {
        let mut committed = buffer(2, 1);
        write(&mut committed, 0, 0, "界", 2);
        let mut working = buffer(2, 1);
        write(&mut working, 0, 0, "x", 1);

        assert_eq!(
            changes(&committed, &working),
            [
                (
                    Position::new(0, 0),
                    "x".to_owned(),
                    TerminalStyle::default()
                ),
                (
                    Position::new(1, 0),
                    " ".to_owned(),
                    TerminalStyle::default()
                ),
            ]
        );
    }

    #[test]
    fn deleting_a_wide_grapheme_clears_every_cell_it_owned() {
        let mut committed = buffer(2, 1);
        write(&mut committed, 0, 0, "界", 2);
        let working = buffer(2, 1);

        assert_eq!(
            changes(&committed, &working)
                .into_iter()
                .map(|(position, _, _)| position)
                .collect::<Vec<_>>(),
            [Position::new(0, 0), Position::new(1, 0)]
        );
    }

    #[test]
    fn moving_a_wide_grapheme_emits_its_owner_but_not_its_continuation() {
        let mut committed = buffer(3, 1);
        write(&mut committed, 0, 0, "界", 2);
        let mut working = buffer(3, 1);
        write(&mut working, 1, 0, "界", 2);

        assert_eq!(
            changes(&committed, &working)
                .into_iter()
                .map(|(position, symbol, _)| (position, symbol))
                .collect::<Vec<_>>(),
            [
                (Position::new(0, 0), " ".to_owned()),
                (Position::new(1, 0), "界".to_owned()),
            ]
        );
    }

    #[test]
    fn overlapping_wide_graphemes_clear_only_uncovered_old_cells() {
        let mut committed = buffer(3, 1);
        write(&mut committed, 1, 0, "界", 2);
        let mut working = buffer(3, 1);
        write(&mut working, 0, 0, "好", 2);

        assert_eq!(
            changes(&committed, &working)
                .into_iter()
                .map(|(position, symbol, _)| (position, symbol))
                .collect::<Vec<_>>(),
            [
                (Position::new(0, 0), "好".to_owned()),
                (Position::new(2, 0), " ".to_owned()),
            ]
        );
    }

    #[test]
    fn diff_rejects_mismatched_bounds() {
        let committed = buffer(2, 1);
        let working = buffer(1, 2);

        assert!(matches!(
            committed.diff(&working),
            Err(BufferBoundsMismatch { .. })
        ));
    }

    #[test]
    fn size_overflow_is_reported_without_allocating() {
        assert_eq!(
            Buffer::new(TerminalSize::new(usize::MAX, 2)),
            Err(BufferSizeError {
                size: TerminalSize::new(usize::MAX, 2)
            })
        );
        assert_eq!(
            Buffer::new(TerminalSize::new(usize::MAX, 1)),
            Err(BufferSizeError {
                size: TerminalSize::new(usize::MAX, 1)
            })
        );
    }
}
