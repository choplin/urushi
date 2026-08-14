//! Ratatui widget rendering for urushi's box model.

use ratatui::{buffer::Buffer, layout::Rect, style::Style as InnerStyle, widgets::Widget};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{RatatuiStyle, style::convert_color};
use urushi::{Align, Border, Color, Sides, Style, VerticalAlign, visible_width, wrap_text};

/// A stateless ratatui widget backed by an urushi [`Style`].
///
/// The widget draws into the buffer passed to [`Widget::render`]. It does not
/// initialize a terminal, read events, or own terminal I/O. Content is plain
/// text; ANSI escape sequences are not interpreted inside a ratatui buffer.
#[derive(Debug, Clone, Copy)]
pub struct RatatuiWidget<'a> {
    content: &'a str,
    style: &'a Style,
}

impl<'a> RatatuiWidget<'a> {
    /// Creates a widget that renders `content` with `style`.
    pub const fn new(content: &'a str, style: &'a Style) -> Self {
        Self { content, style }
    }
}

/// Extension methods for adapting an Urushi style to Ratatui.
pub trait RatatuiStyleExt {
    /// Adapts this style and `content` into a stateless Ratatui widget.
    fn widget<'a>(&'a self, content: &'a str) -> RatatuiWidget<'a>;
}

impl RatatuiStyleExt for Style {
    fn widget<'a>(&'a self, content: &'a str) -> RatatuiWidget<'a> {
        RatatuiWidget::new(content, self)
    }
}

#[derive(Debug, Clone, Copy)]
struct BoxParts {
    padding: Sides,
    margin: Sides,
    border: Option<Border>,
    border_top: bool,
    border_right: bool,
    border_bottom: bool,
    border_left: bool,
    border_foreground: Option<Color>,
    border_background: Option<Color>,
    width: Option<u16>,
    height: Option<u16>,
    align: Align,
    vertical_align: VerticalAlign,
}

impl BoxParts {
    const fn from_style(style: &Style) -> Self {
        Self {
            padding: style.padding_sides(),
            margin: style.margin_sides(),
            border: style.border_kind(),
            border_top: style.is_border_top_enabled(),
            border_right: style.is_border_right_enabled(),
            border_bottom: style.is_border_bottom_enabled(),
            border_left: style.is_border_left_enabled(),
            border_foreground: style.border_foreground_color(),
            border_background: style.border_background_color(),
            width: style.fixed_width(),
            height: style.fixed_height(),
            align: style.horizontal_alignment(),
            vertical_align: style.vertical_alignment(),
        }
    }
}

impl Widget for RatatuiWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        render_widget(self, area, buffer);
    }
}

impl Widget for &RatatuiWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        render_widget(*self, area, buffer);
    }
}

fn render_widget(widget: RatatuiWidget<'_>, area: Rect, buffer: &mut Buffer) {
    let maximum_area = Rect::new(
        area.x,
        area.y,
        parts_limit(area.width, widget.style.maximum_width()),
        parts_limit(area.height, widget.style.maximum_height()),
    );
    let clip = maximum_area.intersection(buffer.area);
    render_widget_in_area(widget, area, clip, true, buffer);
}

