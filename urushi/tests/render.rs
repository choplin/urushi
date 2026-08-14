use urushi::{Align, Border, Color, Modifier, Style, VerticalAlign, visible_width};

#[test]
fn plain_text_passes_through() {
    assert_eq!(Style::new().render("hello"), "hello");
    assert_eq!(Style::new().render("a\nb"), "a\nb");
}

#[test]
fn padding_and_border() {
    let style = Style::new().padding((0, 1)).border(Border::ROUNDED);
    assert_eq!(
        style.render("Hello"),
        "╭───────╮\n\
         │ Hello │\n\
         ╰───────╯"
    );
}

#[test]
fn cjk_content_keeps_border_aligned() {
    let style = Style::new().border(Border::NORMAL);
    assert_eq!(
        style.render("日本語"),
        "┌──────┐\n\
         │日本語│\n\
         └──────┘"
    );
}

#[test]
fn mixed_width_lines_align() {
    let out = Style::new().border(Border::ASCII).render("ab\nあい");
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
    let out = Style::new().width(11).align(Align::Center).render("abc");
    assert_eq!(out, "    abc    ");
}

#[test]
fn align_right_with_fixed_width() {
    let out = Style::new().width(5).align(Align::Right).render("ab");
    assert_eq!(out, "   ab");
}

#[test]
fn fixed_height_expands_the_padded_content_box() {
    let style = Style::new()
        .width(5)
        .height(5)
        .padding((1, 1))
        .border(Border::ASCII);

    assert_eq!(
        style.render("x"),
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
    let style = Style::new().width(4).height(3);

    assert_eq!(style.render(""), "    \n    \n    ");
    assert_eq!(style.render("a\nb"), "a   \nb   \n    ");
    assert_eq!(style.render("日本"), "日本\n    \n    ");
}

#[test]
fn fixed_height_aligns_content_vertically_inside_padding() {
    let base = Style::new().width(4).height(6).padding((1, 0));

    assert_eq!(
        base.clone().align_vertical(VerticalAlign::Top).render("x"),
        "    \nx   \n    \n    \n    \n    "
    );
    assert_eq!(
        base.clone()
            .align_vertical(VerticalAlign::Center)
            .render("x"),
        "    \n    \nx   \n    \n    \n    "
    );
    assert_eq!(
        base.align_vertical(VerticalAlign::Bottom).render("x"),
        "    \n    \n    \n    \nx   \n    "
    );
}

#[test]
fn centered_vertical_alignment_puts_an_odd_extra_row_below_like_lip_gloss() {
    let out = Style::new()
        .width(6)
        .height(7)
        .padding((1, 1))
        .align(Align::Center)
        .align_vertical(VerticalAlign::Center)
        .render("日\nx");

    assert_eq!(
        out,
        "      \n      \n  日  \n  x   \n      \n      \n      "
    );
}

#[test]
fn right_and_center_alignment_combine_for_cjk_content() {
    let out = Style::new()
        .width(6)
        .height(7)
        .padding((1, 1))
        .align(Align::Right)
        .align_vertical(VerticalAlign::Center)
        .render("日\nx");

    assert_eq!(
        out,
        "      \n      \n   日 \n    x \n      \n      \n      "
    );
}

#[test]
fn content_taller_than_fixed_height_expands_instead_of_truncating() {
    let out = Style::new().width(3).height(2).render("one\ntwo\n三");

    assert_eq!(out, "one\ntwo\n三 ");
}

#[test]
fn enabled_border_rows_and_margin_are_outside_fixed_height() {
    let out = Style::new()
        .width(3)
        .height(2)
        .border(Border::ASCII)
        .border_top(false)
        .margin((1, 0, 0, 0))
        .render("x");

    assert_eq!(out, "     \n|x  |\n|   |\n+---+");
}

#[test]
fn width_only_rendering_is_unchanged_by_the_height_default() {
    assert_eq!(
        Style::new().width(4).render("a\nb"),
        Style::new()
            .width(4)
            .remove(urushi::StylePropertyKey::Height)
            .render("a\nb")
    );
}

#[test]
fn colors_and_modifiers_emit_sgr() {
    let out = Style::new()
        .foreground(Color::Ansi256(212))
        .bold()
        .render("hi");
    assert_eq!(out, "\x1b[1;38;5;212mhi\x1b[0m");
}

#[test]
fn modifiers_can_be_removed_from_a_style_value() {
    let out = Style::new().bold().remove(Modifier::all()).render("plain");

    assert_eq!(out, "plain");
}

#[test]
fn background_covers_padding() {
    let out = Style::new()
        .background(Color::BLUE)
        .padding((0, 1))
        .render("x");
    assert_eq!(out, "\x1b[44m x \x1b[0m");
}

#[test]
fn border_color_is_scoped_to_border() {
    let out = Style::new()
        .border(Border::NORMAL)
        .border_foreground(Color::RED)
        .render("x");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "\x1b[31m┌─┐\x1b[0m");
    assert_eq!(lines[1], "\x1b[31m│\x1b[0mx\x1b[31m│\x1b[0m");
    assert_eq!(lines[2], "\x1b[31m└─┘\x1b[0m");
}

