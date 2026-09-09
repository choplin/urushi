use ratatui::{
    Terminal,
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color as RatatuiColor, Modifier},
    widgets::Widget as _,
};
use urushi::{
    Align, AnsiPolicy, AnsiRenderer, Available, BlockStyle, Border, Color, ColorProfile, Length,
    Modifier as UrushiModifier, Overflow, PanelRole, PrintableText, SemanticTokens, StyledText,
    TerminalProfile, TextSpan, TextStyle, Theme, VerticalAlign, View, measure,
};
use urushi_tui::ratatui::{RatatuiStyle, RatatuiStyleExt as _, ViewWidget};

#[test]
fn one_theme_component_renders_to_plain_cli_and_ratatui() {
    let theme = Theme::from_tokens(tokens());
    let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
    let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));

    let block = panel.render("保存しました");
    assert_eq!(block.size().width(), 16);
    let plain = block.into_string();
    assert!(plain.contains("保存しました"));
    assert!(plain.contains("38;2;80;160;255"));

    let backend = TestBackend::new(16, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| frame.render_widget(panel.widget("保存しました"), frame.area()))
        .expect("draw frame");
    let buffer = terminal.backend().buffer();

    assert_eq!(visual_line(buffer, 0), "╭──────────────╮");
    assert_eq!(visual_line(buffer, 1), "│ 保存しました │");
    assert_eq!(visual_line(buffer, 2), "╰──────────────╯");

    let border = buffer.cell((0, 0)).expect("border cell");
    assert_eq!(border.fg, RatatuiColor::Rgb(80, 160, 255));
    let content = buffer.cell((2, 1)).expect("content cell");
    assert_eq!(content.fg, RatatuiColor::White);
    assert_eq!(content.bg, RatatuiColor::Black);
    assert!(!content.modifier.contains(Modifier::BOLD));
}

