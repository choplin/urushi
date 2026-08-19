//! The grid node: one width per column across every row, and the lines the
//! container draws between its cells.

use urushi::{
    Align, Available, BlockStyle, Border, GridStyle, Length, Size, StyledGrapheme, TextStyle,
    VerticalAlign, View, measure, resolve,
};

fn text(content: &str) -> View {
    View::text(content, TextStyle::new())
}

fn rows(view: &View, available: Available) -> Vec<String> {
    resolve(view, available)
        .rows()
        .iter()
        .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
        .collect()
}

/// A grid whose column demands and lines add up to 41 cells: 8, 19, and 10 for
/// the three columns, plus two outer edges and two column rules.
fn report() -> View {
    View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [
            [text("id"), text("name"), text("status")],
            [text("000001"), text("urushi-view-model"), text("ok")],
            [text("2"), text("short"), text("failed!!")],
        ],
    )
}

#[test]
fn a_column_is_as_wide_as_its_widest_cell_in_every_row() {
    let view = report();

    assert_eq!(measure(&view), Size::new(41, 7));
    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            "┌────────┬───────────────────┬──────────┐",
            "│ id     │ name              │ status   │",
            "├────────┼───────────────────┼──────────┤",
            "│ 000001 │ urushi-view-model │ ok       │",
            "├────────┼───────────────────┼──────────┤",
            "│ 2      │ short             │ failed!! │",
            "└────────┴───────────────────┴──────────┘",
        ]
    );
}

#[test]
fn a_narrow_area_moves_the_lines_and_the_column_boundaries_together() {
    // The whole point of the node: the separators are drawn from the widths
    // the pass decided, so a grid squeezed to less than half its demand still
    // has every vertical in one column of the rectangle.
    let resolved = rows(&report(), Available::columns(18));

    assert_eq!(
        resolved,
        vec![
            "┌───┬───────┬────┐",
            "│ i │ name  │ st │",
            "│ d │       │ at │",
            "│   │       │ us │",
            "├───┼───────┼────┤",
            "│ 0 │ urush │ ok │",
            "│ 0 │ i-vie │    │",
            "│ 0 │ w-mod │    │",
            "│ 0 │ el    │    │",
            "│ 0 │       │    │",
            "│ 1 │       │    │",
            "├───┼───────┼────┤",
            "│ 2 │ short │ fa │",
            "│   │       │ il │",
            "│   │       │ ed │",
            "│   │       │ !! │",
            "└───┴───────┴────┘",
        ]
    );

    // Every glyph that carries a vertical line, whether it turns there or not.
    let verticals = |line: &str| {
        line.chars()
            .enumerate()
            .filter(|(_, glyph)| "│├┼┤┌┬┐└┴┘".contains(*glyph))
            .map(|(column, _)| column)
            .collect::<Vec<_>>()
    };
    for line in &resolved {
        assert_eq!(
            verticals(line),
            vec![0, 4, 12, 17],
            "every row breaks at the same four columns: {line}"
        );
    }
}

#[test]
fn an_area_wider_than_the_grid_leaves_it_at_its_own_width() {
    // A grid claims its columns' demand and no more: the remainder belongs to
    // whatever contains it, exactly as an auto `Row` leaves it.
    assert_eq!(
        resolve(&report(), Available::columns(200)).size(),
        Size::new(41, 7)
    );
    assert_eq!(
        rows(&report(), Available::columns(200)),
        rows(&report(), Available::NONE)
    );
}

#[test]
fn a_column_claim_is_built_from_each_cell_alone() {
    // The no-solver boundary: a column's width is the greatest of what its own
    // cells measure to, never a number that depends on what a sibling column
    // resolved to. Widening one column therefore leaves the others untouched.
    let narrow = report();
    let wide = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [
            [text("id"), text("name"), text("status")],
            [
                text("000001"),
                text("urushi-view-model-and-then-some"),
                text("ok"),
            ],
            [text("2"), text("short"), text("failed!!")],
        ],
    );

    let boundary = |view: &View| rows(view, Available::NONE)[0].find('┬').unwrap();
    assert_eq!(boundary(&narrow), boundary(&wide));
    assert_eq!(
        measure(&wide).width() - measure(&narrow).width(),
        "-and-then-some".len(),
        "only the column that grew is wider"
    );
}

