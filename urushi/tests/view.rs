//! The view tree: composition of blocks inside rows, the two alignment biases,
//! and what one layout pass hands a renderer.

use urushi::{
    Align, AnsiPolicy, AnsiRenderer, Available, BlockStyle, Border, Color, ColorProfile, Length,
    StyledGrapheme, TerminalProfile, TextStyle, VerticalAlign, View, measure, resolve,
};

fn plain_rows(view: &View) -> Vec<String> {
    resolve(view, Available::NONE)
        .rows()
        .iter()
        .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
        .collect()
}

fn badge(text: &str) -> View {
    View::block(
        BlockStyle::new().border(Border::ROUNDED).padding((0, 1)),
        View::text(text, TextStyle::new()),
    )
}

#[test]
fn a_bordered_block_inside_a_row_resolves_to_one_rectangle() {
    // The case a line of inline spans could not express: a block is three rows
    // tall, so the row that contains it is three rows tall too.
    let view = View::row(
        VerticalAlign::Center,
        [
            View::text("status ", TextStyle::new()),
            badge("ok"),
            View::text(" done", TextStyle::new()),
        ],
    );

    assert_eq!(measure(&view), urushi::Size::new(18, 3));
    assert_eq!(
        plain_rows(&view),
        vec![
            "       ╭────╮     ",
            "status │ ok │ done",
            "       ╰────╯     ",
        ]
    );
}

#[test]
fn vertical_padding_and_height_inside_a_row_stay_rectangular() {
    let block = View::block(
        BlockStyle::new().padding(1).height(5),
        View::text("x", TextStyle::new()),
    );
    let view = View::row(
        VerticalAlign::Top,
        [
            View::text("a", TextStyle::new()),
            block,
            View::text("b", TextStyle::new()),
        ],
    );

    let rows = plain_rows(&view);
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|row| row.chars().count() == rows[0].chars().count()),
        "every row spans the same rectangle: {rows:?}"
    );
    assert_eq!(rows[1], "  x  ");
}

#[test]
fn a_row_mixing_cjk_text_and_a_block_keeps_its_columns() {
    let view = View::row(
        VerticalAlign::Center,
        [
            View::text("状態 ", TextStyle::new()),
            badge("完了"),
            View::text(" です", TextStyle::new()),
        ],
    );
    let resolved = resolve(&view, Available::NONE);

    // Width is decided once, by the layout pass, and carried per grapheme.
    for row in resolved.rows() {
        let width: usize = row.iter().map(StyledGrapheme::width).sum();
        assert_eq!(width, resolved.size().width());
    }
    assert_eq!(
        plain_rows(&view),
        vec![
            "     ╭──────╮     ",
            "状態 │ 完了 │ です",
            "     ╰──────╯     ",
        ]
    );
}

#[test]
fn the_two_center_biases_are_separate_rules() {
    // A Row places the odd extra row of a Center alignment above the shorter
    // child; a BlockStyle's vertical_align places it below the content.
    let tall = View::block(
        BlockStyle::new().height(4),
        View::text("1\n2\n3\n4", TextStyle::new()),
    );
    let row = View::row(
        VerticalAlign::Center,
        [tall, View::text("s", TextStyle::new())],
    );
    assert_eq!(
        plain_rows(&row),
        vec!["1 ", "2 ", "3s", "4 "],
        "two blank rows above the shorter child, one below"
    );

    let block = View::block(
        BlockStyle::new()
            .height(4)
            .align_vertical(VerticalAlign::Center),
        View::text("s", TextStyle::new()),
    );
    assert_eq!(
        plain_rows(&block),
        vec![" ", "s", " ", " "],
        "one blank row above the content, two below"
    );
}

#[test]
fn a_column_pads_its_children_to_one_width() {
    let view = View::column(
        Align::Right,
        [
            View::text("a", TextStyle::new()),
            View::text("long", TextStyle::new()),
        ],
    );

    assert_eq!(plain_rows(&view), vec!["   a", "long"]);
}

#[test]
fn a_resolved_view_carries_logical_styles_and_grapheme_widths() {
    let accent = TextStyle::new().foreground(Color::CYAN).bold();
    let view = View::row(
        VerticalAlign::Top,
        [
            View::text("日", accent.clone()),
            View::text("a", TextStyle::new()),
        ],
    );
    let resolved = resolve(&view, Available::NONE);
    let row = &resolved.rows()[0];

    assert_eq!(row[0].symbol(), "日");
    assert_eq!(row[0].width(), 2);
    assert_eq!(row[0].style(), &accent, "styles stay logical until output");
    assert_eq!(row[1].width(), 1);
}

#[test]
fn the_area_and_a_blocks_own_maximum_are_one_cap() {
    let view = View::block(
        BlockStyle::new().width(10).max_width(8),
        View::text("abcdefghij", TextStyle::new()),
    );

    // Both bound the same box, and the tighter one wins: the maximum shapes
    // the intrinsic size, and a narrower area tightens it further.
    assert_eq!(measure(&view).width(), 8);
    assert_eq!(resolve(&view, Available::NONE).size().width(), 8);
    assert_eq!(resolve(&view, Available::columns(4)).size().width(), 4);
    assert_eq!(resolve(&view, Available::size(4, 1)).size().height(), 1);
}

