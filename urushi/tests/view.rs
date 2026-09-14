//! The view tree: composition of blocks inside rows, the two alignment biases,
//! and what one layout pass hands a renderer.

use urushi::{
    Align, Available, BlockStyle, BlockTitle, Border, Color, ColorLevel, Length, Modifier,
    Overflow, RenderSettings, ResolvedView, Size, StyledGrapheme, StyledText, TextSpan, TextStyle,
    UnderlineStyleSet, VerticalAlign, View, measure, render, resolve,
};

fn resolve_ok(view: &View, available: Available) -> ResolvedView {
    resolve(view, available).unwrap()
}

fn ansi_settings() -> RenderSettings {
    RenderSettings::default()
        .with_colors(ColorLevel::TrueColor)
        .with_modifiers(Modifier::all())
        .with_underline_styles(UnderlineStyleSet::all())
        .with_underline_colors(true)
        .with_hyperlinks(true)
}

fn plain_rows(view: &View) -> Vec<String> {
    resolve_ok(view, Available::NONE)
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
    let resolved = resolve_ok(&view, Available::NONE);

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
fn an_area_wider_than_a_text_leaf_leaves_it_at_its_own_width() {
    // The clamp only caps. A leaf offered more columns than it needs keeps the
    // width its lines need, so nothing downstream sees phantom trailing cells.
    let view = View::text("hi", TextStyle::new());

    assert_eq!(resolve_ok(&view, Available::columns(20)).size().width(), 2);
}

#[test]
fn a_stated_block_width_still_lays_its_text_out_across_that_width() {
    // Padding a narrow child out to its container is assembly's job, not the
    // text's width. A leaf that stops at 2 is still placed inside 20.
    let view = View::block(
        BlockStyle::new().width(20).align(Align::Center),
        View::text("hi", TextStyle::new()),
    );

    assert_eq!(
        view_rows(&view, Available::NONE),
        vec!["         hi         "]
    );
}

#[test]
fn a_centered_column_offsets_a_narrow_child_against_a_wide_one() {
    // The offset is the difference between the child's width and the column's.
    // The area reaches every child, so a child that read it as a size to take
    // would come out as wide as its sibling and leave nothing to centre.
    let view = View::column(
        Align::Center,
        [
            View::text("hi", TextStyle::new()),
            View::text("long text", TextStyle::new()),
        ],
    );

    assert_eq!(
        view_rows(&view, Available::columns(20)),
        vec!["   hi    ", "long text"]
    );
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
    let resolved = resolve_ok(&view, Available::NONE);
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
    assert_eq!(resolve_ok(&view, Available::NONE).size().width(), 8);
    assert_eq!(resolve_ok(&view, Available::columns(4)).size().width(), 4);
    assert_eq!(resolve_ok(&view, Available::size(4, 1)).size().height(), 1);
}

#[test]
fn a_renderer_coalesces_adjacent_graphemes_of_equal_style() {
    let view = View::row(
        VerticalAlign::Top,
        [
            View::text("ab", TextStyle::new().foreground(Color::RED)),
            View::text("cd", TextStyle::new().foreground(Color::RED)),
            View::text("ef", TextStyle::new()),
        ],
    );

    assert_eq!(
        render(&resolve_ok(&view, Available::NONE), &ansi_settings()),
        "\x1b[31mabcd\x1b[0mef",
        "one SGR scope spans the run, not one per node"
    );
}

#[test]
fn a_style_renders_the_single_block_case_of_the_same_pass() {
    let style = BlockStyle::new().border(Border::ROUNDED).padding((0, 1));
    let block = View::block(style, View::text("ok", TextStyle::new()));
    let resolved = resolve_ok(&block, Available::NONE);

    assert_eq!(
        render(&resolved, &ansi_settings()),
        "╭────╮\n│ ok │\n╰────╯"
    );
    assert_eq!(resolved.size(), Size::new(6, 3));
}

#[test]
fn a_title_is_block_content_and_sets_an_automatic_width_demand() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        "Files",
        View::text("x", TextStyle::new()),
    );

    assert_eq!(measure(&view), Size::new(9, 3));
    assert_eq!(
        plain_rows(&view),
        vec!["┌ Files ┐", "│x      │", "└───────┘"]
    );
}

#[test]
fn a_title_aligns_its_complete_slot_between_the_corners() {
    let titled = |align| {
        View::titled_block(
            BlockStyle::new().border(Border::ROUNDED).width(12),
            BlockTitle::new("X").align(align),
            View::empty(),
        )
    };

    assert_eq!(plain_rows(&titled(Align::Left))[0], "╭ X ───────╮");
    assert_eq!(plain_rows(&titled(Align::Center))[0], "╭─── X ────╮");
    assert_eq!(plain_rows(&titled(Align::Right))[0], "╭─────── X ╮");
}

