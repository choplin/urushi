//! Ratatui widgets that draw a resolved Urushi view into a caller-owned buffer.
//!
//! These widgets compute no geometry. A target [`Rect`] becomes an
//! [`Available`] area, the
//! core layout pass resolves the view once, and each resulting
//! [`StyledGrapheme`] is written to a cell. Border reservation, padding,
//! alignment, and dimension resolution live in `urushi` alone, so the Ratatui
//! output and the ANSI output cannot drift apart.

use ::ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::RatatuiStyle;
use urushi::{Available, BlockStyle, ResolvedView, StyledGrapheme, View, resolve};

/// A stateless Ratatui widget that draws an Urushi [`View`].
///
/// The widget draws into the buffer passed to [`Widget::render`]. It does not
/// initialize a terminal, read events, or own terminal I/O. Text in the view is
/// plain; ANSI escape sequences are not interpreted inside a Ratatui buffer.
///
/// The target `Rect` bounds the view as an outer clip, not as a layout
/// constraint the view is re-fitted into: the view resolves at its intrinsic
/// size and whatever falls outside the `Rect` is cropped, exactly as a terminal
/// width crops the ANSI backend.
#[derive(Debug, Clone, Copy)]
pub struct ViewWidget<'a> {
    view: &'a View,
}

impl<'a> ViewWidget<'a> {
    /// Creates a widget that draws `view`.
    pub const fn new(view: &'a View) -> Self {
        Self { view }
    }
}

impl Widget for ViewWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        draw(self.view, area, buffer);
    }
}

impl Widget for &ViewWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        draw(self.view, area, buffer);
    }
}

/// A stateless Ratatui widget backed by an Urushi [`BlockStyle`].
///
/// This is the single-block case of [`ViewWidget`], the counterpart of
/// [`BlockStyle::render`]: both resolve `Block(style, Text(content, …))`, so a
/// block drawn here and the same block rendered to ANSI differ only by the crop
/// the target `Rect` imposes.
#[derive(Debug, Clone, Copy)]
pub struct RatatuiWidget<'a> {
    content: &'a str,
    style: &'a BlockStyle,
}

impl<'a> RatatuiWidget<'a> {
    /// Creates a widget that renders `content` with `style`.
    pub const fn new(content: &'a str, style: &'a BlockStyle) -> Self {
        Self { content, style }
    }

    /// Builds the view this widget draws.
    fn view(&self) -> View {
        View::block(
            self.style.clone(),
            View::text(self.content, self.style.text().clone()),
        )
    }
}

/// Extension methods for adapting an Urushi style to Ratatui.
pub trait RatatuiStyleExt {
    /// Adapts this style and `content` into a stateless Ratatui widget.
    fn widget<'a>(&'a self, content: &'a str) -> RatatuiWidget<'a>;
}

impl RatatuiStyleExt for BlockStyle {
    fn widget<'a>(&'a self, content: &'a str) -> RatatuiWidget<'a> {
        RatatuiWidget::new(content, self)
    }
}

impl Widget for RatatuiWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        draw(&self.view(), area, buffer);
    }
}

impl Widget for &RatatuiWidget<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        draw(&self.view(), area, buffer);
    }
}

/// Resolves `view` for `area` and writes the resulting rectangle.
fn draw(view: &View, area: Rect, buffer: &mut Buffer) {
    draw_resolved(&resolve(view, available(area)), area, buffer);
}

/// Writes an already resolved view into `buffer`, anchored at `area`'s origin.
///
/// This is the cell-writing path the widgets take, exposed for a caller that
/// resolves the view itself — a renderer that needs the resolution as well as
/// the cells, and resolves exactly once per frame. Resolve under
/// [`available`] to reproduce what the widgets draw.
///
/// The part of `area` outside `buffer` masks which cells are written; it never
/// moves the rectangle. A grapheme that would straddle the mask is dropped
/// rather than split, matching how the layout pass crops.
pub fn draw_resolved(resolved: &ResolvedView, area: Rect, buffer: &mut Buffer) {
    let clip = area.intersection(buffer.area);
    if clip.is_empty() {
        return;
    }
    write_cells(resolved, area, clip, buffer);
}

/// Translates a target rectangle into the area the layout pass resolves under.
pub fn available(area: Rect) -> Available {
    Available::size(usize::from(area.width), usize::from(area.height))
}

/// Writes a resolved rectangle, anchored at `area`'s origin, under `clip`.
fn write_cells(resolved: &ResolvedView, area: Rect, clip: Rect, buffer: &mut Buffer) {
    for (row, graphemes) in resolved.rows().iter().enumerate() {
        let Some(y) = offset(area.y, row) else {
            return;
        };
        if y < clip.top() {
            continue;
        }
        if y >= clip.bottom() {
            return;
        }

        let mut column = 0;
        for grapheme in graphemes {
            let Some(x) = offset(area.x, column) else {
                break;
            };
            column += grapheme.width();
            write_grapheme(grapheme, x, y, clip, buffer);
        }
    }
}

