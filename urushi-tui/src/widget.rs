//! Ratatui widget rendering for urushi's box model.

use ratatui::{buffer::Buffer, layout::Rect, style::Style as InnerStyle, widgets::Widget};

use super::{RatatuiStyle, style::convert_color};
use urushi::{Align, Border, Color, Sides, Style, visible_width, wrap_text};

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
    border_foreground: Option<Color>,
    border_background: Option<Color>,
    width: Option<u16>,
    align: Align,
}

impl BoxParts {
    const fn from_style(style: &Style) -> Self {
        Self {
            padding: style.padding_sides(),
            margin: style.margin_sides(),
            border: style.border_kind(),
            border_foreground: style.border_foreground_color(),
            border_background: style.border_background_color(),
            width: style.fixed_width(),
            align: style.horizontal_alignment(),
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
    let area = area.intersection(buffer.area);
    if area.is_empty() {
        return;
    }

    let parts = BoxParts::from_style(widget.style);
    let Some(available) = inset(area, parts.margin) else {
        return;
    };

    let border_cells = u16::from(parts.border.is_some()) * 2;
    let max_box_width = available.width.saturating_sub(border_cells);
    let max_box_height = available.height.saturating_sub(border_cells);
    if max_box_width == 0 || max_box_height == 0 {
        render_degenerate_border(available, parts, buffer);
        return;
    }

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
    let box_width = requested_box_width.min(max_box_width);
    let inner_width = box_width.saturating_sub(horizontal_padding);

    let lines = if parts.width.is_some() && inner_width > 0 {
        wrap_text(widget.content, usize::from(inner_width))
    } else {
        let mut lines: Vec<String> = widget.content.lines().map(str::to_owned).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        lines
    };

    let content_height = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let requested_box_height = parts
        .padding
        .top
        .saturating_add(content_height)
        .saturating_add(parts.padding.bottom);
    let box_height = requested_box_height.min(max_box_height);
    let total_width = box_width.saturating_add(border_cells);
    let total_height = box_height.saturating_add(border_cells);
    let widget_area = Rect::new(available.x, available.y, total_width, total_height);

    let text_style = RatatuiStyle::from(widget.style).into_inner();
    let box_area = if parts.border.is_some() {
        Rect::new(
            widget_area.x.saturating_add(1),
            widget_area.y.saturating_add(1),
            box_width,
            box_height,
        )
    } else {
        widget_area
    };
    fill(box_area, text_style, buffer);

    if let Some(border) = parts.border {
        draw_border(widget_area, border, border_style(parts), buffer);
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

    for (offset, line) in lines.iter().take(usize::from(content_rows)).enumerate() {
        let line_width = visible_width(line).min(usize::from(u16::MAX)) as u16;
        let gap = content_width.saturating_sub(line_width);
        let alignment_offset = match parts.align {
            Align::Left => 0,
            Align::Center => gap / 2,
            Align::Right => gap,
        };
        let y = content_y.saturating_add(offset as u16);
        buffer.set_stringn(
            content_x.saturating_add(alignment_offset),
            y,
            line,
            usize::from(content_width.saturating_sub(alignment_offset)),
            text_style,
        );
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

fn draw_border(area: Rect, border: Border, style: InnerStyle, buffer: &mut Buffer) {
    if area.is_empty() {
        return;
    }

    let left = area.left();
    let right = area.right().saturating_sub(1);
    let top = area.top();
    let bottom = area.bottom().saturating_sub(1);

    for x in left..=right {
        let top_char = if x == left {
            border.top_left
        } else if x == right {
            border.top_right
        } else {
            border.top
        };
        set_char(buffer, x, top, top_char, style);

        if bottom != top {
            let bottom_char = if x == left {
                border.bottom_left
            } else if x == right {
                border.bottom_right
            } else {
                border.bottom
            };
            set_char(buffer, x, bottom, bottom_char, style);
        }
    }

    for y in top.saturating_add(1)..bottom {
        set_char(buffer, left, y, border.left, style);
        if right != left {
            set_char(buffer, right, y, border.right, style);
        }
    }
}

fn render_degenerate_border(area: Rect, parts: BoxParts, buffer: &mut Buffer) {
    if let Some(border) = parts.border {
        draw_border(area, border, border_style(parts), buffer);
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
                .align(Align::Center),
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