#[test]
fn a_stated_column_takes_its_size_and_shrinks_last() {
    let marker = |glyph: &str| {
        View::grid(
            GridStyle::new()
                .border(Border::NORMAL)
                .cell_padding((0, 1))
                .columns([Some(Length::Cells(3)), None]),
            [[text(glyph), text("a description that is long")]],
        )
    };

    assert_eq!(
        rows(&marker("!"), Available::NONE),
        vec![
            "┌───┬────────────────────────────┐",
            "│ ! │ a description that is long │",
            "└───┴────────────────────────────┘",
        ],
        "the stated column takes its size rather than its content's"
    );
    assert_eq!(
        rows(&marker("!"), Available::columns(20)),
        vec![
            "┌───┬──────────────┐",
            "│ ! │ a            │",
            "│   │ description  │",
            "│   │ that is long │",
            "└───┴──────────────┘",
        ],
        "and gives nothing up while the auto column still can"
    );
}

#[test]
fn a_fill_column_divides_what_the_others_leave() {
    let view = View::grid(
        GridStyle::new()
            .border(Border::NORMAL)
            .cell_padding((0, 1))
            .columns([None, Some(Length::Fill(1))]),
        [[text("key"), text("value")]],
    );

    assert_eq!(
        rows(&view, Available::columns(24)),
        vec![
            "┌─────┬────────────────┐",
            "│ key │ value          │",
            "└─────┴────────────────┘",
        ]
    );
    assert_eq!(
        measure(&view),
        Size::new(15, 3),
        "with no area to divide, a fill column contributes its intrinsic width"
    );
}

#[test]
fn a_row_is_as_tall_as_its_tallest_cell() {
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [[
            text("flat"),
            View::block(BlockStyle::new().border(Border::ROUNDED), text("boxed")),
        ]],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            "┌──────┬─────────┐",
            "│ flat │ ╭─────╮ │",
            "│      │ │boxed│ │",
            "│      │ ╰─────╯ │",
            "└──────┴─────────┘",
        ]
    );
}

#[test]
fn a_cell_shorter_or_narrower_than_its_place_is_put_by_its_own_alignment() {
    let cell = |align, vertical| {
        View::block(
            BlockStyle::new().align(align).align_vertical(vertical),
            text("x"),
        )
    };
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL),
        [[
            text("wide cell\nover\nthree rows"),
            cell(Align::Right, VerticalAlign::Bottom),
            cell(Align::Center, VerticalAlign::Center),
        ]],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            "┌──────────┬─┬─┐",
            "│wide cell │ │ │",
            "│over      │ │x│",
            "│three rows│x│ │",
            "└──────────┴─┴─┘",
        ]
    );
}

#[test]
fn every_intersection_takes_the_glyph_its_four_directions_name() {
    let view = View::grid(
        GridStyle::new().border(Border::THICK),
        [
            [text("a"), text("b")],
            [text("c"), text("d")],
            [text("e"), text("f")],
        ],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            "┏━┳━┓", // two corners and a tee opening downward
            "┃a┃b┃",
            "┣━╋━┫", // two tees and a cross
            "┃c┃d┃",
            "┣━╋━┫",
            "┃e┃f┃",
            "┗━┻━┛", // two corners and a tee opening upward
        ]
    );
}

#[test]
fn a_disabled_edge_removes_the_line_and_the_cell_it_occupied() {
    let grid = |style: GridStyle| {
        View::grid(
            style.border(Border::NORMAL).cell_padding((0, 1)),
            [[text("a"), text("bb")], [text("ccc"), text("d")]],
        )
    };

    assert_eq!(
        rows(
            &grid(GridStyle::new().border_column(false)),
            Available::NONE
        ),
        vec![
            "┌─────────┐",
            "│ a    bb │",
            "├─────────┤",
            "│ ccc  d  │",
            "└─────────┘",
        ],
        "the row rule crosses no vertical, so it has no tee to draw"
    );
    assert_eq!(
        rows(&grid(GridStyle::new().border_row(false)), Available::NONE),
        vec![
            "┌─────┬────┐",
            "│ a   │ bb │",
            "│ ccc │ d  │",
            "└─────┴────┘"
        ]
    );
    assert_eq!(
        rows(
            &grid(GridStyle::new().border_left(false).border_right(false)),
            Available::NONE
        ),
        vec![
            "─────┬────",
            " a   │ bb ",
            "─────┼────",
            " ccc │ d  ",
            "─────┴────",
        ],
        "a horizontal line without a left edge starts with the horizontal glyph"
    );
}