fn render_widget_in_area(
    widget: RatatuiWidget<'_>,
    area: Rect,
    clip: Rect,
    preserve_legacy_layout: bool,
    buffer: &mut Buffer,
) {
    let parts = BoxParts::from_style(widget.style);
    let Some(available) = inset(area, parts.margin) else {
        return;
    };

    let has_border = parts.border.is_some();
    let border_columns = u16::from(has_border && parts.border_left)
        .saturating_add(u16::from(has_border && parts.border_right));
    let border_rows = u16::from(has_border && parts.border_top)
        .saturating_add(u16::from(has_border && parts.border_bottom));
    let max_box_width = available.width.saturating_sub(border_columns);
    let max_box_height = available.height.saturating_sub(border_rows);

    let horizontal_padding = parts.padding.left.saturating_add(parts.padding.right);
    let natural_inner_width = widget
        .content
        .lines()
        .map(visible_width)
        .max()
        .unwrap_or(0)
        .min(usize::from(u16::MAX)) as u16;
    let requested_box_width = parts
        .width
        .unwrap_or_else(|| natural_inner_width.saturating_add(horizontal_padding));
    let mut box_width = requested_box_width.min(max_box_width);
    let initial_inner_width = box_width.saturating_sub(horizontal_padding);

    let lines = if parts.width.is_some() && (initial_inner_width > 0 || parts.height.is_some()) {
        wrap_text(widget.content, usize::from(initial_inner_width.max(1)))
    } else {
        let mut lines: Vec<String> = widget.content.lines().map(str::to_owned).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    };

    // For fixed-height boxes, match direct rendering when one displayed
    // character is wider than the requested inner width: preserve the
    // character and expand when the area can accommodate it. The height guard
    // keeps width-only rendering on its existing resolution path.
    if parts.height.is_some() {
        let rendered_inner_width = lines
            .iter()
            .map(|line| visible_width(line))
            .max()
            .unwrap_or(0)
            .min(usize::from(u16::MAX)) as u16;
        let rendered_box_width = rendered_inner_width.saturating_add(horizontal_padding);
        box_width = box_width.max(rendered_box_width).min(max_box_width);
    }

    let content_height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let natural_box_height = parts
        .padding
        .top
        .saturating_add(content_height)
        .saturating_add(parts.padding.bottom);
    let requested_box_height = parts
        .height
        .map_or(natural_box_height, |height| height.max(natural_box_height));
    let box_height = requested_box_height.min(max_box_height);
    let total_width = box_width.saturating_add(border_columns);
    let total_height = box_height.saturating_add(border_rows);
    let widget_area = Rect::new(available.x, available.y, total_width, total_height);

    let text_style = RatatuiStyle::from(widget.style).into_inner();
    let box_area = Rect::new(
        widget_area
            .x
            .saturating_add(u16::from(has_border && parts.border_left)),
        widget_area
            .y
            .saturating_add(u16::from(has_border && parts.border_top)),
        box_width,
        box_height,
    );
    let outer_right = widget_area
        .right()
        .saturating_add(parts.margin.right)
        .min(area.right());
    let outer_bottom = widget_area
        .bottom()
        .saturating_add(parts.margin.bottom)
        .min(area.bottom());
    let maximum_width_is_binding = widget
        .style
        .maximum_width()
        .is_some_and(|maximum| maximum > 0 && area.x.saturating_add(maximum) < outer_right);
    let maximum_height_is_binding = widget
        .style
        .maximum_height()
        .is_some_and(|maximum| maximum > 0 && area.y.saturating_add(maximum) < outer_bottom);
    if preserve_legacy_layout && !maximum_width_is_binding && !maximum_height_is_binding {
        let legacy_area = area.intersection(buffer.area);
        return render_widget_in_area(widget, legacy_area, legacy_area, false, buffer);
    }
    if clip.is_empty() {
        return;
    }

    let maximum_width_crops_box = maximum_width_is_binding && clip.right() < box_area.right();
    if !maximum_width_crops_box {
        fill(box_area.intersection(clip), text_style, buffer);
    }
    if let Some(border) = parts.border {
        draw_border(
            widget_area,
            available.intersection(clip),
            border,
            parts,
            border_style(parts),
            buffer,
        );
    }

    let content_x = box_area
        .x
        .saturating_add(parts.padding.left.min(box_area.width));
    let content_y = box_area
        .y
        .saturating_add(parts.padding.top.min(box_area.height));
    let content_width = box_area
        .width
        .saturating_sub(parts.padding.left)
        .saturating_sub(parts.padding.right);
    let content_rows = box_area
        .height
        .saturating_sub(parts.padding.top)
        .saturating_sub(parts.padding.bottom);
    let rendered_content_height = content_height.min(content_rows);
    let vertical_gap = content_rows.saturating_sub(rendered_content_height);
    let vertical_offset = match parts.vertical_align {
        VerticalAlign::Top => 0,
        VerticalAlign::Center => vertical_gap / 2,
        VerticalAlign::Bottom => vertical_gap,
    };
    let first_content_row = content_y.saturating_add(vertical_offset);
    let last_content_row = first_content_row.saturating_add(rendered_content_height);

    for y in box_area.top()..box_area.bottom() {
        if y < first_content_row || y >= last_content_row {
            fill(
                Rect::new(box_area.x, y, box_area.width, 1).intersection(clip),
                text_style,
                buffer,
            );
        }
    }

    for (offset, line) in lines.iter().take(usize::from(content_rows)).enumerate() {
        let line_width = visible_width(line).min(usize::from(u16::MAX)) as u16;
        let gap = content_width.saturating_sub(line_width);
        let alignment_offset = match parts.align {
            Align::Left => 0,
            Align::Center => gap / 2,
            Align::Right => gap,
        };
        let y = content_y
            .saturating_add(vertical_offset)
            .saturating_add(offset as u16);
        let x = content_x.saturating_add(alignment_offset);
        if y >= clip.top() && y < clip.bottom() {
            fill(
                Rect::new(box_area.x, y, x.saturating_sub(box_area.x), 1).intersection(clip),
                text_style,
                buffer,
            );
        }
        if y >= clip.top() && y < clip.bottom() {
            let available_cells = content_width.saturating_sub(alignment_offset);
            draw_line_clipped(line, x, y, available_cells, clip, text_style, buffer);
            if line_width <= available_cells {
                let end_x = x.saturating_add(line_width);
                fill(
                    Rect::new(end_x, y, box_area.right().saturating_sub(end_x), 1)
                        .intersection(clip),
                    text_style,
                    buffer,
                );
            }
        }
    }
}

