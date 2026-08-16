use urushi::{Align, BlockStyle, Border, Color, Modifier, VerticalAlign, visible_width};

#[test]
fn plain_text_passes_through() {
    assert_eq!(BlockStyle::new().render("hello").into_string(), "hello");
    assert_eq!(BlockStyle::new().render("a\nb").into_string(), "a\nb");
}

#[test]
fn padding_and_border() {
    let style = BlockStyle::new().padding((0, 1)).border(Border::ROUNDED);
    assert_eq!(
        style.render("Hello").into_string(),
        "╭───────╮\n\
         │ Hello │\n\
         ╰───────╯"
    );
}

#[test]
fn cjk_content_keeps_border_aligned() {
    let style = BlockStyle::new().border(Border::NORMAL);
    assert_eq!(
        style.render("日本語").into_string(),
        "┌──────┐\n\
         │日本語│\n\
         └──────┘"
    );
}

#[test]
fn mixed_width_lines_align() {
    let out = BlockStyle::new()
        .border(Border::ASCII)
        .render("ab\nあい")
        .into_string();
    assert_eq!(
        out,
        "+----+\n\
         |ab  |\n\
         |あい|\n\
         +----+"
    );
}

#[test]
fn align_center_with_fixed_width() {
    let out = BlockStyle::new()
        .width(11)
        .align(Align::Center)
        .render("abc")
        .into_string();
    assert_eq!(out, "    abc    ");
}

#[test]
fn align_right_with_fixed_width() {
    let out = BlockStyle::new()
        .width(5)
        .align(Align::Right)
        .render("ab")
        .into_string();
    assert_eq!(out, "   ab");
}

#[test]
fn fixed_height_expands_the_padded_content_box() {
    let style = BlockStyle::new()
        .width(5)
        .height(5)
        .padding((1, 1))
        .border(Border::ASCII);

    assert_eq!(
        style.render("x").into_string(),
        "+-----+\n\
         |     |\n\
         | x   |\n\
         |     |\n\
         |     |\n\
         |     |\n\
         +-----+"
    );
}

#[test]
fn fixed_height_preserves_empty_multiline_and_cjk_content() {
    let style = BlockStyle::new().width(4).height(3);

    assert_eq!(style.render("").into_string(), "    \n    \n    ");
    assert_eq!(style.render("a\nb").into_string(), "a   \nb   \n    ");
    assert_eq!(style.render("日本").into_string(), "日本\n    \n    ");
}

#[test]
fn fixed_height_aligns_content_vertically_inside_padding() {
    let base = BlockStyle::new().width(4).height(6).padding((1, 0));

    assert_eq!(
        base.clone()
            .align_vertical(VerticalAlign::Top)
            .render("x")
            .into_string(),
        "    \nx   \n    \n    \n    \n    "
    );
    assert_eq!(
        base.clone()
            .align_vertical(VerticalAlign::Center)
            .render("x")
            .into_string(),
        "    \n    \nx   \n    \n    \n    "
    );
    assert_eq!(
        base.align_vertical(VerticalAlign::Bottom)
            .render("x")
            .into_string(),
        "    \n    \n    \n    \nx   \n    "
    );
}

#[test]
fn centered_vertical_alignment_puts_an_odd_extra_row_below_like_lip_gloss() {
    let out = BlockStyle::new()
        .width(6)
        .height(7)
        .padding((1, 1))
        .align(Align::Center)
        .align_vertical(VerticalAlign::Center)
        .render("日\nx")
        .into_string();

    assert_eq!(
        out,
        "      \n      \n  日  \n  x   \n      \n      \n      "
    );
}

#[test]
fn right_and_center_alignment_combine_for_cjk_content() {
    let out = BlockStyle::new()
        .width(6)
        .height(7)
        .padding((1, 1))
        .align(Align::Right)
        .align_vertical(VerticalAlign::Center)
        .render("日\nx")
        .into_string();

    assert_eq!(
        out,
        "      \n      \n   日 \n    x \n      \n      \n      "
    );
}

#[test]
fn content_taller_than_fixed_height_expands_instead_of_truncating() {
    let out = BlockStyle::new()
        .width(3)
        .height(2)
        .render("one\ntwo\n三")
        .into_string();

    assert_eq!(out, "one\ntwo\n三 ");
}