/// Writes one grapheme, leaving the cells a wide grapheme hides reset.
fn write_grapheme(grapheme: &StyledGrapheme, x: u16, y: u16, clip: Rect, buffer: &mut Buffer) {
    // Zero-width graphemes have no cell of their own, and a grapheme is never
    // split across the clip boundary.
    let Ok(width) = u16::try_from(grapheme.width()) else {
        return;
    };
    if width == 0 {
        return;
    }
    let end = x.saturating_add(width);
    if x < clip.left() || end > clip.right() {
        return;
    }

    let style = RatatuiStyle::from(grapheme.style()).into_inner();
    if let Some(cell) = buffer.cell_mut((x, y)) {
        cell.set_symbol(grapheme.symbol()).set_style(style);
    }
    for hidden in x.saturating_add(1)..end {
        if let Some(cell) = buffer.cell_mut((hidden, y)) {
            cell.reset();
        }
    }
}

fn offset(origin: u16, cells: usize) -> Option<u16> {
    u16::try_from(cells)
        .ok()
        .and_then(|cells| origin.checked_add(cells))
}

#[cfg(test)]
mod tests {
    use ::ratatui::style::{Color as RatatuiColor, Modifier};
    use urushi::{Align, Border, Color, TextStyle, VerticalAlign};

    use super::*;

    #[test]
    fn converts_colors_and_every_modifier() {
        let converted = RatatuiStyle::from(
            &TextStyle::new()
                .foreground(Color::Rgb(1, 2, 3))
                .background(Color::Ansi256(212))
                .bold()
                .dim()
                .italic()
                .underline()
                .blink()
                .reverse()
                .hide()
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
                | Modifier::HIDDEN
                | Modifier::CROSSED_OUT
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
                RatatuiStyle::from(&TextStyle::new().foreground(Color::Ansi(index as u8)))
                    .into_inner();
            assert_eq!(converted.fg, Some(expected));
        }
    }

    /// A block's border colors reach the buffer as the border graphemes' own
    /// text style, so the adapter needs no separate border-style channel.
    #[test]
    fn border_colors_arrive_as_the_border_graphemes_text_style() {
        let style = BlockStyle::new()
            .border(Border::ROUNDED)
            .border_foreground(Color::Rgb(10, 20, 30))
            .border_background(Color::Ansi256(236));
        let area = Rect::new(0, 0, 3, 3);
        let mut buffer = Buffer::empty(area);

        style.widget("x").render(area, &mut buffer);

        let corner = buffer.cell((0, 0)).expect("border cell");
        assert_eq!(corner.symbol(), "╭");
        assert_eq!(corner.fg, RatatuiColor::Rgb(10, 20, 30));
        assert_eq!(corner.bg, RatatuiColor::Indexed(236));

        let content = buffer.cell((1, 1)).expect("content cell");
        assert_eq!(content.fg, RatatuiColor::Reset);
        assert_eq!(content.bg, RatatuiColor::Reset);
    }

    #[test]
    fn widget_preserves_box_model_alignment_and_cjk_width() {
        let style = BlockStyle::new()
            .foreground(Color::GREEN)
            .background(Color::BLUE)
            .padding((0, 1))
            .margin((1, 2))
            .border(Border::ROUNDED)
            .border_foreground(Color::RED)
            // The width measures the outer box: two border columns, two
            // padding columns, and six cells of content.
            .width(10)
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

    /// A `Rect` narrower than the block is an area the box resolves under, so
    /// the frame closes inside it and the wide content reflows. Only one
    /// content row fits the three-row area, so `本語` falls outside the box.
    #[test]
    fn a_narrow_rect_refits_wide_content_and_the_frame_stays_closed() {
        let area = Rect::new(0, 0, 5, 3);
        let mut buffer = Buffer::empty(area);

        BlockStyle::new()
            .border(Border::NORMAL)
            .widget("日本語")
            .render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "┌───┐");
        assert_eq!(buffer_line(&buffer, 1), "│日 │");
        assert_eq!(buffer_line(&buffer, 2), "└───┘");
    }

