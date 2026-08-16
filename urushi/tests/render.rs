use urushi::{Align, BlockStyle, Border, Color, Modifier, Overflow, RenderedBlock, VerticalAlign};

/// The cells one rendered row occupies.
///
/// Rendered output is measured through the crate's one ANSI-aware entry point,
/// so a test asserting that a block really is a rectangle measures its rows the
/// same way the block itself was measured.
fn row_width(line: &str) -> usize {
    RenderedBlock::from_ansi(line).size().width()
}

/// The cells an expected-output literal occupies.
///
/// The fixtures compared this way are ASCII or box-drawing text with no escape
/// sequences, so counting characters is counting cells.
fn literal_width(text: &str) -> usize {
    text.lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

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
fn a_size_measures_the_box_the_terminal_shows() {
    // Border edges and padding lie inside the size: a 5x5 box is five visible
    // cells wide and five rows tall, one of each being content.
    let style = BlockStyle::new()
        .width(5)
        .height(5)
        .padding((1, 1))
        .border(Border::ASCII);

    assert_eq!(
        style.render("x").into_string(),
        "+---+\n\
         |   |\n\
         | x |\n\
         |   |\n\
         +---+"
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
fn content_taller_than_the_height_clips_inside_the_frame() {
    let out = BlockStyle::new()
        .width(3)
        .height(2)
        .render("one\ntwo\n三")
        .into_string();

    assert_eq!(out, "one\ntwo");
}

#[test]
fn border_edges_are_inside_the_size_and_margin_is_outside_it() {
    let out = BlockStyle::new()
        .width(3)
        .height(2)
        .border(Border::ASCII)
        .border_top(false)
        .margin((1, 0, 0, 0))
        .render("x")
        .into_string();

    // The box is 3x2: one content cell, the left and right edges, and the
    // bottom edge. The margin row sits outside it.
    assert_eq!(out, "   \n|x|\n+-+");
}

#[test]
fn maximum_dimensions_bound_the_box_without_cutting_its_frame() {
    let out_block = BlockStyle::new()
        .width(8)
        .height(8)
        .padding((1, 1))
        .border(Border::ASCII)
        .margin(1)
        .max_width(6)
        .max_height(6)
        .render("ab");
    let out = out_block.as_str();

    // The maxima won over the stated size, and the frame still closes: a 6x6
    // box inside a one-cell margin.
    assert_eq!(
        out,
        "        \n\
         \x20+----+ \n\
         \x20|    | \n\
         \x20| ab | \n\
         \x20|    | \n\
         \x20|    | \n\
         \x20+----+ \n\
         \x20       "
    );
    assert_eq!(out_block.size().width(), 8);
}

#[test]
fn a_maximum_width_bounds_the_box_and_the_content_reflows_into_it() {
    let out = BlockStyle::new()
        .width(6)
        .max_width(4)
        .render("ab")
        .into_string();
    assert_eq!(out, "ab  ");

    // A maximum with no width is shrink-to-fit with a cap, and the content
    // wraps to the capped width instead of being cut.
    let max_only = BlockStyle::new()
        .max_width(5)
        .render("hello world")
        .into_string();
    assert_eq!(max_only, "hello\nworld");
}

#[test]
fn a_maximum_height_bounds_the_box_before_vertical_alignment_places_content() {
    let out = BlockStyle::new()
        .width(3)
        .height(5)
        .align_vertical(VerticalAlign::Bottom)
        .max_height(3)
        .render("x")
        .into_string();

    // The bound wins over the stated height, and the content is then placed
    // at the bottom of what is left.
    assert_eq!(out, "   \n   \nx  ");
}

#[test]
fn a_bounded_width_wraps_wide_graphemes_and_keeps_the_block_rectangular() {
    // A wide grapheme cannot be split, so it moves to the next row rather
    // than being cut, and every row keeps the box's width.
    let styled = BlockStyle::new()
        .foreground(Color::RED)
        .max_width(3)
        .render("日本語");
    assert_eq!(
        styled.as_str(),
        "\x1b[31m日 \x1b[0m\n\x1b[31m本 \x1b[0m\n\x1b[31m語 \x1b[0m"
    );
    assert_eq!(styled.size().width(), 3);

    assert_eq!(
        BlockStyle::new().max_width(1).render("e\u{301}x").as_str(),
        "e\u{301}\nx"
    );
}

#[test]
fn a_zero_bound_is_a_bound_not_a_disabled_flag() {
    // Zero is a real maximum. On the width axis the grapheme floor stops the
    // box at one cell; the height axis has no such floor, so it empties.
    assert_eq!(
        BlockStyle::new().max_width(0).render("x").into_string(),
        "x"
    );
    assert_eq!(
        BlockStyle::new().max_height(0).render("x").into_string(),
        ""
    );
}

#[test]
fn a_maximum_width_wins_over_a_stated_width_and_the_content_wraps_to_it() {
    let out_block = BlockStyle::new()
        .foreground(Color::RED)
        .width(4)
        .max_width(3)
        .render("abcdef");
    let out = out_block.as_str();

    assert_eq!(out, "\x1b[31mabc\x1b[0m\n\x1b[31mdef\x1b[0m");
    assert_eq!(out_block.size().width(), 3);
}

#[test]
fn a_bounded_width_uses_grapheme_width_for_zwj_emoji() {
    let out_block = BlockStyle::new().width(3).max_width(2).render("👩‍💻x");
    let out = out_block.as_str();

    assert_eq!(out, "👩‍💻\nx ");
    assert_eq!(out_block.size().width(), 2);
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
        let block = style.render("x");
        assert_eq!(block.as_str(), expected, "{case}");
        assert_eq!(block.size().width(), literal_width(expected), "{case}");
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
    let all_sides_block = BlockStyle::new().border(Border::ASCII).render("");
    let all_sides = all_sides_block.as_str();
    assert_eq!(all_sides, "++\n||\n++");
    assert_eq!(all_sides_block.size().width(), 2);

    let adjacent_block = BlockStyle::new()
        .border(Border::ASCII)
        .border_right(false)
        .border_bottom(false)
        .render("");
    let adjacent = adjacent_block.as_str();
    assert_eq!(adjacent, "+\n|");
    assert_eq!(adjacent_block.size().width(), 1);
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
fn rendered_block_width_is_consistent() {
    let style = BlockStyle::new()
        .padding(1)
        .margin(1)
        .border(Border::DOUBLE)
        .width(10);
    let block = style.render("wrap して しまう ながい ぶんしょう");
    assert_eq!(block.size().width(), 12);
    assert!(
        block
            .as_str()
            .lines()
            .all(|line| row_width(line) == block.size().width()),
        "every row should fill the measured width\n{}",
        block.as_str()
    );
}

// --- The sizing model: one box, bounds that reshape it, a frame that closes.

#[test]
fn every_sizing_property_measures_the_same_box() {
    // A bordered box "of width 6" is six visible cells wide.
    let out_block = BlockStyle::new()
        .width(6)
        .border(Border::NORMAL)
        .render("abcdefgh");
    let out = out_block.as_str();

    assert_eq!(out, "┌────┐\n│abcd│\n│efgh│\n└────┘");
    assert_eq!(out_block.size().width(), 6);

    // The bounds measure that same box, so a maximum of six leaves the same
    // rectangle as a width of six.
    assert_eq!(
        BlockStyle::new()
            .max_width(6)
            .border(Border::NORMAL)
            .render("abcdefgh")
            .into_string(),
        out
    );
    // As does a minimum, when the content is narrower than it.
    assert_eq!(
        BlockStyle::new()
            .min_width(6)
            .border(Border::NORMAL)
            .render("ab")
            .into_string(),
        "┌────┐\n│ab  │\n└────┘"
    );
}

#[test]
fn frame_size_reports_border_edges_and_padding_per_axis() {
    let style = BlockStyle::new()
        .border(Border::NORMAL)
        .border_top(false)
        .padding((1, 2));

    // Two side columns plus two padding columns each side; one bottom row
    // plus one padding row each side.
    assert_eq!(style.frame_size().width(), 6);
    assert_eq!(style.frame_size().height(), 3);

    assert_eq!(BlockStyle::new().frame_size().width(), 0);
    // A border glyph set with every edge disabled contributes nothing.
    assert_eq!(
        BlockStyle::new()
            .border(Border::NORMAL)
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_left(false)
            .frame_size()
            .height(),
        0
    );
}

#[test]
fn the_frame_closes_at_every_combination_of_bound_and_content_length() {
    let contents = ["", "a", "hello world", "日本語テキスト", "a\nbb\nccc"];

    for content in contents {
        for width in 3..=12u16 {
            for height in 3..=6u16 {
                let block = BlockStyle::new()
                    .border(Border::ASCII)
                    .width(width)
                    .height(height)
                    .render(content);
                let out = block.as_str();
                let lines: Vec<&str> = out.lines().collect();

                assert_eq!(
                    lines.len(),
                    usize::from(height),
                    "{content:?} at {width}x{height}"
                );
                // The width is the requested one, except where an
                // unsplittable grapheme floors the box wider.
                let resolved = block.size().width();
                assert!(
                    resolved >= usize::from(width),
                    "{content:?} at {width}x{height}: {out}"
                );
                for line in &lines {
                    assert_eq!(
                        row_width(line),
                        resolved,
                        "{content:?} at {width}x{height}: {out}"
                    );
                }
                let closed = |line: &str| {
                    line.starts_with('+') && line.ends_with('+')
                        || line.starts_with('|') && line.ends_with('|')
                };
                assert!(
                    lines.iter().all(|line| closed(line)),
                    "{content:?} at {width}x{height} left the frame open:\n{out}"
                );
            }
        }
    }
}

#[test]
fn overflow_absorbs_the_width_under_the_policy_the_block_chose() {
    let base = BlockStyle::new().max_width(5);

    assert_eq!(
        base.clone().render("hello world").into_string(),
        "hello\nworld",
        "Wrap is the default"
    );
    assert_eq!(
        base.clone()
            .overflow(Overflow::clip())
            .render("hello world")
            .into_string(),
        "hello"
    );
    assert_eq!(
        base.clone()
            .overflow(Overflow::ellipsis())
            .render("hello world")
            .into_string(),
        "hell…"
    );
    // The marker is not one cell by decree: an ASCII marker takes three, and
    // the content keeps what is left.
    assert_eq!(
        base.overflow(Overflow::clip_with("..."))
            .render("hello world")
            .into_string(),
        "he..."
    );
}

#[test]
fn a_marker_the_box_cannot_hold_is_dropped_rather_than_filling_it() {
    // Three cells cannot show a three-cell marker and any content, so the cut
    // goes silent instead.
    assert_eq!(
        BlockStyle::new()
            .max_width(3)
            .overflow(Overflow::clip_with("..."))
            .render("hello")
            .into_string(),
        "hel"
    );
    // A marker exactly as wide as the box is dropped for the same reason.
    assert_eq!(
        BlockStyle::new()
            .max_width(1)
            .overflow(Overflow::ellipsis())
            .render("hello")
            .into_string(),
        "h"
    );
    // A wide marker is measured by its display width, not its length.
    assert_eq!(
        BlockStyle::new()
            .max_width(4)
            .overflow(Overflow::clip_with("→"))
            .render("hello")
            .into_string(),
        "hel→"
    );
}

#[test]
fn overflow_never_opens_the_frame() {
    for overflow in [
        Overflow::Wrap,
        Overflow::clip(),
        Overflow::ellipsis(),
        Overflow::clip_with("..."),
    ] {
        let block = BlockStyle::new()
            .border(Border::ASCII)
            .padding((0, 1))
            .width(8)
            .overflow(overflow.clone())
            .render("abcdefghij");
        let out = block.as_str();

        assert_eq!(block.size().width(), 8, "{overflow:?}: {out}");
        assert!(out.starts_with("+------+"), "{overflow:?}: {out}");
        assert!(out.ends_with("+------+"), "{overflow:?}: {out}");
    }
}

#[test]
fn a_height_clips_its_content_inside_the_frame() {
    let out = BlockStyle::new()
        .border(Border::ASCII)
        .height(3)
        .width(5)
        .render("one\ntwo\nthree")
        .into_string();

    // Three rows: two border edges and one content row.
    assert_eq!(out, "+---+\n|one|\n+---+");
}

#[test]
fn minimum_dimensions_floor_the_box_below_its_content() {
    let out = BlockStyle::new()
        .min_width(6)
        .min_height(3)
        .max_width(2)
        .max_height(1)
        .render("x")
        .into_string();

    // A floor that binds wins over the cap above it.
    assert_eq!(out, "x     \n      \n      ");
}

#[test]
fn wide_graphemes_and_clusters_survive_the_clamp_and_the_clip() {
    // Wrapping never splits a wide character.
    assert_eq!(
        BlockStyle::new()
            .max_width(3)
            .render("日本語")
            .into_string(),
        "日 \n本 \n語 "
    );
    // Clipping stops before one that would straddle the bound.
    assert_eq!(
        BlockStyle::new()
            .max_width(3)
            .overflow(Overflow::clip())
            .render("日本語")
            .into_string(),
        "日 "
    );
    // A cluster is one grapheme on either policy.
    assert_eq!(
        BlockStyle::new()
            .max_width(2)
            .overflow(Overflow::clip())
            .render("👩‍💻x")
            .into_string(),
        "👩‍💻"
    );
    assert_eq!(
        BlockStyle::new()
            .max_width(1)
            .overflow(Overflow::clip())
            .render("e\u{301}x")
            .into_string(),
        "e\u{301}"
    );
}

#[test]
fn the_width_floor_is_the_content_and_the_height_floor_is_the_frame() {
    // A grapheme cannot be cut in half, so the width floors at the frame plus
    // one unsplittable token however small the stated width.
    assert_eq!(
        BlockStyle::new()
            .border(Border::ASCII)
            .width(1)
            .height(3)
            .render("x")
            .into_string(),
        "+-+\n|x|\n+-+"
    );
    // A row can simply be absent, so the height floors at the frame alone: a
    // closed frame around nothing.
    assert_eq!(
        BlockStyle::new()
            .border(Border::ASCII)
            .width(3)
            .height(1)
            .render("x")
            .into_string(),
        "+-+\n+-+"
    );
}

#[test]
fn the_width_is_resolved_before_the_content_and_the_height_after_it() {
    let text = "one two three four";

    // A narrower box is a taller one: the width is settled first, and the
    // reflow it causes is what decides the row count.
    // The box stays at the cap rather than shrinking to the longest wrapped
    // line: that would be a second width decision derived from the content
    // this one produced, and sizes flow down only once.
    let wide = BlockStyle::new().max_width(9).render(text).into_string();
    let narrow = BlockStyle::new().max_width(5).render(text).into_string();
    assert_eq!(wide, "one two  \nthree    \nfour     ");
    assert_eq!(narrow, "one  \ntwo  \nthree\nfour ");

    // The height then clamps against that reflowed count, not the original
    // line count.
    assert_eq!(
        BlockStyle::new()
            .max_width(5)
            .max_height(2)
            .render(text)
            .into_string(),
        "one  \ntwo  "
    );
}