fn parts_limit(available: u16, maximum: Option<u16>) -> u16 {
    match maximum {
        Some(maximum) if maximum > 0 => available.min(maximum),
        None => available,
        Some(_) => available,
    }
}

fn inset(area: Rect, sides: Sides) -> Option<Rect> {
    let width = area
        .width
        .saturating_sub(sides.left)
        .saturating_sub(sides.right);
    let height = area
        .height
        .saturating_sub(sides.top)
        .saturating_sub(sides.bottom);
    (width > 0 && height > 0).then(|| {
        Rect::new(
            area.x.saturating_add(sides.left.min(area.width)),
            area.y.saturating_add(sides.top.min(area.height)),
            width,
            height,
        )
    })
}

fn fill(area: Rect, style: InnerStyle, buffer: &mut Buffer) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buffer.cell_mut((x, y)) {
                cell.set_symbol(" ").set_style(style);
            }
        }
    }
}

fn draw_line_clipped(
    line: &str,
    x: u16,
    y: u16,
    available_cells: u16,
    clip: Rect,
    style: InnerStyle,
    buffer: &mut Buffer,
) {
    let content_right = x.saturating_add(available_cells);
    let mut cursor = x;

    for grapheme in line.graphemes(true) {
        let width = UnicodeWidthStr::width(grapheme).min(usize::from(u16::MAX)) as u16;
        let next = cursor.saturating_add(width);
        if next > content_right {
            break;
        }
        if cursor >= clip.left() && next <= clip.right() {
            buffer.set_stringn(cursor, y, grapheme, usize::from(width), style);
        }
        cursor = next;
    }
}

fn border_style(parts: BoxParts) -> InnerStyle {
    let mut style = InnerStyle::new();
    if let Some(color) = parts.border_foreground {
        style = style.fg(convert_color(color));
    }
    if let Some(color) = parts.border_background {
        style = style.bg(convert_color(color));
    }
    style
}