#[test]
fn enabled_border_rows_and_margin_are_outside_fixed_height() {
    let out = BlockStyle::new()
        .width(3)
        .height(2)
        .border(Border::ASCII)
        .border_top(false)
        .margin((1, 0, 0, 0))
        .render("x")
        .into_string();

    assert_eq!(out, "     \n|x  |\n|   |\n+---+");
}

#[test]
fn maximum_dimensions_crop_the_final_outer_block() {
    let out = BlockStyle::new()
        .width(4)
        .height(3)
        .padding((1, 1))
        .border(Border::ASCII)
        .margin(1)
        .max_width(6)
        .max_height(4)
        .render("ab")
        .into_string();

    assert_eq!(out, "      \n +----\n |    \n | ab ");
    assert_eq!(out.lines().count(), 4);
    assert!(out.lines().all(|line| visible_width(line) <= 6));
}

#[test]
fn maximum_width_truncates_after_fixed_width_without_rewrapping() {
    let out = BlockStyle::new()
        .width(6)
        .max_width(4)
        .render("ab")
        .into_string();
    assert_eq!(out, "ab  ");

    let max_only = BlockStyle::new()
        .max_width(5)
        .render("hello world")
        .into_string();
    assert_eq!(max_only, "hello");
}

#[test]
fn maximum_height_truncates_after_fixed_height_and_vertical_alignment() {
    let out = BlockStyle::new()
        .width(3)
        .height(5)
        .align_vertical(VerticalAlign::Bottom)
        .max_height(3)
        .render("x")
        .into_string();

    assert_eq!(out, "   \n   \n   ");
}

#[test]
fn maximum_width_crops_between_graphemes_and_keeps_the_block_rectangular() {
    // A wide grapheme that would straddle the bound is dropped rather than
    // split, and the freed cell keeps the row at the cropped width.
    let styled = BlockStyle::new()
        .foreground(Color::RED)
        .max_width(3)
        .render("日本語");
    assert_eq!(styled.as_str(), "\x1b[31m日\x1b[0m ");
    assert_eq!(styled.size().width(), 3);

    assert_eq!(
        BlockStyle::new().max_width(1).render("e\u{301}x").as_str(),
        "e\u{301}"
    );
}

#[test]
fn zero_maximum_dimensions_are_disabled_like_lip_gloss() {
    assert_eq!(
        BlockStyle::new().max_width(0).render("x").into_string(),
        "x"
    );
    assert_eq!(
        BlockStyle::new().max_height(0).render("x").into_string(),
        "x"
    );
}

#[test]
fn a_fixed_width_wraps_before_a_maximum_width_crops() {
    let out = BlockStyle::new()
        .foreground(Color::RED)
        .width(4)
        .max_width(3)
        .render("abcdef")
        .into_string();

    assert_eq!(out, "\x1b[31mabc\x1b[0m\n\x1b[31mef \x1b[0m");
    assert!(out.lines().all(|line| visible_width(line) == 3));
}

#[test]
fn maximum_width_uses_grapheme_width_for_zwj_emoji() {
    let out = BlockStyle::new()
        .width(3)
        .max_width(2)
        .render("👩‍💻x")
        .into_string();

    assert_eq!(out, "👩‍💻");
    assert_eq!(visible_width(&out), 2);
}

#[test]
fn non_binding_maximum_preserves_the_rendered_bytes() {
    let text = "red\ntext";
    let unconstrained = BlockStyle::new().render(text).into_string();

    assert_eq!(
        BlockStyle::new().max_width(99).render(text).into_string(),
        unconstrained
    );
    assert_eq!(
        BlockStyle::new().max_height(99).render(text).into_string(),
        unconstrained
    );
}

#[test]
fn width_only_rendering_is_unchanged_by_the_height_default() {
    assert_eq!(
        BlockStyle::new().width(4).render("a\nb").into_string(),
        BlockStyle::new()
            .width(4)
            .remove(urushi::BlockStylePropertyKey::Height)
            .render("a\nb")
            .into_string()
    );
}

#[test]
fn colors_and_modifiers_emit_sgr() {
    let out = BlockStyle::new()
        .foreground(Color::Ansi256(212))
        .bold()
        .render("hi")
        .into_string();
    assert_eq!(out, "\x1b[1;38;5;212mhi\x1b[0m");
}

#[test]
fn modifiers_can_be_removed_from_a_style_value() {
    let out = BlockStyle::new()
        .bold()
        .remove(Modifier::all())
        .render("plain")
        .into_string();

    assert_eq!(out, "plain");
}