#[test]
fn a_narrow_title_gives_up_padding_then_clips_between_graphemes() {
    let titled = |width| {
        View::titled_block(
            BlockStyle::new().border(Border::NORMAL).width(width),
            BlockTitle::new("日本語"),
            View::empty(),
        )
    };

    assert_eq!(view_rows(&titled(5), Available::NONE)[0], "┌ 日┐");
    assert_eq!(view_rows(&titled(4), Available::NONE)[0], "┌日┐");
    assert_eq!(
        view_rows(&titled(2), Available::NONE)[0],
        "┌┐",
        "corners survive when no title grapheme fits"
    );
}

#[test]
fn title_spans_keep_complete_styles_and_padding_keeps_the_border_style() {
    let border = TextStyle::new().foreground(Color::RED);
    let first = TextStyle::new().foreground(Color::CYAN).bold();
    let second = TextStyle::new().foreground(Color::GREEN);
    let title = StyledText::try_from_spans([
        TextSpan::new("A", first.clone()),
        TextSpan::new("B", second.clone()),
    ])
    .unwrap();
    let view = View::titled_block(
        BlockStyle::new()
            .border(Border::DOUBLE)
            .border_foreground(Color::RED),
        BlockTitle::new(title),
        View::empty(),
    );

    let resolved = resolve_ok(&view, Available::NONE);
    let top = &resolved.rows()[0];
    assert_eq!(
        top.iter().map(StyledGrapheme::symbol).collect::<String>(),
        "╔ AB ╗"
    );
    assert_eq!(top[0].style(), &border);
    assert_eq!(top[1].style(), &border);
    assert_eq!(top[2].style(), &first);
    assert_eq!(top[3].style(), &second);
    assert_eq!(top[4].style(), &border);
    assert_eq!(top[5].style(), &border);
}

#[test]
fn a_title_uses_the_whole_top_edge_when_side_borders_are_disabled() {
    let view = View::titled_block(
        BlockStyle::new()
            .border(Border::NORMAL)
            .border_left(false)
            .border_right(false),
        BlockTitle::new("Files"),
        View::text("x", TextStyle::new()),
    );

    assert_eq!(plain_rows(&view), vec![" Files ", "x      ", "───────"]);
}

#[test]
fn an_empty_title_does_not_open_the_border_or_add_width() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        BlockTitle::new(""),
        View::text("x", TextStyle::new()),
    );

    assert_eq!(measure(&view).width(), 3);
    assert_eq!(plain_rows(&view)[0], "┌─┐");
}

#[test]
fn explicit_and_external_width_caps_clip_a_title_without_changing_height() {
    let automatic = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        "abcdefgh",
        View::text("x", TextStyle::new()),
    );
    let explicit = View::titled_block(
        BlockStyle::new().border(Border::NORMAL).width(7),
        "abcdefgh",
        View::text("x", TextStyle::new()),
    );
    let maximum = View::titled_block(
        BlockStyle::new().border(Border::NORMAL).max_width(6),
        "abcdefgh",
        View::text("x", TextStyle::new()),
    );

    assert_eq!(measure(&automatic), Size::new(12, 3));
    assert_eq!(plain_rows(&explicit)[0], "┌abcde┐");
    assert_eq!(plain_rows(&maximum)[0], "┌abcd┐");
    assert_eq!(view_rows(&automatic, Available::columns(5))[0], "┌abc┐");
    assert_eq!(
        resolve_ok(&automatic, Available::columns(5)).size(),
        Size::new(5, 3)
    );
}

#[test]
fn a_fill_width_uses_its_allocation_instead_of_the_title_demand() {
    let view = View::titled_block(
        BlockStyle::new()
            .border(Border::NORMAL)
            .width(Length::fill(1)),
        "Title",
        View::empty(),
    );

    assert_eq!(measure(&view).width(), 9);
    assert_eq!(resolve_ok(&view, Available::columns(12)).size().width(), 12);
    assert_eq!(resolve_ok(&view, Available::columns(6)).size().width(), 6);
}

#[test]
fn an_automatic_titled_block_keeps_its_title_demand_over_a_fill_child() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        "Files",
        View::block(BlockStyle::new().width(Length::fill(1)), View::empty()),
    );

    assert_eq!(measure(&view), Size::new(9, 2));
    assert_eq!(resolve_ok(&view, Available::NONE).size(), Size::new(9, 2));
    assert_eq!(plain_rows(&view)[0], "┌ Files ┐");
    assert_eq!(resolve_ok(&view, Available::columns(12)).size().width(), 12);
}

#[test]
fn a_minimum_width_applies_after_title_demand() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL).min_width(10),
        "X",
        View::empty(),
    );

    assert_eq!(measure(&view).width(), 10);
    assert_eq!(plain_rows(&view)[0], "┌ X ─────┐");
}

