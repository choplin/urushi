use urushi::{Align, Border, Color, Modifier, Style, visible_width};

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