#[test]
fn background_covers_padding() {
    let out = BlockStyle::new()
        .background(Color::BLUE)
        .padding((0, 1))
        .render("x")
        .into_string();
    assert_eq!(out, "\x1b[44m x \x1b[0m");
}

#[test]
fn border_color_is_scoped_to_border() {
    let out = BlockStyle::new()
        .border(Border::NORMAL)
        .border_foreground(Color::RED)
        .render("x")
        .into_string();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "\x1b[31m┌─┐\x1b[0m");
    assert_eq!(lines[1], "\x1b[31m│\x1b[0mx\x1b[31m│\x1b[0m");
    assert_eq!(lines[2], "\x1b[31m└─┘\x1b[0m");
}

#[test]
fn border_sides_render_independently_with_geometric_corners() {
    let cases = [
        (
            BlockStyle::new().border(Border::ASCII),
            "+-+\n|x|\n+-+",
            "all sides",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            "x",
            "no sides",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            "-\nx",
            "top only",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
            "|x",
            "left only",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false),
            "+-\n|x",
            "adjacent sides",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_left(false),
            "-\nx\n-",
            "opposite horizontal sides",
        ),
        (
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_bottom(false),
            "|x|",
            "opposite vertical sides",
        ),
    ];

    for (style, expected, case) in cases {
        let actual = style.render("x").into_string();
        assert_eq!(actual, expected, "{case}");
        let widths: Vec<_> = actual.lines().map(visible_width).collect();
        assert!(
            widths.iter().all(|width| *width == widths[0]),
            "inconsistent widths for {case}: {widths:?}"
        );
    }
}

#[test]
fn enabled_edges_are_the_only_border_cells_styled() {
    let style = BlockStyle::new()
        .border(Border::NORMAL)
        .border_top(false)
        .border_right(false)
        .border_bottom(false)
        .border_foreground(Color::RED);

    assert_eq!(style.render("x").into_string(), "\x1b[31m│\x1b[0mx");

    let no_edges = style.border_left(false);
    assert_eq!(no_edges.render("x").into_string(), "x");
}

#[test]
fn empty_content_keeps_degenerate_border_geometry_consistent() {
    let all_sides = BlockStyle::new()
        .border(Border::ASCII)
        .render("")
        .into_string();
    assert_eq!(all_sides, "++\n||\n++");
    assert!(all_sides.lines().all(|line| visible_width(line) == 2));

    let adjacent = BlockStyle::new()
        .border(Border::ASCII)
        .border_right(false)
        .border_bottom(false)
        .render("")
        .into_string();
    assert_eq!(adjacent, "+\n|");
    assert!(adjacent.lines().all(|line| visible_width(line) == 1));
}

#[test]
fn width_wraps_cjk_text() {
    let out = BlockStyle::new()
        .width(4)
        .render("こんにちは")
        .into_string();
    assert_eq!(out, "こん\nにち\nは  ");
}

#[test]
fn margin_is_unstyled() {
    let out = BlockStyle::new()
        .background(Color::BLUE)
        .margin((0, 0, 0, 2))
        .render("hi")
        .into_string();
    assert_eq!(out, "  \x1b[44mhi\x1b[0m");
}

#[test]
fn block_content_is_plain_text() {
    // The layout pass never inspects text for escape sequences: rendered output
    // handed back as content is measured as ordinary graphemes, so the block
    // comes out deterministically too wide instead of guessing.
    let inner = BlockStyle::new()
        .foreground(Color::RED)
        .render("hi")
        .into_string();
    let out = BlockStyle::new().border(Border::NORMAL).render(&inner);

    assert!(out.size().width() > 4);
    assert_eq!(
        BlockStyle::new()
            .border(Border::NORMAL)
            .render("hi")
            .size()
            .width(),
        4
    );
}

#[test]
fn rendered_block_width_is_consistent() {
    let style = BlockStyle::new()
        .padding(1)
        .margin(1)
        .border(Border::DOUBLE)
        .width(10);
    let out = style
        .render("wrap して しまう ながい ぶんしょう")
        .into_string();
    let widths: Vec<usize> = out.lines().map(visible_width).collect();
    assert!(
        widths.iter().all(|&w| w == widths[0]),
        "all lines should have equal width, got {widths:?}\n{out}"
    );
}