#[test]
fn block_overflow_does_not_add_a_marker_to_a_clipped_title() {
    let view = View::titled_block(
        BlockStyle::new()
            .border(Border::NORMAL)
            .width(6)
            .overflow(Overflow::ellipsis()),
        "abcdefgh",
        View::text("12345678", TextStyle::new()),
    );

    assert_eq!(plain_rows(&view), vec!["┌abcd┐", "│123…│", "└────┘"]);
}

#[test]
fn a_nonempty_zero_width_title_still_demands_its_padding() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        "\u{0301}",
        View::empty(),
    );

    assert_eq!(measure(&view).width(), 4);
}

#[test]
fn an_emoji_title_is_clipped_as_one_grapheme() {
    let titled = |width| {
        View::titled_block(
            BlockStyle::new().border(Border::NORMAL).width(width),
            "👩‍💻",
            View::empty(),
        )
    };

    assert_eq!(plain_rows(&titled(4))[0], "┌👩‍💻┐");
    assert_eq!(plain_rows(&titled(3))[0], "┌─┐");
}

#[test]
fn title_placement_respects_each_border_edge_toggle() {
    let top =
        |style: BlockStyle| plain_rows(&View::titled_block(style.width(6), "X", View::empty()));

    assert_eq!(
        top(BlockStyle::new().border(Border::NORMAL).border_left(false))[0],
        " X ──┐"
    );
    assert_eq!(
        top(BlockStyle::new().border(Border::NORMAL).border_right(false))[0],
        "┌ X ──"
    );
    assert_eq!(
        top(BlockStyle::new()
            .border(Border::NORMAL)
            .border_bottom(false)),
        vec!["┌ X ─┐"]
    );
}

#[test]
fn block_spacing_and_title_padding_have_independent_widths() {
    let view = View::titled_block(
        BlockStyle::new()
            .border(Border::NORMAL)
            .padding((0, 2))
            .margin((0, 1)),
        BlockTitle::new("X").padding(0),
        View::text("long", TextStyle::new()),
    );

    assert_eq!(measure(&view), Size::new(12, 3));
    assert_eq!(plain_rows(&view)[0], " ┌X───────┐ ");
}

#[test]
fn extreme_title_padding_saturates_and_degrades_before_text() {
    let view = View::titled_block(
        BlockStyle::new().border(Border::NORMAL),
        BlockTitle::new("X").padding(u16::MAX),
        View::empty(),
    );

    assert_eq!(view_rows(&view, Available::columns(3))[0], "┌X┐");
    assert_eq!(view_rows(&view, Available::columns(2))[0], "┌┐");
    assert_eq!(view_rows(&view, Available::columns(1))[0], "┌");
    assert_eq!(view_rows(&view, Available::columns(0)), vec!["", ""]);
}

#[test]
#[should_panic(expected = "a block title must be exactly one line")]
fn a_block_title_rejects_multiple_lines() {
    let _ = BlockTitle::new("top\nbottom");
}

#[test]
#[should_panic(expected = "a titled block requires an enabled top border")]
fn a_titled_block_rejects_a_missing_top_edge() {
    let _ = View::titled_block(
        BlockStyle::new().border(Border::NORMAL).border_top(false),
        BlockTitle::new("Title"),
        View::empty(),
    );
}

// --- The area as an input to layout.

#[test]
fn a_bounded_area_closes_the_frame_instead_of_cutting_it() {
    let view = View::block(
        BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
        View::text("hello world", TextStyle::new()),
    );

    let resolved = resolve_ok(&view, Available::columns(7));
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
    resolve_ok(view, available)
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
            .width(Length::fill(1)),
        View::text("ab", TextStyle::new()),
    );

    assert_eq!(resolve_ok(&view, Available::columns(10)).size().width(), 10);
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

// --- How siblings share an area.

/// A bordered box of `width`, holding `text`.
fn panel(text: &str, width: impl Into<Length>) -> View {
    View::block(
        BlockStyle::new().border(Border::NORMAL).width(width),
        View::text(text, TextStyle::new()),
    )
}

#[test]
fn a_row_gives_stated_children_their_size_and_the_rest_to_fill() {
    // The most ordinary full-screen layout: a fixed sidebar, and main takes
    // what is left.
    let view = View::row(
        VerticalAlign::Top,
        [panel("nav", 6), panel("main", Length::fill(1))],
    );

    assert_eq!(
        view_rows(&view, Available::columns(16)),
        vec!["┌────┐┌────────┐", "│nav ││main    │", "└────┘└────────┘",]
    );
}

