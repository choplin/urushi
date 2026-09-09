//! The grid node: a rectangle with one shared width per column.

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

/// A grid whose three column demands are 8, 19, and 10 cells.
fn report() -> View {
    View::grid(
        GridStyle::new().cell_padding((0, 1)),
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

    assert_eq!(measure(&view), Size::new(37, 3));
    assert_eq!(
        rows(&view, Available::NONE),
        vec![
            " id      name               status   ",
            " 000001  urushi-view-model  ok       ",
            " 2       short              failed!! ",
        ]
    );
}

#[test]
fn a_narrow_area_keeps_every_row_at_the_shared_column_widths() {
    let resolved = resolve(&report(), Available::columns(18));

    assert_eq!(resolved.size().width(), 18);
    assert!(resolved.size().height() > 3, "narrow cells wrap");
    for row in resolved.rows() {
        assert_eq!(
            row.iter().map(StyledGrapheme::width).sum::<usize>(),
            18,
            "every row uses the same settled columns"
        );
    }
}

#[test]
fn an_area_wider_than_the_grid_leaves_it_at_its_own_width() {
    assert_eq!(
        resolve(&report(), Available::columns(200)).size(),
        Size::new(37, 3)
    );
    assert_eq!(
        rows(&report(), Available::columns(200)),
        rows(&report(), Available::NONE)
    );
}

#[test]
fn a_column_claim_is_built_from_each_cell_alone() {
    let narrow = report();
    let wide = View::grid(
        GridStyle::new().cell_padding((0, 1)),
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
                .cell_padding((0, 1))
                .columns([Some(Length::Cells(3)), None]),
            [[text(glyph), text("a description that is long")]],
        )
    };

    assert_eq!(
        rows(&marker("!"), Available::NONE),
        vec![" !  a description that is long "]
    );
    assert_eq!(
        rows(&marker("!"), Available::columns(20)),
        vec![" !  a description   ", "    that is long    "],
        "the stated column gives nothing up while the auto column still can"
    );
}

#[test]
fn a_fill_column_divides_what_the_others_leave() {
    let view = View::grid(
        GridStyle::new()
            .cell_padding((0, 1))
            .columns([None, Some(Length::fill(1))]),
        [[text("key"), text("value")]],
    );

    assert_eq!(
        rows(&view, Available::columns(24)),
        vec![" key  value             "]
    );
    assert_eq!(
        measure(&view),
        Size::new(12, 1),
        "without a bounded area, a fill column contributes its intrinsic width"
    );
}

#[test]
fn a_row_is_as_tall_as_its_tallest_cell() {
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
        [[
            text("flat"),
            View::block(BlockStyle::new().border(Border::ROUNDED), text("boxed")),
        ]],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec![" flat  ╭─────╮ ", "       │boxed│ ", "       ╰─────╯ "]
    );
}

#[test]
fn a_cell_shorter_or_narrower_than_its_place_uses_its_own_alignment() {
    let cell = |align, vertical| {
        View::block(
            BlockStyle::new().align(align).align_vertical(vertical),
            text("x"),
        )
    };
    let view = View::grid(
        GridStyle::new(),
        [[
            text("wide cell\nover\nthree rows"),
            cell(Align::Right, VerticalAlign::Bottom),
            cell(Align::Center, VerticalAlign::Center),
        ]],
    );

    assert_eq!(
        rows(&view, Available::NONE),
        vec!["wide cell   ", "over       x", "three rowsx "]
    );
}

#[test]
fn a_cell_that_states_padding_replaces_the_grids_and_still_lines_up() {
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
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
        vec!["   a    b ", " cc     d "],
        "the wider padding sets the column, and the other cell keeps the grid's"
    );
}

#[test]
fn a_grid_with_no_cells_is_empty() {
    let empty: [[View; 0]; 0] = [];
    let view = View::grid(GridStyle::new(), empty);

    assert_eq!(measure(&view), Size::ZERO);
    assert!(rows(&view, Available::NONE).is_empty());
}

#[test]
fn a_surrounding_block_supplies_box_geometry() {
    let grid = View::grid(
        GridStyle::new().cell_padding((0, 1)),
        [[text("a"), text("bb")], [text("ccc"), text("d")]],
    );
    let view = View::block(BlockStyle::new().border(Border::NORMAL), grid);

    assert_eq!(
        rows(&view, Available::NONE),
        vec!["┌─────────┐", "│ a    bb │", "│ ccc  d  │", "└─────────┘"]
    );
}

#[test]
fn a_bounded_height_divides_the_rows_the_way_a_column_divides_its_children() {
    let stack = |content: &str| View::block(BlockStyle::new(), text(content));
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
        [[stack("one\ntwo\nthree")], [stack("four")]],
    );

    assert_eq!(measure(&view), Size::new(7, 4));
    assert_eq!(
        rows(&view, Available::size(7, 2)),
        vec![" one   ", " four  "]
    );
}

#[test]
fn an_anchor_inside_a_cell_reports_where_the_grid_put_it() {
    let view = View::grid(
        GridStyle::new().cell_padding((0, 1)),
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

    assert_eq!((region.x(), region.y()), (5, 1));
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

    assert_eq!((region.x(), region.y()), (10, 1));
}
