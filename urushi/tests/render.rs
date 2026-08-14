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
fn maximum_dimensions_crop_the_final_outer_block() {
    let out = Style::new()
        .width(4)
        .height(3)
        .padding((1, 1))
        .border(Border::ASCII)
        .margin(1)
        .max_width(6)
        .max_height(4)
        .render("ab");

    assert_eq!(out, "      \n +----\n |    \n | ab ");
    assert_eq!(out.lines().count(), 4);
    assert!(out.lines().all(|line| visible_width(line) <= 6));
}

#[test]
fn maximum_width_truncates_after_fixed_width_without_rewrapping() {
    let out = Style::new().width(6).max_width(4).render("ab");
    assert_eq!(out, "ab  ");

    let max_only = Style::new().max_width(5).render("hello world");
    assert_eq!(max_only, "hello");
}

#[test]
fn maximum_height_truncates_after_fixed_height_and_vertical_alignment() {
    let out = Style::new()
        .width(3)
        .height(5)
        .align_vertical(VerticalAlign::Bottom)
        .max_height(3)
        .render("x");

    assert_eq!(out, "   \n   \n   ");
}

#[test]
fn maximum_width_preserves_ansi_scopes_graphemes_and_cjk_cells() {
    let styled = Style::new()
        .foreground(Color::RED)
        .max_width(3)
        .render("日本語");
    assert_eq!(styled, "\x1b[31m日\x1b[0m");
    assert_eq!(visible_width(&styled), 2);

    assert_eq!(Style::new().max_width(1).render("e\u{301}x"), "e\u{301}");
}

#[test]
fn zero_maximum_dimensions_are_disabled_like_lip_gloss() {
    assert_eq!(Style::new().max_width(0).render("x"), "x");
    assert_eq!(Style::new().max_height(0).render("x"), "x");
}

#[test]
fn maximum_height_preserves_ansi_closures_from_removed_rows() {
    let out = Style::new()
        .max_height(1)
        .render("\x1b[31mred\nhidden\x1b[0m");

    assert_eq!(out, "\x1b[31mred   \x1b[0m");
}

#[test]
fn maximum_dimensions_discard_controls_that_start_in_cropped_content() {
    assert_eq!(Style::new().max_width(2).render("abcdef\x1b[2J"), "ab");
    assert_eq!(
        Style::new().max_height(1).render("safe\n\x1b[31mhidden"),
        "safe  "
    );
}

#[test]
fn fixed_and_maximum_width_preserve_ansi_during_wrap_and_truncate() {
    let out = Style::new()
        .width(4)
        .max_width(3)
        .render("\x1b[31mabcdef\x1b[0m");

    assert_eq!(out, "\x1b[31mabc\x1b[0m\n\x1b[31mef\x1b[0m ");
    assert!(out.lines().all(|line| visible_width(line) <= 3));
}

#[test]
fn fixed_and_maximum_width_keep_control_strings_with_spaces_atomic() {
    let rendered = Style::new()
        .width(2)
        .max_width(1)
        .render("\x1b]8;;https://exa mple.com\x1b\\link\x1b]8;;\x1b\\");

    assert_eq!(
        rendered,
        "\x1b]8;;https://exa mple.com\x1b\\l\x1b]8;;\x1b\\\n\
         \x1b]8;;https://exa mple.com\x1b\\n\x1b]8;;\x1b\\"
    );
}

#[test]
fn maximum_width_uses_grapheme_width_for_zwj_emoji() {
    let out = Style::new().width(3).max_width(2).render("👩‍💻x");

    assert_eq!(out, "👩‍💻");
    assert_eq!(visible_width(&out), 2);
}

#[test]
fn maximum_width_keeps_ansi_embedded_inside_a_grapheme() {
    let out = Style::new().max_width(1).render("e\x1b[31m\u{301}x");

    assert_eq!(out, "e\x1b[31m\u{301}\x1b[0m");
    assert_eq!(visible_width(&out), 1);
}

#[test]
fn non_binding_maximum_preserves_ansi_bytes() {
    let text = "\x1b[31mred\ntext\x1b[0m";
    let unconstrained = Style::new().render(text);

    assert_eq!(Style::new().max_width(99).render(text), unconstrained);
    assert_eq!(Style::new().max_height(99).render(text), unconstrained);
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
