//! What a grid is, apart from how large it is: its rectangular shape and the
//! padding and placement each cell takes.
//!
//! Nothing here decides a width or a height. The phases that do —
//! [`width`](super::width), [`height`](super::height), and
//! [`assemble`](super::assemble) — read these answers the way they read a
//! [`BlockStyle`](crate::BlockStyle): as what the style already fixed. The
//! rules are `docs/design/grid.md`'s.

use crate::{Align, BlockStyle, GridStyle, Sides, VerticalAlign, View};

/// The cell a ragged grid does not hold.
///
/// A grid is a rectangle, and building one that is not violates the contract
/// [`View::Grid`] states. Debug builds panic on it; this is what release
/// builds resolve in a missing cell's place, chosen so the shape stays a
/// rectangle without the grid inventing a style it was never given.
static ABSENT: View = View::empty();

/// The number of columns a grid has.
///
/// Every row holds that many cells. A ragged grid is a contract violation, so
/// this is the widest row rather than a shape the grid negotiated.
pub(super) fn columns(rows: &[Vec<View>]) -> usize {
    debug_assert!(
        rows.windows(2).all(|pair| pair[0].len() == pair[1].len()),
        "a grid is a rectangle: every row holds the same number of cells"
    );
    rows.iter().map(Vec::len).max().unwrap_or(0)
}

/// The cell at `(row, column)`.
pub(super) fn cell(rows: &[Vec<View>], row: usize, column: usize) -> &View {
    rows[row].get(column).unwrap_or(&ABSENT)
}

/// The padding a cell takes: its own where it states one, the grid's
/// otherwise.
///
/// The grid's value states the ordinary case rather than an invariant the
/// alignment rests on — a column is as wide as its widest cell, whatever
/// padding that cell carries — so a cell replacing it costs the grid nothing.
pub(super) fn cell_padding(style: &GridStyle, cell: &View) -> Sides {
    match box_style(cell) {
        Some(block) if block.padding_sides() != Sides::default() => Sides::default(),
        _ => style.cell_padding_sides(),
    }
}

/// How a cell narrower than its column is placed in it.
///
/// Nothing is renegotiated: the remainder is the cell's own alignment fill,
/// exactly as a `Row` child below its share is placed. A cell that is not a
/// box states no alignment, and takes the default.
pub(super) fn cell_alignment(cell: &View) -> (Align, VerticalAlign) {
    match box_style(cell) {
        Some(block) => (block.horizontal_alignment(), block.vertical_alignment()),
        None => (Align::default(), VerticalAlign::default()),
    }
}

/// The style of the box a cell is, if it is one.
///
/// An anchor is a box in every respect sizing and placement reason about, so a
/// grid reads its style the same way it reads a plain block's.
const fn box_style(cell: &View) -> Option<&BlockStyle> {
    match cell {
        View::Block(block, _) | View::AnchorBlock(_, block, _) => Some(block),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Length, TextStyle};

    fn text(content: &str) -> View {
        View::text(content, TextStyle::new())
    }

    #[test]
    fn every_cell_of_a_rectangle_is_the_one_that_was_put_there() {
        let rows = vec![vec![text("a"), text("b")], vec![text("c"), text("d")]];

        assert_eq!(columns(&rows), 2);
        assert_eq!(cell(&rows, 1, 0), &text("c"));
        assert_eq!(cell(&rows, 1, 1), &text("d"));
    }

    #[test]
    #[should_panic(expected = "a grid is a rectangle")]
    fn rejects_a_ragged_grid() {
        let _ = columns(&[vec![text("a"), text("b")], vec![text("c")]]);
    }

    #[test]
    fn a_cell_a_ragged_grid_does_not_hold_is_empty() {
        // What a release build resolves where the contract was broken: the
        // shape stays a rectangle, and the grid invents no style.
        let rows = vec![vec![text("a"), text("b")], vec![text("c")]];

        assert_eq!(cell(&rows, 1, 1), &View::empty());
    }

    #[test]
    fn a_cell_that_states_padding_replaces_the_grids() {
        let style = GridStyle::new().cell_padding((0, 1));
        let stated = View::block(BlockStyle::new().padding((1, 2)), text("a"));

        assert_eq!(cell_padding(&style, &text("a")), Sides::from((0, 1)));
        assert_eq!(
            cell_padding(&style, &View::block(BlockStyle::new(), text("a"))),
            Sides::from((0, 1)),
            "a box that states none takes the grid's"
        );
        assert_eq!(
            cell_padding(&style, &stated),
            Sides::default(),
            "and one that states its own keeps only that"
        );
    }

    #[test]
    fn a_cell_that_is_not_a_box_states_no_alignment() {
        let aligned = View::block(
            BlockStyle::new()
                .align(Align::Right)
                .align_vertical(VerticalAlign::Bottom),
            text("a"),
        );

        assert_eq!(
            cell_alignment(&text("a")),
            (Align::Left, VerticalAlign::Top)
        );
        assert_eq!(
            cell_alignment(&aligned),
            (Align::Right, VerticalAlign::Bottom)
        );
    }

    #[test]
    fn a_stated_column_length_is_read_by_index() {
        let style = GridStyle::new().columns([Some(Length::fill(2))]);

        assert_eq!(style.column_length(0), Some(Length::fill(2)));
        assert_eq!(style.column_length(1), None);
    }
}