#[test]
fn theme_widget_refits_safely_at_a_boundary_size() {
    let theme = Theme::from_tokens(tokens());
    let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
    let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
    let backend = TestBackend::new(6, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    terminal
        .draw(|frame| frame.render_widget(panel.widget("日本語"), frame.area()))
        .expect("draw frame");

    // The area is narrower than the panel, so the box resolves under it and
    // the frame closes inside the area; the ANSI backend given the same area
    // produces the same rectangle.
    let buffer = terminal.backend().buffer();
    assert_eq!(visual_line(buffer, 0), "╭────╮");
    assert_eq!(visual_line(buffer, 1), "│ 日 │");
    assert_eq!(visual_line(buffer, 2), "╰────╯");
}

/// The two backends consume the same `ResolvedView`, so a shared corpus must
/// come out as the same rectangle in both. Each case is drawn into a `Rect` and
/// compared with the ANSI renderer given the `Available` area that `Rect`
/// implies, so any divergence shows up here.
#[test]
fn both_backends_render_the_shared_view_corpus_identically() {
    for (case, view, area) in corpus() {
        let rendered = plain_renderer().render_within(&view, area_of(area));
        // Cells beyond the resolved rectangle belong to the caller's buffer,
        // not to the view, so the comparison is bounded by the resolved size.
        let width = u16::try_from(rendered.size().width()).expect("resolved width");
        let expected: Vec<&str> = rendered.as_str().lines().collect();

        let mut buffer = Buffer::empty(area);
        ViewWidget::new(&view).render(area, &mut buffer);

        assert_eq!(
            rendered.size().height(),
            expected.len(),
            "{case}: row count"
        );
        for (row, expected_line) in expected.into_iter().enumerate() {
            assert_eq!(
                visual_row(&buffer, row as u16, width),
                expected_line,
                "{case}: row {row}",
            );
        }
    }
}

/// A row mixing plain text with a bordered block resolves to one rectangle
/// whose height is the block's, in both backends.
#[test]
fn a_bordered_block_inside_a_row_has_the_same_rectangle_in_both_backends() {
    let view = bordered_block_in_a_row();
    let size = measure(&view);
    assert_eq!(size.height(), 3, "the block's height wins the row");

    let area = Rect::new(0, 0, 12, 3);
    let mut buffer = Buffer::empty(area);
    ViewWidget::new(&view).render(area, &mut buffer);

    let ansi = plain_renderer().render(&view);
    assert_eq!(ansi.size(), size);
    for (row, expected_line) in ansi.as_str().lines().enumerate() {
        assert_eq!(visual_line(&buffer, row as u16), expected_line);
    }
}

/// Styles survive the conversion, not just geometry: the border and the text
/// inside the block keep the colors the view gave them.
#[test]
fn the_ratatui_backend_keeps_the_logical_styles_of_a_resolved_view() {
    let view = bordered_block_in_a_row();
    let area = Rect::new(0, 0, 12, 3);
    let mut buffer = Buffer::empty(area);

    ViewWidget::new(&view).render(area, &mut buffer);

    assert_eq!(
        buffer.cell((8, 0)).expect("border cell").fg,
        RatatuiColor::Rgb(80, 160, 255)
    );
    assert_eq!(
        buffer.cell((9, 1)).expect("block content cell").fg,
        RatatuiColor::Green
    );
    assert_eq!(
        buffer.cell((0, 1)).expect("row text cell").fg,
        RatatuiColor::Reset
    );
}

fn bordered_block_in_a_row() -> View {
    View::row(
        VerticalAlign::Center,
        [
            View::text("status: ", TextStyle::new()),
            View::block(
                BlockStyle::new()
                    .border(Border::ROUNDED)
                    .border_foreground(Color::Rgb(80, 160, 255)),
                View::text("ok", TextStyle::new().foreground(Color::GREEN)),
            ),
        ],
    )
}

/// Views exercising text, blocks, rows, columns, CJK, fixed and maximum
/// dimensions, both alignment biases, and cropping.
fn corpus() -> Vec<(&'static str, View, Rect)> {
    let plain = TextStyle::new();
    let styled = StyledText::try_from_spans([
        TextSpan::new("日本", TextStyle::new().foreground(Color::CYAN)),
        TextSpan::new("👩‍💻 text", TextStyle::new().bold()),
    ])
    .expect("whole grapheme boundaries");
    vec![
        (
            "styled text wraps as one flow",
            View::block(
                BlockStyle::new().border(Border::NORMAL),
                View::styled_text(styled),
            ),
            Rect::new(0, 0, 7, 4),
        ),
        (
            "multiline text",
            View::text("alpha\nbeta gamma", plain.clone()),
            Rect::new(0, 0, 10, 2),
        ),
        (
            "bordered block with padding and CJK",
            View::block(
                BlockStyle::new().border(Border::ROUNDED).padding((0, 1)),
                View::text("保存しました", plain.clone()),
            ),
            Rect::new(0, 0, 16, 3),
        ),
        (
            "bordered block in a row",
            bordered_block_in_a_row(),
            Rect::new(0, 0, 12, 3),
        ),
        (
            "column centering narrower children",
            View::column(
                Align::Center,
                [
                    View::text("wide enough line", plain.clone()),
                    View::text("short", plain.clone()),
                    View::block(
                        BlockStyle::new().border(Border::ASCII),
                        View::text("x", plain.clone()),
                    ),
                ],
            ),
            Rect::new(0, 0, 16, 5),
        ),
        (
            "row centering shorter children, odd row above",
            View::row(
                VerticalAlign::Center,
                [
                    View::text("a", plain.clone()),
                    View::block(
                        BlockStyle::new().height(4).border(Border::ASCII),
                        View::text("tall", plain.clone()),
                    ),
                ],
            ),
            Rect::new(0, 0, 8, 6),
        ),
        (
            "block vertical align centers with the odd row below",
            View::block(
                BlockStyle::new()
                    .width(6)
                    .height(4)
                    .align(Align::Right)
                    .align_vertical(VerticalAlign::Center),
                View::text("x", plain.clone()),
            ),
            Rect::new(0, 0, 6, 4),
        ),
        (
            "a maximum width reflows a wide grapheme",
            View::block(
                BlockStyle::new().max_width(3),
                View::text("日本語", plain.clone()),
            ),
            Rect::new(0, 0, 6, 3),
        ),
        (
            "a narrow rect refits the block",
            View::block(
                BlockStyle::new().border(Border::NORMAL),
                View::text("日本語", plain.clone()),
            ),
            Rect::new(0, 0, 5, 3),
        ),
        (
            "a short rect closes the frame lower",
            View::block(
                BlockStyle::new().border(Border::NORMAL),
                View::text("one\ntwo\nthree", plain.clone()),
            ),
            Rect::new(0, 0, 7, 3),
        ),
        (
            "an ellipsis marks a clipped width",
            View::block(
                BlockStyle::new()
                    .border(Border::NORMAL)
                    .max_width(8)
                    .overflow(Overflow::ellipsis()),
                View::text("hello world", plain.clone()),
            ),
            Rect::new(0, 0, 8, 3),
        ),
        (
            "a clipped width keeps the frame closed",
            View::block(
                BlockStyle::new()
                    .border(Border::NORMAL)
                    .overflow(Overflow::clip()),
                View::text("hello world", plain.clone()),
            ),
            Rect::new(0, 0, 6, 3),
        ),
        (
            "a fill width takes the area",
            View::block(
                BlockStyle::new()
                    .border(Border::NORMAL)
                    .width(Length::fill(1))
                    .align(Align::Center),
                View::text("ab", plain.clone()),
            ),
            Rect::new(0, 0, 10, 3),
        ),
        (
            "a minimum width floors the box",
            View::block(
                BlockStyle::new().border(Border::NORMAL).min_width(8),
                View::text("ab", plain.clone()),
            ),
            Rect::new(0, 0, 10, 3),
        ),
        (
            "a row splits its width between a stated child and a fill",
            View::row(
                VerticalAlign::Top,
                [
                    View::block(
                        BlockStyle::new().border(Border::NORMAL).width(5),
                        View::text("nav", plain.clone()),
                    ),
                    View::block(
                        BlockStyle::new()
                            .border(Border::NORMAL)
                            .width(Length::fill(1)),
                        View::text("main", plain.clone()),
                    ),
                ],
            ),
            Rect::new(0, 0, 14, 3),
        ),
        (
            "a row shrinks past its intrinsic width, fill first",
            View::row(
                VerticalAlign::Top,
                [
                    View::block(
                        BlockStyle::new().border(Border::NORMAL),
                        View::text("abcd", plain.clone()),
                    ),
                    View::block(
                        BlockStyle::new()
                            .border(Border::NORMAL)
                            .width(Length::fill(1)),
                        View::text("wxyz", plain.clone()),
                    ),
                ],
            ),
            Rect::new(0, 0, 8, 4),
        ),
        (
            "a fill stretches across a row's cross axis",
            View::row(
                VerticalAlign::Top,
                [
                    View::block(
                        BlockStyle::new()
                            .border(Border::NORMAL)
                            .width(5)
                            .height(Length::fill(1)),
                        View::text("nav", plain.clone()),
                    ),
                    View::text("x", plain.clone()),
                ],
            ),
            Rect::new(0, 0, 6, 5),
        ),
        (
            "a column divides its height between a fill and its siblings",
            View::column(
                Align::Left,
                [
                    View::text("head", plain.clone()),
                    View::block(
                        BlockStyle::new()
                            .border(Border::NORMAL)
                            .height(Length::fill(1)),
                        View::text("body", plain.clone()),
                    ),
                    View::text("foot", plain.clone()),
                ],
            ),
            Rect::new(0, 0, 6, 6),
        ),
        (
            "a degenerate area cuts the frame only as a last resort",
            View::block(
                BlockStyle::new().border(Border::NORMAL).padding(1),
                View::text("x", plain),
            ),
            Rect::new(0, 0, 2, 3),
        ),
    ]
}

fn area_of(area: Rect) -> Available {
    Available::size(usize::from(area.width), usize::from(area.height))
}

fn plain_renderer() -> AnsiRenderer {
    AnsiRenderer::new(TerminalProfile::new(
        ColorProfile::TrueColor,
        AnsiPolicy::Disabled,
    ))
}

#[test]
fn ratatui_uses_the_same_terminal_profile_degradation_as_plain_output() {
    let theme = Theme::from_tokens(tokens());

    for (color_profile, expected_border) in [
        (ColorProfile::TrueColor, RatatuiColor::Rgb(80, 160, 255)),
        (ColorProfile::Ansi256, RatatuiColor::Indexed(75)),
        (ColorProfile::Ansi16, RatatuiColor::LightCyan),
    ] {
        let profile = TerminalProfile::new(color_profile, AnsiPolicy::Enabled);
        let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
        let buffer = render_panel(&panel);
        assert_eq!(
            buffer.cell((0, 0)).expect("border cell").fg,
            expected_border
        );
        assert!(panel.render("保存しました").as_str().contains('\x1b'));
    }

    for profile in [
        TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled),
        TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled),
    ] {
        let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
        let buffer = render_panel(&panel);
        let border = buffer.cell((0, 0)).expect("border cell");
        let content = buffer.cell((2, 1)).expect("content cell");
        assert_eq!(border.fg, RatatuiColor::Reset);
        assert_eq!(content.fg, RatatuiColor::Reset);
        assert_eq!(content.bg, RatatuiColor::Reset);
        assert!(!panel.render("保存しました").as_str().contains('\x1b'));
    }
}

