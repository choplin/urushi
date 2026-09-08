//! What a grid is, apart from how large it is: its shape, the padding and
//! placement each cell takes, the rows and columns its own lines occupy, and
//! the glyph at each intersection of those lines.
//!
//! Nothing here decides a width or a height. The phases that do —
//! [`width`](super::width), [`height`](super::height), and
//! [`assemble`](super::assemble) — read these answers the way they read a
//! [`BlockStyle`](crate::BlockStyle): as what the style already fixed. The
//! rules are `docs/design/grid.md`'s, and the edge rules the outer edges follow
//! are `docs/design/border-edges.md`'s.

use crate::{Align, BlockStyle, Border, GridStyle, Sides, VerticalAlign, View};

use super::geometry::Size;

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

/// Whether the gap between row `gap` and row `gap + 1` carries a line.
///
/// Every gap follows `border_row`, except the first one of a grid that states
/// a header: a header row is the one row whose line below it is a statement
/// about the header rather than about the rows in general.
pub(super) fn draws_row_rule(style: &GridStyle, gap: usize) -> bool {
    match style.border_header_enabled() {
        Some(enabled) if gap == 0 => enabled,
        _ => style.is_border_row_enabled(),
    }
}

/// The columns and rows a grid's own lines occupy.
///
/// An enabled outer edge contributes one column or one row; an enabled
/// separator contributes one between each pair of neighbours and none at the
/// ends. A grid with no glyph set draws nothing and occupies nothing.
pub(super) fn line_extent(style: &GridStyle, columns: usize, rows: usize) -> Size {
    if style.border_kind().is_none() {
        return Size::ZERO;
    }
    let columns_between = if style.is_border_column_enabled() {
        columns.saturating_sub(1)
    } else {
        0
    };
    let rows_between = (0..rows.saturating_sub(1))
        .filter(|gap| draws_row_rule(style, *gap))
        .count();
    Size::new(
        usize::from(style.is_border_left_enabled())
            + usize::from(style.is_border_right_enabled())
            + columns_between,
        usize::from(style.is_border_top_enabled())
            + usize::from(style.is_border_bottom_enabled())
            + rows_between,
    )
}

/// The glyph where four directions meet.
///
/// Which of the four carry a line follows from the edge switches and from
/// where the intersection sits; the glyph follows from those four facts alone,
/// which is why no cell states one. `horizontal` and `vertical` are the glyphs
/// of the lines that pass through — the outer edges and the separators use
/// different ones — and answer the cases where a line does not turn.
pub(crate) const fn junction(
    border: &Border,
    up: bool,
    down: bool,
    left: bool,
    right: bool,
    horizontal: char,
    vertical: char,
) -> char {
    match (up, down, left, right) {
        (true, true, true, true) => border.middle,
        (false, true, true, true) => border.middle_top,
        (true, false, true, true) => border.middle_bottom,
        (true, true, false, true) => border.middle_left,
        (true, true, true, false) => border.middle_right,
        (false, true, false, true) => border.top_left,
        (false, true, true, false) => border.top_right,
        (true, false, false, true) => border.bottom_left,
        (true, false, true, false) => border.bottom_right,
        (true, _, false, false) | (false, true, false, false) => vertical,
        (false, false, true, _) | (false, false, false, true) => horizontal,
        (false, false, false, false) => ' ',
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
    fn lines_occupy_the_edges_they_draw_and_the_gaps_between_neighbours() {
        let bordered = GridStyle::new().border(Border::NORMAL);

        assert_eq!(
            line_extent(&GridStyle::new(), 3, 2),
            Size::ZERO,
            "no glyph set, no lines"
        );
        assert_eq!(line_extent(&bordered, 3, 2), Size::new(4, 3));
        assert_eq!(
            line_extent(&bordered.clone().border_column(false), 3, 2),
            Size::new(2, 3)
        );
        assert_eq!(
            line_extent(&bordered.clone().border_row(false), 3, 2),
            Size::new(4, 2)
        );
        assert_eq!(
            line_extent(&bordered.clone().border_left(false), 3, 2),
            Size::new(3, 3)
        );
        assert_eq!(
            line_extent(&bordered, 1, 1),
            Size::new(2, 2),
            "a single cell has no gap for a separator"
        );
        assert_eq!(
            line_extent(
                &bordered.clone().border_row(false).border_header(true),
                3,
                4
            ),
            Size::new(4, 3),
            "a header rule occupies its gap where no other gap carries one"
        );
        assert_eq!(
            line_extent(&bordered.clone().border_header(false), 3, 4),
            Size::new(4, 4),
            "and a header without one takes its gap back from border_row"
        );
        assert_eq!(line_extent(&bordered, 0, 0), Size::new(2, 2));
    }

    #[test]
    fn a_stated_header_owns_the_first_gap_and_only_that_one() {
        let plain = GridStyle::new().border(Border::NORMAL).border_row(false);
        let ruled = GridStyle::new().border(Border::NORMAL);

        assert!(!draws_row_rule(&plain, 0), "no header stated, no rule");
        assert!(draws_row_rule(&plain.clone().border_header(true), 0));
        assert!(
            !draws_row_rule(&plain.clone().border_header(true), 1),
            "the statement is about the header, not about the rows below it"
        );
        assert!(
            !draws_row_rule(&ruled.clone().border_header(false), 0),
            "a header that draws none takes its gap out of border_row's hands"
        );
        assert!(draws_row_rule(&ruled.border_header(false), 1));
    }

    #[test]
    fn a_junction_takes_the_glyph_its_four_directions_name() {
        let border = &Border::NORMAL;
        let at = |up, down, left, right| junction(border, up, down, left, right, '─', '│');

        assert_eq!(at(true, true, true, true), '┼');
        assert_eq!(at(false, true, true, true), '┬');
        assert_eq!(at(true, false, true, true), '┴');
        assert_eq!(at(true, true, false, true), '├');
        assert_eq!(at(true, true, true, false), '┤');
        assert_eq!(at(false, true, false, true), '┌');
        assert_eq!(at(false, true, true, false), '┐');
        assert_eq!(at(true, false, false, true), '└');
        assert_eq!(at(true, false, true, false), '┘');
    }

    #[test]
    fn a_line_that_does_not_turn_keeps_the_glyph_it_arrived_with() {
        let border = &Border::NORMAL;
        let at = |up, down, left, right| junction(border, up, down, left, right, '━', '┃');

        assert_eq!(at(true, true, false, false), '┃', "straight through");
        assert_eq!(at(false, false, true, true), '━');
        assert_eq!(at(true, false, false, false), '┃', "a stub still is one");
        assert_eq!(at(false, false, false, true), '━');
        assert_eq!(at(false, false, false, false), ' ', "nothing meets here");
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
        let style = GridStyle::new().columns([Some(Length::Fill(2))]);

        assert_eq!(style.column_length(0), Some(Length::Fill(2)));
        assert_eq!(style.column_length(1), None);
    }
}