    #[test]
    fn widget_is_safe_for_zero_and_narrow_areas() {
        let style = BlockStyle::new().border(Border::NORMAL).padding(2);
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
    fn widget_styles_only_enabled_border_edges() {
        let style = BlockStyle::new()
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

    /// A one-cell area is a degenerate case, and the axes degrade differently:
    /// the content may vanish vertically, so a horizontal edge takes the cell,
    /// while the width cannot go below one unsplittable grapheme, so a
    /// vertical edge leaves the box two cells wide and the safety net keeps
    /// its left cell.
    #[test]
    fn a_one_cell_area_degrades_to_the_frame_or_the_leading_cell() {
        // (case, the one enabled edge as (top, right, bottom, left), expected)
        let cases = [
            ("top edge", (true, false, false, false), "-"),
            ("left edge", (false, false, false, true), "|"),
            ("bottom edge", (false, false, true, false), "-"),
            ("right edge", (false, true, false, false), "x"),
        ];

        for (case, (top, right, bottom, left), expected) in cases {
            let style = BlockStyle::new()
                .border(Border::ASCII)
                .border_top(top)
                .border_right(right)
                .border_bottom(bottom)
                .border_left(left);
            let area = Rect::new(0, 0, 1, 1);
            let mut buffer = Buffer::empty(area);

            style.widget("x").render(area, &mut buffer);

            assert_eq!(buffer_line(&buffer, 0), expected, "{case}");
        }
    }

    /// The widget resolves the same view `BlockStyle::render` does, so a `Rect`
    /// at least as large as the block reproduces the direct output verbatim.
    #[test]
    fn widget_matches_direct_rendering_for_border_side_combinations() {
        let styles = [
            BlockStyle::new().border(Border::ASCII),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false)
                .border_left(false),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_right(false)
                .border_bottom(false),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_bottom(false),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_right(false)
                .border_left(false),
            BlockStyle::new()
                .border(Border::ASCII)
                .border_top(false)
                .border_bottom(false),
        ];

        for (index, style) in styles.into_iter().enumerate() {
            assert_widget_matches_direct(&style, "x", &format!("border combination {index}"));
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
            let style = BlockStyle::new()
                .width(6)
                .height(4)
                .padding((1, 1))
                .border(Border::ASCII)
                .border_top(false);
            assert_widget_matches_direct(&style, content, case);
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
                let style = BlockStyle::new()
                    .width(6)
                    .height(7)
                    .padding((1, 1))
                    .align(horizontal)
                    .align_vertical(vertical);
                assert_widget_matches_direct(
                    &style,
                    "日\nx",
                    &format!("{horizontal:?} {vertical:?}"),
                );
            }
        }
    }

    #[test]
    fn widget_matches_direct_rendering_for_maximum_dimensions() {
        let style = BlockStyle::new()
            .width(6)
            .height(4)
            .padding((1, 1))
            .border(Border::ASCII)
            .margin(1)
            .max_width(6)
            .max_height(4);

        assert_widget_matches_direct(&style, "ab", "maximum dimensions");
    }

    #[test]
    fn widget_expands_narrow_fixed_width_for_cjk_parity() {
        // A grapheme wider than the requested width expands the block rather
        // than being dropped, in both backends.
        assert_widget_matches_direct(&BlockStyle::new().width(1).height(2), "日本", "wrapped");
        assert_widget_matches_direct(&BlockStyle::new().width(1), "日", "single grapheme");
    }

    #[test]
    fn widget_matches_direct_maximum_width_cropping_of_wide_graphemes() {
        // Cropping drops a straddling grapheme rather than splitting it, and
        // the freed cells keep the block rectangular.
        assert_widget_matches_direct(&BlockStyle::new().max_width(3), "日本語", "CJK");
        assert_widget_matches_direct(&BlockStyle::new().max_width(3), "👩‍💻x", "ZWJ emoji");
        assert_widget_matches_direct(
            &BlockStyle::new().margin((0, 0, 0, 1)).max_width(2),
            "日",
            "margin before a wide cell",
        );
    }

    /// A `Rect` smaller than the block is the area it resolves under, so the
    /// box takes the area's height and the alignment places the content inside
    /// what actually fits.
    #[test]
    fn a_rect_smaller_than_the_block_resizes_it() {
        let style = BlockStyle::new().width(4).height(6);
        let area = Rect::new(0, 0, 4, 4);

        for (align, expected_row) in [
            (VerticalAlign::Center, Some(1)),
            (VerticalAlign::Bottom, Some(3)),
        ] {
            let style = style.clone().align_vertical(align);
            let mut buffer = Buffer::empty(area);

            style.widget("x").render(area, &mut buffer);

            for y in 0..area.height {
                let expected = if Some(y) == expected_row {
                    "x   "
                } else {
                    "    "
                };
                assert_eq!(buffer_line(&buffer, y), expected, "{align:?} row {y}");
            }
        }
    }

    #[test]
    fn a_shorter_area_closes_the_frame_instead_of_dropping_its_bottom_edge() {
        let style = BlockStyle::new().width(4).height(6).border(Border::ASCII);
        let area = Rect::new(0, 0, 6, 4);
        let mut buffer = Buffer::empty(area);

        style.widget("x").render(area, &mut buffer);

        // The box resolves to 4x4 under the area, so the bottom edge is drawn
        // at the fourth row instead of falling outside it.
        assert_eq!(buffer_line(&buffer, 0), "+--+  ");
        assert_eq!(buffer_line(&buffer, 1), "|x |  ");
        assert_eq!(buffer_line(&buffer, 2), "|  |  ");
        assert_eq!(buffer_line(&buffer, 3), "+--+  ");
    }