#[test]
fn a_grid_with_no_glyph_set_draws_nothing_and_occupies_nothing() {
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
        [[text("a"), text("bb")], [text("ccc"), text("d")]],
    );

    assert_eq!(measure(&view), Size::new(9, 2), "no line takes a cell");
    assert_eq!(rows(&view, Available::NONE), vec![" a    bb ", " ccc  d  "]);
}

#[test]
fn a_cell_that_states_padding_replaces_the_grids_and_still_lines_up() {
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [
            [
                View::block(BlockStyle::new().padding((0, 3)), text("a")),
                text("b"),
            ],
            [text("cc"), text("d")],
        ],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            "┌───────┬───┐",
            "│   a   │ b │",
            "├───────┼───┤",
            "│ cc    │ d │",
            "└───────┴───┘",
        ],
        "the wider padding sets the column, and the other cell keeps the grid's"
    );
}

#[test]
fn a_grid_with_no_cells_is_still_a_closed_frame() {
    let empty: [[View; 0]; 0] = [];
    let view = View::grid(GridStyle::new().border(Border::NORMAL), empty);

    assert_eq!(
        measure(&view),
        Size::new(2, 2),
        "the outer edges, and nothing between"
    );
    assert_eq!(rows(&view, Available::NONE), vec!["──", "──"]);
}

#[test]
fn a_single_cell_has_no_gap_for_a_separator() {
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [[text("only")]],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec!["┌──────┐", "│ only │", "└──────┘"]
    );
}

#[test]
fn an_area_too_small_for_the_frame_reaches_the_crop_rather_than_an_open_one() {
    // The degenerate safety net, not a layout rule: the columns and the lines
    // shrink together until nothing is left to give, and the crop bounds what
    // remains. Every row still has the area's width.
    let resolved = resolve(&report(), Available::size(6, 5));

    assert_eq!(resolved.size(), Size::new(6, 5));
    for row in resolved.rows() {
        assert_eq!(
            row.iter().map(StyledGrapheme::width).sum::<usize>(),
            6,
            "every row is exactly the area wide"
        );
    }
}

#[test]
fn a_bounded_height_divides_the_rows_the_way_a_column_divides_its_children() {
    let stack = |content: &str| View::block(BlockStyle::new(), text(content));
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [[stack("one\ntwo\nthree")], [stack("four")]],
    );

    assert_eq!(measure(&view), Size::new(9, 7));
    assert_eq!(
        rows(&view, Available::size(9, 5)),
        vec![
            "┌───────┐",
            "│ one   │",
            "├───────┤",
            "│ four  │",
            "└───────┘",
        ],
        "the taller row gives up the rows it can, and the line moves with it"
    );
}

#[test]
fn an_anchor_inside_a_cell_reports_where_the_grid_put_it() {
    // Assembly is the phase that knows the offsets a grid introduces: the
    // outer edge, the columns and rules before the cell, the rows above, and
    // the cell's own padding and alignment.
    let view = View::grid(
        GridStyle::new().border(Border::NORMAL).cell_padding((0, 1)),
        [
            [text("id"), text("chart")],
            [
                text("1"),
                View::anchor_block("chart", BlockStyle::new().width(5).height(2), View::empty()),
            ],
        ],
    );

    let resolved = resolve(&view, Available::NONE);
    let region = resolved.anchor("chart").expect("the anchor resolved");

    assert_eq!(
        (region.x(), region.y()),
        (7, 3),
        "across: one outer edge, the four-cell id column, one rule, one cell \
         pad; down: the top edge, the first row, one rule"
    );
    assert_eq!((region.width(), region.height()), (5, 2));
}

#[test]
fn an_anchor_cell_is_placed_by_its_own_alignment_like_any_box() {
    let view = View::grid(
        GridStyle::new(),
        [
            [text("a wide heading")],
            [View::anchor_block(
                "region",
                BlockStyle::new().align(Align::Right).width(4),
                View::empty(),
            )],
        ],
    );

    let resolved = resolve(&view, Available::NONE);
    let region = resolved.anchor("region").expect("the anchor resolved");

    assert_eq!(
        (region.x(), region.y()),
        (10, 1),
        "the column is 14 wide and the box takes 4 of them, aligned right"
    );
}