#[test]
fn ratatui_converts_the_active_modifier_set() {
    let converted = RatatuiStyle::from(
        &TextStyle::new()
            .add(UrushiModifier::BOLD | UrushiModifier::ITALIC)
            .remove(UrushiModifier::ITALIC),
    )
    .into_inner();

    assert_eq!(converted.add_modifier, Modifier::BOLD);
    assert!(converted.sub_modifier.is_empty());
}

fn render_panel(style: &urushi::BlockStyle) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(16, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| frame.render_widget(style.widget("保存しました"), frame.area()))
        .expect("draw frame");
    terminal.backend().buffer().clone()
}

fn tokens() -> SemanticTokens {
    SemanticTokens {
        text: Color::BRIGHT_WHITE,
        text_muted: Color::BRIGHT_BLACK,
        background: Color::BLACK,
        surface: Color::BLACK,
        accent: Color::Rgb(80, 160, 255),
        accent_text: Color::BLACK,
        success: Color::GREEN,
        warning: Color::YELLOW,
        error: Color::RED,
        border: Color::BRIGHT_BLACK,
    }
}

fn visual_line(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    visual_row(buffer, y, buffer.area.width)
}

/// Reads `width` cells of row `y`, joining each cell's symbol.
fn visual_row(buffer: &ratatui::buffer::Buffer, y: u16, width: u16) -> String {
    let mut line = String::new();
    let mut x = buffer.area.left();
    let right = buffer
        .area
        .left()
        .saturating_add(width)
        .min(buffer.area.right());
    while x < right {
        let symbol = buffer.cell((x, y)).expect("cell").symbol();
        line.push_str(symbol);
        let width = PrintableText::new(symbol)
            .width()
            .max(1)
            .min(usize::from(u16::MAX)) as u16;
        x = x.saturating_add(width);
    }
    line
}