fn draw_border(
    area: Rect,
    clip: Rect,
    border: Border,
    parts: BoxParts,
    style: InnerStyle,
    buffer: &mut Buffer,
) {
    if area.is_empty() {
        return;
    }

    let left = area.left();
    let content_left = left.saturating_add(u16::from(parts.border_left));
    let content_width = area
        .width
        .saturating_sub(u16::from(parts.border_left))
        .saturating_sub(u16::from(parts.border_right));
    let right = content_left.saturating_add(content_width);
    let top = area.top();
    let content_top = top.saturating_add(u16::from(parts.border_top));
    let content_height = area
        .height
        .saturating_sub(u16::from(parts.border_top))
        .saturating_sub(u16::from(parts.border_bottom));
    let bottom = content_top.saturating_add(content_height);

    if parts.border_top {
        if parts.border_left {
            set_char_clipped(buffer, clip, left, top, border.top_left, style);
        }
        for x in content_left..content_left.saturating_add(content_width) {
            set_char_clipped(buffer, clip, x, top, border.top, style);
        }
        if parts.border_right {
            set_char_clipped(buffer, clip, right, top, border.top_right, style);
        }
    }

    for y in content_top..content_top.saturating_add(content_height) {
        if parts.border_left {
            set_char_clipped(buffer, clip, left, y, border.left, style);
        }
        if parts.border_right {
            set_char_clipped(buffer, clip, right, y, border.right, style);
        }
    }

    if parts.border_bottom {
        if parts.border_left {
            set_char_clipped(buffer, clip, left, bottom, border.bottom_left, style);
        }
        for x in content_left..content_left.saturating_add(content_width) {
            set_char_clipped(buffer, clip, x, bottom, border.bottom, style);
        }
        if parts.border_right {
            set_char_clipped(buffer, clip, right, bottom, border.bottom_right, style);
        }
    }
}

fn set_char_clipped(
    buffer: &mut Buffer,
    clip: Rect,
    x: u16,
    y: u16,
    value: char,
    style: InnerStyle,
) {
    if x >= clip.left() && x < clip.right() && y >= clip.top() && y < clip.bottom() {
        set_char(buffer, x, y, value, style);
    }
}