#[test]
fn a_renderer_coalesces_adjacent_graphemes_of_equal_style() {
    let renderer = AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::TrueColor,
        AnsiPolicy::Enabled,
    ));
    let view = View::row(
        VerticalAlign::Top,
        [
            View::text("ab", TextStyle::new().foreground(Color::RED)),
            View::text("cd", TextStyle::new().foreground(Color::RED)),
            View::text("ef", TextStyle::new()),
        ],
    );

    assert_eq!(
        renderer.render(&view).as_str(),
        "\x1b[31mabcd\x1b[0mef",
        "one SGR scope spans the run, not one per node"
    );
}

#[test]
fn a_style_renders_the_single_block_case_of_the_same_pass() {
    let style = BlockStyle::new().border(Border::ROUNDED).padding((0, 1));
    let direct = style.render("ok");
    let through_the_tree = AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::TrueColor,
        AnsiPolicy::Enabled,
    ))
    .render(&View::block(style, View::text("ok", TextStyle::new())));

    assert_eq!(direct.as_str(), through_the_tree.as_str());
    assert_eq!(direct.size(), through_the_tree.size());
}

// --- The area as an input to layout.

#[test]
fn a_bounded_area_closes_the_frame_instead_of_cutting_it() {
    let view = View::block(
        BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
        View::text("hello world", TextStyle::new()),
    );

    let resolved = resolve(&view, Available::columns(7));
    assert_eq!(resolved.size().width(), 7);
    assert_eq!(
        view_rows(&view, Available::columns(7)),
        // Two border columns and two padding columns leave three cells of
        // content, and the text reflows into them.
        vec![
            "┌─────┐",
            "│ hel │",
            "│ lo  │",
            "│ wor │",
            "│ ld  │",
            "└─────┘"
        ]
    );
}

/// Resolving a view under an area, as rows of symbols.
fn view_rows(view: &View, available: Available) -> Vec<String> {
    resolve(view, available)
        .rows()
        .iter()
        .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
        .collect()
}

#[test]
fn a_fill_length_resolves_against_the_area_and_falls_back_to_the_intrinsic_size() {
    let view = View::block(
        BlockStyle::new()
            .border(Border::NORMAL)
            .width(Length::Fill(1)),
        View::text("ab", TextStyle::new()),
    );

    assert_eq!(resolve(&view, Available::columns(10)).size().width(), 10);
    // Under no area a Fill length has nothing to divide, so it contributes the
    // intrinsic size.
    assert_eq!(measure(&view).width(), 4);
}

#[test]
fn a_narrow_area_degrades_margin_then_padding_then_content() {
    let view = || {
        View::block(
            BlockStyle::new()
                .border(Border::NORMAL)
                .padding((0, 1))
                .margin((0, 2)),
            View::text("x", TextStyle::new()),
        )
    };

    // The box is five cells wide inside a four-cell margin.
    assert_eq!(measure(&view()).width(), 9);
    // The margin collapses first, and the box itself is untouched.
    assert_eq!(
        view_rows(&view(), Available::columns(7)),
        vec!["┌───┐", "│ x │", "└───┘"]
    );
    // Then the padding.
    assert_eq!(
        view_rows(&view(), Available::columns(4)),
        vec!["┌─┐", "│x│", "└─┘"]
    );
    // Only an area that cannot hold the frame itself reaches the safety net,
    // and that is the one place a border edge is ever cut.
    assert_eq!(
        view_rows(&view(), Available::columns(2)),
        vec!["┌─", "│x", "└─"]
    );
}

#[test]
fn an_area_bounds_the_height_by_closing_the_frame_lower() {
    let view = View::block(
        BlockStyle::new().border(Border::NORMAL),
        View::text("one\ntwo\nthree", TextStyle::new()),
    );

    assert_eq!(measure(&view).height(), 5);
    assert_eq!(
        view_rows(&view, Available::size(7, 3)),
        vec!["┌─────┐", "│one  │", "└─────┘"]
    );
}

#[test]
fn a_wide_grapheme_survives_a_narrow_area() {
    let view = View::block(
        BlockStyle::new().border(Border::NORMAL),
        View::text("日本", TextStyle::new()),
    );

    // The box cannot go below one wide grapheme plus its frame, so it stays
    // four cells wide and the content reflows.
    assert_eq!(
        view_rows(&view, Available::columns(4)),
        vec!["┌──┐", "│日│", "│本│", "└──┘"]
    );
}

#[test]
fn an_empty_content_box_is_a_closed_frame_of_no_content() {
    let view = View::block(
        BlockStyle::new().border(Border::NORMAL).width(2),
        View::text("", TextStyle::new()),
    );

    // The width leaves no content column, and the empty text still occupies
    // one row, so the frame closes around a zero-width content box.
    assert_eq!(view_rows(&view, Available::NONE), vec!["┌┐", "││", "└┘"]);
}

#[test]
fn a_column_passes_its_cross_axis_area_to_every_child() {
    let view = View::column(
        Align::Left,
        [
            View::text("hello world", TextStyle::new()),
            View::block(
                BlockStyle::new().border(Border::NORMAL),
                View::text("abcdef", TextStyle::new()),
            ),
        ],
    );

    // The width flows down: the bare text wraps and the block's frame closes
    // inside the same area.
    assert_eq!(
        view_rows(&view, Available::columns(6)),
        vec!["hello ", "world ", "┌────┐", "│abcd│", "│ef  │", "└────┘"]
    );
}