#[test]
fn border_sides_render_independently_with_geometric_corners() {
    let cases = [
        (
            Style::new().border(Border::ASCII),
            "+-+\n|x|\n+-+",
            "all sides",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            "x",
            "no sides",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            "-\nx",
            "top only",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
            "|x",
            "left only",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false),
            "+-\n|x",
            "adjacent sides",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_left(false),
            "-\nx\n-",
            "opposite horizontal sides",
        ),
        (
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_bottom(false),
            "|x|",
            "opposite vertical sides",
        ),
    ];

    for (style, expected, case) in cases {
        let actual = style.render("x");
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
    let style = Style::new()
        .border(Border::NORMAL)
        .border_top(false)
        .border_right(false)
        .border_bottom(false)
        .border_foreground(Color::RED);

    assert_eq!(style.render("x"), "\x1b[31m│\x1b[0mx");

    let no_edges = style.border_left(false);
    assert_eq!(no_edges.render("x"), "x");
}

#[test]
fn empty_content_keeps_degenerate_border_geometry_consistent() {
    let all_sides = Style::new().border(Border::ASCII).render("");
    assert_eq!(all_sides, "++\n||\n++");
    assert!(all_sides.lines().all(|line| visible_width(line) == 2));

    let adjacent = Style::new()
        .border(Border::ASCII)
        .border_right(false)
        .border_bottom(false)
        .render("");
    assert_eq!(adjacent, "+\n|");
    assert!(adjacent.lines().all(|line| visible_width(line) == 1));
}

#[test]
fn width_wraps_cjk_text() {
    let out = Style::new().width(4).render("こんにちは");
    assert_eq!(out, "こん\nにち\nは  ");
}

#[test]
fn margin_is_unstyled() {
    let out = Style::new()
        .background(Color::BLUE)
        .margin((0, 0, 0, 2))
        .render("hi");
    assert_eq!(out, "  \x1b[44mhi\x1b[0m");
}

#[test]
fn styled_content_is_measured_by_visible_width() {
    let inner = Style::new().foreground(Color::RED).render("hi");
    let out = Style::new().border(Border::NORMAL).render(&inner);
    assert_eq!(out.lines().next().unwrap(), "┌──┐");
}

#[test]
fn nested_style_restores_outer_style_after_inner_reset() {
    let inner = Style::new().foreground(Color::RED).render("hi");
    let out = Style::new()
        .background(Color::BLUE)
        .bold()
        .underline()
        .padding((0, 1))
        .width(8)
        .border(Border::NORMAL)
        .render(&inner);

    assert_eq!(
        out,
        "┌────────┐\n\
         │\x1b[1;4;44m \x1b[31mhi\x1b[0m\x1b[1;4;44m     \x1b[0m│\n\
         └────────┘"
    );
    assert!(out.lines().all(|line| visible_width(line) == 10));
}

#[test]
fn nested_style_preserves_osc_hyperlinks() {
    let link = "\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\";
    let inner = Style::new().foreground(Color::RED).render(link);
    let out = Style::new()
        .background(Color::BLUE)
        .bold()
        .padding((0, 1))
        .render(&inner);

    assert_eq!(
        out,
        "\x1b[1;44m \x1b[31m\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\x1b[0m\x1b[1;44m \x1b[0m"
    );
    assert_eq!(visible_width(&out), 6);
}

#[test]
fn rendered_block_width_is_consistent() {
    let style = Style::new()
        .padding(1)
        .margin(1)
        .border(Border::DOUBLE)
        .width(10);
    let out = style.render("wrap して しまう ながい ぶんしょう");
    let widths: Vec<usize> = out.lines().map(visible_width).collect();
    assert!(
        widths.iter().all(|&w| w == widths[0]),
        "all lines should have equal width, got {widths:?}\n{out}"
    );
}