    /// A buffer covering only part of the target `Rect` masks which cells are
    /// written; it never moves the layout's origin.
    #[test]
    fn a_partial_buffer_masks_cells_without_moving_the_layout() {
        let style = BlockStyle::new().width(10);
        let mut buffer = Buffer::empty(Rect::new(5, 0, 5, 1));

        style
            .widget("abcdefghij")
            .render(Rect::new(0, 0, 10, 1), &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "fghij");

        let style = BlockStyle::new().width(1).height(4);
        let mut buffer = Buffer::empty(Rect::new(0, 2, 1, 2));

        style
            .widget("a\nb\nc\nd")
            .render(Rect::new(0, 0, 1, 4), &mut buffer);

        assert_eq!(buffer.cell((0, 2)).expect("third row").symbol(), "c");
        assert_eq!(buffer.cell((0, 3)).expect("fourth row").symbol(), "d");
    }

    #[test]
    fn view_widget_draws_a_bordered_block_inside_a_row() {
        let view = View::row(
            VerticalAlign::Center,
            [
                View::text("status: ", TextStyle::new()),
                View::block(
                    BlockStyle::new()
                        .border(Border::ROUNDED)
                        .border_foreground(Color::GREEN),
                    View::text("ok", TextStyle::new().foreground(Color::GREEN)),
                ),
            ],
        );
        let area = Rect::new(0, 0, 12, 3);
        let mut buffer = Buffer::empty(area);

        ViewWidget::new(&view).render(area, &mut buffer);

        assert_eq!(buffer_line(&buffer, 0), "        ╭──╮");
        assert_eq!(buffer_line(&buffer, 1), "status: │ok│");
        assert_eq!(buffer_line(&buffer, 2), "        ╰──╯");
        assert_eq!(
            buffer.cell((8, 0)).expect("border cell").fg,
            RatatuiColor::Green
        );
    }

    /// A caller that resolves the view itself — the renderer, which needs the
    /// resolution as well as the cells — reaches the same buffer the widget
    /// draws, because both take one cell-writing path.
    #[test]
    fn resolving_first_and_drawing_reaches_the_same_cells_as_the_widget() {
        let view = View::row(
            VerticalAlign::Center,
            [
                View::text("status: ", TextStyle::new()),
                View::block(
                    BlockStyle::new()
                        .border(Border::ROUNDED)
                        .border_foreground(Color::GREEN),
                    View::text("日本", TextStyle::new().foreground(Color::GREEN)),
                ),
            ],
        );
        // A buffer narrower than the target rectangle also masks the cells, so
        // the two paths have to agree on the crop as well as the content.
        let area = Rect::new(1, 0, 14, 3);
        let mut through_widget = Buffer::empty(Rect::new(0, 0, 10, 3));
        let mut through_resolved = Buffer::empty(Rect::new(0, 0, 10, 3));

        ViewWidget::new(&view).render(area, &mut through_widget);
        draw_resolved(
            &resolve(&view, available(area)),
            area,
            &mut through_resolved,
        );

        assert_eq!(through_resolved, through_widget);
        // The comparison is only meaningful because cells were written and the
        // buffer cropped the block's right half.
        assert_eq!(buffer_line(&through_widget, 1), " status: │");
    }

    /// Asserts the widget reproduces `BlockStyle::render` in a `Rect` sized to
    /// the block.
    fn assert_widget_matches_direct(style: &BlockStyle, content: &str, case: &str) {
        let direct = style.render(content);
        let expected: Vec<&str> = direct.as_str().lines().collect();
        let area = Rect::new(
            0,
            0,
            u16::try_from(direct.size().width()).expect("block width"),
            u16::try_from(direct.size().height()).expect("block height"),
        );
        let mut buffer = Buffer::empty(area);

        style.widget(content).render(area, &mut buffer);

        for (y, expected_line) in expected.into_iter().enumerate() {
            assert_eq!(buffer_line(&buffer, y as u16), expected_line, "{case}");
        }
    }

    fn buffer_line(buffer: &Buffer, y: u16) -> String {
        let mut line = String::new();
        let mut x = buffer.area.left();
        while x < buffer.area.right() {
            let symbol = buffer.cell((x, y)).expect("cell").symbol();
            line.push_str(symbol);
            let width = urushi::PrintableText::new(symbol)
                .width()
                .max(1)
                .min(usize::from(u16::MAX)) as u16;
            x = x.saturating_add(width);
        }
        line
    }
}