fn set_char(buffer: &mut Buffer, x: u16, y: u16, value: char, style: InnerStyle) {
    if let Some(cell) = buffer.cell_mut((x, y)) {
        cell.set_char(value).set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color as RatatuiColor, Modifier};

    use super::*;

    #[test]
    fn converts_colors_and_every_modifier() {
        let converted = RatatuiStyle::from(
            &Style::new()
                .foreground(Color::Rgb(1, 2, 3))
                .background(Color::Ansi256(212))
                .bold()
                .dim()
                .italic()
                .underline()
                .blink()
                .reverse()
                .strikethrough(),
        )
        .into_inner();

        assert_eq!(converted.fg, Some(RatatuiColor::Rgb(1, 2, 3)));
        assert_eq!(converted.bg, Some(RatatuiColor::Indexed(212)));
        assert_eq!(
            converted.add_modifier,
            Modifier::BOLD
                | Modifier::DIM
                | Modifier::ITALIC
                | Modifier::UNDERLINED
                | Modifier::SLOW_BLINK
                | Modifier::REVERSED
                | Modifier::CROSSED_OUT
        );
    }

    #[test]
    fn preserves_border_colors_for_composed_ratatui_widgets() {
        let converted = RatatuiStyle::from(
            &Style::new()
                .border_foreground(Color::Rgb(10, 20, 30))
                .border_background(Color::Ansi256(236)),
        );

        assert_eq!(converted.into_inner(), InnerStyle::new());
        assert_eq!(
            converted.border_style(),
            InnerStyle::new()
                .fg(RatatuiColor::Rgb(10, 20, 30))
                .bg(RatatuiColor::Indexed(236))
        );
    }

    #[test]
    fn maps_sixteen_color_palette_to_named_colors() {
        let expected = [
            RatatuiColor::Black,
            RatatuiColor::Red,
            RatatuiColor::Green,
            RatatuiColor::Yellow,
            RatatuiColor::Blue,
            RatatuiColor::Magenta,
            RatatuiColor::Cyan,
            RatatuiColor::Gray,
            RatatuiColor::DarkGray,
            RatatuiColor::LightRed,
            RatatuiColor::LightGreen,
            RatatuiColor::LightYellow,
            RatatuiColor::LightBlue,
            RatatuiColor::LightMagenta,
            RatatuiColor::LightCyan,
            RatatuiColor::White,
        ];

        for (index, expected) in expected.into_iter().enumerate() {
            let converted =
                RatatuiStyle::from(&Style::new().foreground(Color::Ansi(index as u8))).into_inner();
            assert_eq!(converted.fg, Some(expected));
        }
    }

    #[test]
    fn box_model_properties_are_not_in_the_subset() {
        let plain = RatatuiStyle::from(&Style::new()).into_inner();
        let boxed = RatatuiStyle::from(
            &Style::new()
                .padding(1)
                .margin(1)
                .border(Border::ROUNDED)
                .border_foreground(Color::RED)
                .width(20)
                .height(10)
                .max_width(18)
                .max_height(8)
                .align(Align::Center)
                .align_vertical(VerticalAlign::Center),
        )
        .into_inner();

        assert_eq!(boxed, plain);
    }

    #[test]
    fn widget_preserves_box_model_alignment_and_cjk_width() {
        let style = Style::new()
            .foreground(Color::GREEN)
            .background(Color::BLUE)
            .padding((0, 1))
            .margin((1, 2))
            .border(Border::ROUNDED)
            .border_foreground(Color::RED)
            .width(8)
            .align(Align::Center);
        let area = Rect::new(0, 0, 14, 6);
        let mut buffer = Buffer::empty(area);

        style.widget("日本").render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "              ");
        assert_eq!(buffer_line(&buffer, 1), "  ╭────────╮  ");
        assert_eq!(buffer_line(&buffer, 2), "  │  日本  │  ");
        assert_eq!(buffer_line(&buffer, 3), "  ╰────────╯  ");
        let content = buffer.cell((5, 2)).expect("content cell");
        assert_eq!(content.fg, RatatuiColor::Green);
        assert_eq!(content.bg, RatatuiColor::Blue);
        let border = buffer.cell((2, 1)).expect("border cell");
        assert_eq!(border.fg, RatatuiColor::Red);
    }

    #[test]
    fn widget_clips_wide_content_without_splitting_a_grapheme() {
        let area = Rect::new(0, 0, 5, 3);
        let mut buffer = Buffer::empty(area);

        Style::new()
            .border(Border::NORMAL)
            .widget("日本語")
            .render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "┌───┐");
        assert_eq!(buffer_line(&buffer, 1), "│日 │");
        assert_eq!(buffer_line(&buffer, 2), "└───┘");
    }

    #[test]
    fn widget_is_safe_for_zero_and_narrow_areas() {
        let style = Style::new().border(Border::NORMAL).padding(2);
        let mut empty = Buffer::empty(Rect::new(0, 0, 0, 0));
        style
            .widget("content")
            .render(Rect::new(0, 0, 0, 0), &mut empty);

        let area = Rect::new(0, 0, 1, 1);
        let mut narrow = Buffer::empty(area);
        style.widget("content").render(area, &mut narrow);
        assert_eq!(buffer_line(&narrow, 0), "┌");
    }

    #[test]
    fn widget_matches_direct_rendering_for_border_side_combinations() {
        let styles = [
            Style::new().border(Border::ASCII),
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false),
            Style::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_left(false),
            Style::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_bottom(false),
        ];

        for style in styles {
            let direct = style.render("x");
            let expected: Vec<_> = direct.lines().collect();
            let width = expected
                .iter()
                .map(|line| visible_width(line))
                .max()
                .unwrap() as u16;
            let area = Rect::new(0, 0, width, expected.len() as u16);
            let mut buffer = Buffer::empty(area);

            style.widget("x").render(area, &mut buffer);

            for (y, expected_line) in expected.into_iter().enumerate() {
                assert_eq!(buffer_line(&buffer, y as u16), expected_line);
            }
        }
    }

    #[test]
    fn widget_matches_direct_rendering_for_fixed_height_content_cases() {
        let cases = [
            ("empty", ""),
            ("single line", "x"),
            ("multiple lines", "x\ny"),
            ("CJK", "日本"),
            ("overflow", "a\nb\nc\nd\ne"),
        ];

        for (case, content) in cases {
            let style = Style::new()
                .width(6)
                .height(4)
                .padding((1, 1))
                .border(Border::ASCII)
                .border_top(false);
            let direct = style.render(content);
            let expected: Vec<_> = direct.lines().collect();
            let width = expected
                .iter()
                .map(|line| visible_width(line))
                .max()
                .unwrap() as u16;
            let area = Rect::new(0, 0, width, expected.len() as u16);
            let mut buffer = Buffer::empty(area);

            style.widget(content).render(area, &mut buffer);

            for (y, expected_line) in expected.into_iter().enumerate() {
                assert_eq!(buffer_line(&buffer, y as u16), expected_line, "{case}");
            }
        }
    }

    #[test]
    fn widget_matches_direct_vertical_alignment_with_horizontal_alignment_and_cjk() {
        for horizontal in [Align::Left, Align::Center, Align::Right] {
            for vertical in [
                VerticalAlign::Top,
                VerticalAlign::Center,
                VerticalAlign::Bottom,
            ] {
                let content = "日\nx";
                let style = Style::new()
                    .width(6)
                    .height(7)
                    .padding((1, 1))
                    .align(horizontal)
                    .align_vertical(vertical);
                let direct = style.render(content);
                let expected: Vec<_> = direct.lines().collect();
                let area = Rect::new(0, 0, 6, 7);
                let mut buffer = Buffer::empty(area);

                style.widget(content).render(area, &mut buffer);

                for (y, expected_line) in expected.into_iter().enumerate() {
                    assert_eq!(
                        buffer_line(&buffer, y as u16),
                        expected_line,
                        "{horizontal:?} {vertical:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn constrained_widget_realigns_center_and_bottom_within_the_available_height() {
        let area = Rect::new(0, 0, 4, 4);

        for (align, expected_y) in [(VerticalAlign::Center, 1), (VerticalAlign::Bottom, 3)] {
            let style = Style::new().width(4).height(6).align_vertical(align);
            let mut buffer = Buffer::empty(area);

            style.widget("x").render(area, &mut buffer);

            for y in 0..area.height {
                let expected = if y == expected_y { "x   " } else { "    " };
                assert_eq!(buffer_line(&buffer, y), expected, "{align:?} row {y}");
            }
        }
    }

    #[test]
    fn widget_expands_narrow_fixed_width_for_cjk_parity() {
        let style = Style::new().width(1).height(2);
        let direct = style.render("日本");
        let area = Rect::new(0, 0, 2, 2);
        let mut buffer = Buffer::empty(area);

        style.widget("日本").render(area, &mut buffer);

        assert_eq!(direct, "日\n本");
        assert_eq!(buffer_line(&buffer, 0), "日");
        assert_eq!(buffer_line(&buffer, 1), "本");
    }

    #[test]
    fn widget_preserves_narrow_width_only_clipping() {
        let style = Style::new().background(Color::BLUE).width(1);
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);

        style.widget("日").render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "  ");
        assert_eq!(
            buffer.cell((0, 0)).expect("clipped content cell").bg,
            RatatuiColor::Blue
        );
    }

    #[test]
    fn widget_clips_fixed_height_to_the_available_area() {
        let style = Style::new().width(4).height(6).border(Border::ASCII);
        let area = Rect::new(0, 0, 6, 4);
        let mut buffer = Buffer::empty(area);

        style.widget("x").render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "+----+");
        assert_eq!(buffer_line(&buffer, 1), "|x   |");
        assert_eq!(buffer_line(&buffer, 2), "|    |");
        assert_eq!(buffer_line(&buffer, 3), "+----+");
    }

    #[test]
    fn widget_maximum_dimensions_crop_the_resolved_outer_block() {
        let layout = Style::new()
            .width(6)
            .height(4)
            .padding((1, 1))
            .border(Border::ASCII)
            .margin(1)
            .max_width(6)
            .max_height(4);
        let direct = layout.render("ab");
        let expected: Vec<_> = direct.lines().collect();
        let style = layout.background(Color::BLUE);
        let area = Rect::new(0, 0, 12, 8);
        let mut buffer = Buffer::empty(area);

        style.widget("ab").render(area, &mut buffer);

        assert_eq!(expected.len(), 4);
        for (y, expected_line) in expected.into_iter().enumerate() {
            assert_eq!(&buffer_line(&buffer, y as u16)[..6], expected_line);
        }
        assert_eq!(
            buffer.cell((6, 2)).expect("outside max width").bg,
            RatatuiColor::Reset
        );
        assert_eq!(
            buffer.cell((2, 4)).expect("outside max height").bg,
            RatatuiColor::Reset
        );
    }

    #[test]
    fn widget_maximum_width_does_not_split_a_cjk_cell() {
        let style = Style::new().background(Color::BLUE).max_width(3);
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);

        style.widget("日本語").render(area, &mut buffer);

        assert_eq!(buffer.cell((0, 0)).expect("first CJK cell").symbol(), "日");
        assert_eq!(buffer.cell((2, 0)).expect("inside max width").symbol(), " ");
        assert_eq!(
            buffer.cell((2, 0)).expect("ragged crop cell").bg,
            RatatuiColor::Reset
        );
        assert_eq!(
            buffer.cell((3, 0)).expect("outside max width").bg,
            RatatuiColor::Reset
        );
    }

    #[test]
    fn widget_maximum_width_accounts_for_margin_before_a_wide_cell() {
        let style = Style::new()
            .background(Color::BLUE)
            .margin((0, 0, 0, 1))
            .max_width(2);
        let area = Rect::new(0, 0, 3, 1);
        let mut buffer = Buffer::empty(area);

        style.widget("日").render(area, &mut buffer);

        assert_eq!(style.render("日"), " ");
        assert_eq!(buffer.cell((0, 0)).expect("retained margin").symbol(), " ");
        assert_eq!(
            buffer.cell((1, 0)).expect("ragged crop cell").bg,
            RatatuiColor::Reset
        );
        assert_eq!(
            buffer.cell((2, 0)).expect("outside maximum width").bg,
            RatatuiColor::Reset
        );
    }

    #[test]
    fn widget_maximum_width_uses_grapheme_width_for_zwj_emoji() {
        let style = Style::new().background(Color::BLUE).max_width(3);
        let area = Rect::new(0, 0, 6, 1);
        let mut buffer = Buffer::empty(area);

        style.widget("👩‍💻x").render(area, &mut buffer);

        assert_eq!(buffer.cell((0, 0)).expect("emoji cell").symbol(), "👩‍💻");
        assert_eq!(buffer.cell((2, 0)).expect("following cell").symbol(), "x");
        assert_eq!(
            buffer.cell((3, 0)).expect("outside max width").bg,
            RatatuiColor::Reset
        );
    }

    #[test]
    fn widget_maximum_clip_stays_anchored_to_the_requested_area() {
        let style = Style::new().background(Color::BLUE).width(10).max_width(6);
        let buffer_area = Rect::new(5, 0, 10, 1);
        let mut buffer = Buffer::empty(buffer_area);

        style
            .widget("x")
            .render(Rect::new(0, 0, 10, 1), &mut buffer);

        assert_eq!(
            buffer.cell((5, 0)).expect("inside max width").bg,
            RatatuiColor::Blue
        );
        assert_eq!(
            buffer.cell((6, 0)).expect("outside max width").bg,
            RatatuiColor::Reset
        );
    }

    #[test]
    fn widget_buffer_intersection_only_clips_the_resolved_layout() {
        let horizontal = Style::new().width(10).max_width(6);
        let horizontal_area = Rect::new(5, 0, 10, 1);
        let mut horizontal_buffer = Buffer::empty(horizontal_area);

        horizontal
            .widget("abcdefghij")
            .render(Rect::new(0, 0, 10, 1), &mut horizontal_buffer);

        assert_eq!(
            horizontal_buffer
                .cell((5, 0))
                .expect("sixth resolved cell")
                .symbol(),
            "f"
        );
        assert_eq!(
            horizontal_buffer
                .cell((6, 0))
                .expect("outside maximum width")
                .symbol(),
            " "
        );

        let vertical = Style::new().width(1).max_height(4);
        let vertical_area = Rect::new(0, 2, 1, 2);
        let mut vertical_buffer = Buffer::empty(vertical_area);

        vertical
            .widget("a\nb\nc\nd\ne")
            .render(Rect::new(0, 0, 1, 5), &mut vertical_buffer);

        assert_eq!(
            vertical_buffer.cell((0, 2)).expect("third row").symbol(),
            "c"
        );
        assert_eq!(
            vertical_buffer.cell((0, 3)).expect("fourth row").symbol(),
            "d"
        );
    }

    #[test]
    fn widget_preserves_legacy_partial_buffer_layout_without_a_binding_maximum() {
        let styles = [
            Style::new().width(10),
            Style::new().width(10).max_width(0),
            Style::new().width(10).max_width(99),
        ];
        for style in styles {
            let buffer_area = Rect::new(5, 0, 5, 1);
            let mut buffer = Buffer::empty(buffer_area);

            style
                .widget("abcdefghij")
                .render(Rect::new(0, 0, 10, 1), &mut buffer);

            assert_eq!(buffer_line(&buffer, 0), "abcde");
        }

        let styles = [
            Style::new().width(1).height(4),
            Style::new().width(1).height(4).max_height(0),
            Style::new().width(1).height(4).max_height(99),
        ];
        for style in styles {
            let buffer_area = Rect::new(0, 2, 1, 2);
            let mut buffer = Buffer::empty(buffer_area);

            style
                .widget("a\nb\nc\nd")
                .render(Rect::new(0, 0, 1, 4), &mut buffer);

            assert_eq!(
                buffer.cell((0, 2)).expect("first visible row").symbol(),
                "a"
            );
            assert_eq!(
                buffer.cell((0, 3)).expect("second visible row").symbol(),
                "b"
            );
        }
    }

    #[test]
    fn widget_styles_only_enabled_border_edges() {
        let style = Style::new()
            .foreground(Color::GREEN)
            .border(Border::NORMAL)
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_foreground(Color::RED);
        let area = Rect::new(0, 0, 2, 1);
        let mut buffer = Buffer::empty(area);

        style.widget("x").render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "│x");
        assert_eq!(
            buffer.cell((0, 0)).expect("border cell").fg,
            RatatuiColor::Red
        );
        assert_eq!(
            buffer.cell((1, 0)).expect("content cell").fg,
            RatatuiColor::Green
        );
    }

    #[test]
    fn widget_draws_the_configured_edge_in_degenerate_areas() {
        let cases = [
            (
                Style::new()
                    .border(Border::ASCII)
                    .border_right(false)
                    .border_bottom(false)
                    .border_left(false),
                "-",
            ),
            (
                Style::new()
                    .border(Border::ASCII)
                    .border_top(false)
                    .border_right(false)
                    .border_bottom(false),
                "|",
            ),
            (
                Style::new()
                    .border(Border::ASCII)
                    .border_top(false)
                    .border_right(false)
                    .border_left(false),
                "-",
            ),
            (
                Style::new()
                    .border(Border::ASCII)
                    .border_top(false)
                    .border_bottom(false)
                    .border_left(false),
                "|",
            ),
        ];

        for (style, expected) in cases {
            let area = Rect::new(0, 0, 1, 1);
            let mut buffer = Buffer::empty(area);
            style.widget("x").render(area, &mut buffer);
            assert_eq!(buffer_line(&buffer, 0), expected);
        }
    }

    fn buffer_line(buffer: &Buffer, y: u16) -> String {
        let mut line = String::new();
        let mut x = buffer.area.left();
        while x < buffer.area.right() {
            let symbol = buffer.cell((x, y)).expect("cell").symbol();
            line.push_str(symbol);
            let width = visible_width(symbol).max(1).min(usize::from(u16::MAX)) as u16;
            x = x.saturating_add(width);
        }
        line
    }
}