#[test]
fn fill_weights_divide_the_remainder_and_equal_weights_split_it() {
    let equal = View::row(
        VerticalAlign::Top,
        [panel("a", Length::fill(1)), panel("b", Length::fill(1))],
    );
    assert_eq!(
        view_rows(&equal, Available::columns(10)).remove(1),
        "│a  ││b  │"
    );

    let weighted = View::row(
        VerticalAlign::Top,
        [panel("a", Length::fill(1)), panel("b", Length::fill(2))],
    );
    assert_eq!(
        view_rows(&weighted, Available::columns(9)).remove(1),
        "│a││b   │"
    );
}

#[test]
fn a_fill_on_the_cross_axis_stretches_to_the_containers_extent() {
    // Nothing is divided across the cross axis: the row hands its own height
    // to every child, and a Fill length there takes all of it.
    let view = View::row(
        VerticalAlign::Top,
        [
            View::block(
                BlockStyle::new()
                    .border(Border::NORMAL)
                    .width(5)
                    .height(Length::fill(1)),
                View::text("nav", TextStyle::new()),
            ),
            View::text("x", TextStyle::new()),
        ],
    );

    assert_eq!(
        view_rows(&view, Available::size(6, 5)),
        vec!["┌───┐x", "│nav│ ", "│   │ ", "│   │ ", "└───┘ "]
    );
}

#[test]
fn a_capped_fill_leaves_its_remainder_unused_and_the_group_is_placed_by_align() {
    // A Fill child capped by its own maximum does not trigger redistribution:
    // its share is handed over whole, the cap leaves the remainder unused, and
    // the row resolves below its area.
    let capped = View::row(
        VerticalAlign::Top,
        [
            View::block(
                BlockStyle::new()
                    .border(Border::NORMAL)
                    .width(Length::fill(1))
                    .max_width(4),
                View::text("a", TextStyle::new()),
            ),
            panel("b", Length::fill(1)),
        ],
    );
    assert_eq!(
        resolve_ok(&capped, Available::columns(20)).size().width(),
        14,
        "the capped child's six unused cells are not given to its sibling"
    );

    // Capping a *group* is therefore an enclosing block's max_width, and the
    // block above it places the result.
    let group = View::block(
        BlockStyle::new().max_width(8),
        View::row(
            VerticalAlign::Top,
            [panel("a", 3), panel("b", Length::fill(1))],
        ),
    );
    let view = View::block(
        BlockStyle::new()
            .width(Length::fill(1))
            .align(Align::Center),
        group,
    );

    assert_eq!(
        view_rows(&view, Available::columns(12)),
        vec!["  ┌─┐┌───┐  ", "  │a││b  │  ", "  └─┘└───┘  "]
    );
}

#[test]
fn a_deficit_shrinks_fill_then_auto_then_stated_and_freezes_at_the_floors() {
    let view = || {
        View::row(
            VerticalAlign::Top,
            [
                panel("abcd", 6),
                View::block(
                    BlockStyle::new().border(Border::NORMAL),
                    View::text("abcd", TextStyle::new()),
                ),
                panel("x", Length::fill(1)),
            ],
        )
    };

    // With room to spare every child is at its demand: 6 stated, 6 intrinsic,
    // and the Fill takes the remaining 4.
    assert_eq!(measure(&view()).width(), 15);
    assert_eq!(
        view_rows(&view(), Available::columns(16)).remove(0),
        "┌────┐┌────┐┌──┐"
    );
    // Squeezed to ten, the Fill is already at its floor, so the automatic
    // child gives way next and the stated one last.
    assert_eq!(
        view_rows(&view(), Available::columns(10)).remove(0),
        "┌──┐┌─┐┌─┐"
    );
}

#[test]
fn distribution_never_opens_a_frame() {
    let view = View::row(
        VerticalAlign::Top,
        [
            panel("abc", 5),
            panel("de", Length::fill(1)),
            panel("fghi", Length::fill(2)),
        ],
    );

    // Nine cells is the sum of the three children's floors; below it not even
    // the floors fit, which is the degenerate case the safety net owns.
    for width in 9..=24 {
        let rows = view_rows(&view, Available::columns(width));
        // Three closed frames are twelve corners, wherever the children's
        // differing heights put them.
        let corners: usize = rows
            .iter()
            .map(|row| row.matches(['┌', '┐', '└', '┘']).count())
            .sum();

        assert_eq!(corners, 12, "at {width}: {rows:?}");
    }
}

#[test]
fn a_column_divides_its_height_the_same_way_a_row_divides_its_width() {
    let view = View::column(
        Align::Left,
        [
            View::block(
                BlockStyle::new().height(1),
                View::text("head", TextStyle::new()),
            ),
            View::block(
                BlockStyle::new()
                    .height(Length::fill(1))
                    .border(Border::NORMAL),
                View::text("body", TextStyle::new()),
            ),
            View::text("foot", TextStyle::new()),
        ],
    );

    assert_eq!(
        view_rows(&view, Available::size(6, 6)),
        vec!["head  ", "┌────┐", "│body│", "│    │", "└────┘", "foot  "]
    );
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
