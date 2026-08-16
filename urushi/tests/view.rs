//! The view tree: composition of blocks inside rows, the two alignment biases,
//! and what one layout pass hands a renderer.

use urushi::{
    Align, AnsiPolicy, AnsiRenderer, BlockStyle, Border, Color, ColorProfile, Limits,
    StyledGrapheme, TerminalProfile, TextStyle, VerticalAlign, View, measure, resolve,
};

fn plain_rows(view: &View) -> Vec<String> {
    resolve(view, Limits::NONE)
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
    let resolved = resolve(&view, Limits::NONE);

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
    let resolved = resolve(&view, Limits::NONE);
    let row = &resolved.rows()[0];

    assert_eq!(row[0].symbol(), "日");
    assert_eq!(row[0].width(), 2);
    assert_eq!(row[0].style(), &accent, "styles stay logical until output");
    assert_eq!(row[1].width(), 1);
}

#[test]
fn limits_are_applied_after_a_blocks_own_maximums() {
    let view = View::block(
        BlockStyle::new().width(10).max_width(8),
        View::text("abcdefghij", TextStyle::new()),
    );

    // A BlockStyle's own maximum crops during resolution, so it is already in
    // the intrinsic size; Limits are applied last, and the smaller bound wins.
    assert_eq!(measure(&view).width(), 8);
    assert_eq!(resolve(&view, Limits::NONE).size().width(), 8);
    assert_eq!(resolve(&view, Limits::width(4)).size().width(), 4);
    assert_eq!(resolve(&view, Limits::size(4, 1)).size().height(), 1);
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
