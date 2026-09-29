//! Resolves one application view into one terminal frame.

use urushi::{Available, ResolvedView, StyledGrapheme, TextStyle, View};
#[cfg(feature = "graphics")]
use urushi_graphics::image_placements;
use urushi_terminal::{Position, TerminalSize};
use urushi_tui::{Frame, Rect};

use super::evaluator::Evaluator;

pub(crate) trait RenderFrame {
    fn area(&self) -> Rect;
    fn put(&mut self, column: usize, row: usize, cell: &StyledGrapheme);
    fn set_cursor(&mut self, at: Option<Position>);
}

impl RenderFrame for Frame<'_> {
    fn area(&self) -> Rect {
        Frame::area(self)
    }

    fn put(&mut self, column: usize, row: usize, cell: &StyledGrapheme) {
        Frame::put(self, column, row, cell);
    }

    fn set_cursor(&mut self, at: Option<Position>) {
        Frame::set_cursor(self, at);
    }
}

/// Resolves and draws one view, including its cursor request.
pub(crate) fn render(view: &View, frame: &mut impl RenderFrame, evaluator: &mut Evaluator) {
    let resolved = resolve(view, frame.area().size(), evaluator);
    render_resolved(&resolved, frame);
}

pub(crate) fn resolve(view: &View, size: TerminalSize, evaluator: &mut Evaluator) -> ResolvedView {
    evaluator
        .resolve(view, Available::size(size.columns(), size.rows()))
        .expect("a terminal surface supplies finite view geometry")
}

pub(crate) fn render_resolved(resolved: &ResolvedView, frame: &mut impl RenderFrame) {
    render_resolved_with_masks(resolved, &[], frame);
}

#[cfg(feature = "graphics")]
pub(crate) fn render_graphics_resolved(
    view: &View,
    resolved: &ResolvedView,
    frame: &mut impl RenderFrame,
) {
    let masks = image_placements(view, resolved, None)
        .into_iter()
        .filter_map(|placement| {
            let origin = placement.visible_origin()?;
            let size = placement.visible_size()?;
            Some(CellMask {
                column: usize::try_from(origin.x).ok()?,
                row: usize::try_from(origin.y).ok()?,
                width: size.width(),
                height: size.height(),
            })
        })
        .collect::<Vec<_>>();
    render_resolved_with_masks(resolved, &masks, frame);
}

#[derive(Clone, Copy)]
struct CellMask {
    column: usize,
    row: usize,
    width: usize,
    height: usize,
}

impl CellMask {
    fn intersects(self, column: usize, row: usize, width: usize) -> bool {
        let Some(right) = self.column.checked_add(self.width) else {
            return false;
        };
        let Some(bottom) = self.row.checked_add(self.height) else {
            return false;
        };
        let Some(grapheme_right) = column.checked_add(width) else {
            return false;
        };
        row >= self.row && row < bottom && column < right && grapheme_right > self.column
    }
}

fn render_resolved_with_masks(
    resolved: &ResolvedView,
    masks: &[CellMask],
    frame: &mut impl RenderFrame,
) {
    let area = frame.area();
    let origin = area.origin();

    for (row, graphemes) in resolved.rows().iter().enumerate() {
        let Some(row) = origin.row().checked_add(row) else {
            continue;
        };
        let mut column_offset = 0usize;
        for grapheme in graphemes {
            let masked = masks
                .iter()
                .any(|mask| mask.intersects(column_offset, row - origin.row(), grapheme.width()));
            if masked {
                let style = grapheme
                    .style()
                    .get_background()
                    .map_or_else(TextStyle::new, |color| TextStyle::new().background(color));
                let blank = StyledGrapheme::space(style);
                for offset in 0..grapheme.width() {
                    if let Some(column) = origin
                        .column()
                        .checked_add(column_offset)
                        .and_then(|column| column.checked_add(offset))
                    {
                        frame.put(column, row, &blank);
                    }
                }
            } else if let Some(column) = origin.column().checked_add(column_offset) {
                frame.put(column, row, grapheme);
            }
            column_offset = column_offset.saturating_add(grapheme.width());
        }
    }

    let cursor = resolved
        .anchors()
        .iter()
        .find(|anchor| anchor.is_empty())
        .and_then(|anchor| anchor.visible())
        .and_then(|visible| {
            let column = usize::try_from(visible.x()).ok()?;
            let row = usize::try_from(visible.y()).ok()?;
            Some(Position::new(
                origin.column().checked_add(column)?,
                origin.row().checked_add(row)?,
            ))
        });
    frame.set_cursor(cursor);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use urushi::{
        Align, BlockStyle, Canvas, CanvasContext, CanvasItem, Length, Position as CanvasPosition,
        Projection, ProjectionBoundary, Size, TextStyle, VerticalAlign, Viewport,
    };
    #[cfg(feature = "graphics")]
    use urushi::{Color, View};
    #[cfg(feature = "graphics")]
    use urushi_graphics::{CellSize, Image, ImagePresentation, PixelSize};
    use urushi_terminal::TerminalSize;

    use super::*;
    use crate::testing::{InMemoryCell, InMemoryTerminal};

    fn draw(view: &View, size: TerminalSize) -> InMemoryTerminal {
        let mut terminal = InMemoryTerminal::new(size);
        let mut evaluator = Evaluator::default();
        terminal
            .draw(|frame| render(view, frame, &mut evaluator))
            .expect("in-memory draw succeeds");
        terminal
    }

    #[cfg(feature = "graphics")]
    fn draw_graphics(view: &View, size: TerminalSize) -> InMemoryTerminal {
        let mut terminal = InMemoryTerminal::new(size);
        let mut evaluator = Evaluator::default();
        let resolved = resolve(view, size, &mut evaluator);
        terminal
            .draw(|frame| render_graphics_resolved(view, &resolved, frame))
            .expect("in-memory graphics cell draw succeeds");
        terminal
    }

    #[cfg(feature = "graphics")]
    fn image_view(
        placement: &'static str,
        fallback: &str,
        cells: CellSize,
        fallback_style: TextStyle,
    ) -> View {
        let image = Image::rgba(placement, placement, PixelSize::new(1, 1), [255, 0, 0, 0])
            .unwrap()
            .fallback(fallback);
        ImagePresentation::new()
            .fallback_style(fallback_style)
            .compose(&image, cells)
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn graphics_cells_mask_multiple_visible_fallbacks_and_keep_backgrounds() {
        let first = image_view(
            "first",
            "界",
            CellSize::new(2, 1),
            TextStyle::new().background(Color::BLUE).italic(),
        );
        let second = image_view("second", "xy", CellSize::new(2, 1), TextStyle::new());
        let view = View::row(
            VerticalAlign::Top,
            [first, View::text("|", TextStyle::new()), second],
        );

        let terminal = draw_graphics(&view, TerminalSize::new(5, 1));
        let frame = &terminal.frames()[0];

        for column in [0, 1] {
            assert!(matches!(
                frame.cell(column, 0),
                Some(InMemoryCell::Grapheme(grapheme))
                    if grapheme.symbol() == " "
                        && grapheme.style().get_background() == Some(Color::BLUE)
                        && grapheme.style().get_attributes().is_empty()
            ));
        }
        assert!(matches!(
            frame.cell(2, 0),
            Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == "|"
        ));
        for column in [3, 4] {
            assert!(matches!(
                frame.cell(column, 0),
                Some(InMemoryCell::Grapheme(grapheme))
                    if grapheme.symbol() == " " && grapheme.style() == &TextStyle::new()
            ));
        }
    }

    #[cfg(feature = "graphics")]
    #[test]
    fn graphics_cells_mask_only_the_clipped_visible_placement() {
        let view = View::viewport(
            Viewport::horizontal(Projection::new(1, ProjectionBoundary::Preserve)),
            image_view(
                "clipped",
                "abc",
                CellSize::new(3, 1),
                TextStyle::new().background(Color::GREEN),
            ),
        );

        let terminal = draw_graphics(&view, TerminalSize::new(2, 1));
        let frame = &terminal.frames()[0];

        for column in 0..2 {
            assert!(matches!(
                frame.cell(column, 0),
                Some(InMemoryCell::Grapheme(grapheme))
                    if grapheme.symbol() == " "
                        && grapheme.style().get_background() == Some(Color::GREEN)
            ));
        }
    }

    #[test]
    fn anchorless_view_writes_resolved_graphemes_and_hides_the_cursor() {
        let terminal = draw(
            &View::text("界x", TextStyle::new()),
            TerminalSize::new(4, 1),
        );
        let frame = &terminal.frames()[0];

        assert!(matches!(
            frame.cell(0, 0),
            Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == "界"
        ));
        assert_eq!(frame.cell(1, 0), Some(&InMemoryCell::Continuation));
        assert!(matches!(
            frame.cell(2, 0),
            Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == "x"
        ));
        assert_eq!(frame.cursor(), None);
    }

    #[test]
    fn first_empty_anchor_sets_the_cursor_and_sized_anchors_remain_cells() {
        let view = View::row(
            VerticalAlign::Top,
            [
                View::anchor_block(
                    "panel",
                    BlockStyle::new()
                        .width(Length::Cells(2))
                        .height(Length::Cells(1)),
                    View::empty(),
                ),
                View::text(">", TextStyle::new()),
                View::anchor("first-cursor"),
                View::text("x", TextStyle::new()),
                View::anchor("second-cursor"),
            ],
        );
        let terminal = draw(&view, TerminalSize::new(4, 1));
        let frame = &terminal.frames()[0];

        assert!(matches!(
            frame.cell(0, 0),
            Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == " "
        ));
        assert!(matches!(
            frame.cell(1, 0),
            Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == " "
        ));
        assert_eq!(frame.cursor(), Some(Position::new(3, 0)));
    }

    #[test]
    fn cursor_outside_the_resolved_view_is_hidden() {
        let view = View::column(
            Align::Left,
            [
                View::text("a\nb\nc", TextStyle::new()),
                View::anchor("cursor"),
            ],
        );
        let terminal = draw(&view, TerminalSize::new(1, 2));

        assert_eq!(terminal.frames()[0].cursor(), None);
    }

    #[test]
    fn default_evaluator_resolves_viewports_without_retaining_content() {
        let draws = Arc::new(AtomicUsize::new(0));
        let view = viewport_canvas(1, Arc::clone(&draws));
        let mut evaluator = Evaluator::default();
        let mut terminal = InMemoryTerminal::new(TerminalSize::new(2, 1));

        for _ in 0..2 {
            terminal
                .draw(|frame| render(&view, frame, &mut evaluator))
                .expect("in-memory draw succeeds");
        }

        assert_eq!(draws.load(Ordering::Relaxed), 2);
        assert_frame_symbols(&terminal, 0, &["b", "c"]);
        assert_eq!(terminal.frames()[0], terminal.frames()[1]);
    }

    #[test]
    fn retained_evaluator_reuses_content_when_only_viewport_origin_changes() {
        let draws = Arc::new(AtomicUsize::new(0));
        let mut evaluator = Evaluator::retained();
        let mut terminal = InMemoryTerminal::new(TerminalSize::new(2, 1));

        for origin in [0, 1, 0] {
            let view = viewport_canvas(origin, Arc::clone(&draws));
            terminal
                .draw(|frame| render(&view, frame, &mut evaluator))
                .expect("in-memory draw succeeds");
        }

        assert_eq!(draws.load(Ordering::Relaxed), 1);
        assert_frame_symbols(&terminal, 0, &["a", "b"]);
        assert_frame_symbols(&terminal, 1, &["b", "c"]);
        assert_eq!(terminal.frames()[0], terminal.frames()[2]);
    }

    #[test]
    fn retained_evaluator_invalidates_for_resized_available_area() {
        let draws = Arc::new(AtomicUsize::new(0));
        let view = View::canvas(Canvas::new().item(CountingText {
            id: 2,
            draws: Arc::clone(&draws),
        }));
        let mut evaluator = Evaluator::retained();
        let mut terminal = InMemoryTerminal::new(TerminalSize::new(2, 1));
        terminal
            .draw(|frame| render(&view, frame, &mut evaluator))
            .expect("initial draw succeeds");
        terminal
            .resize(TerminalSize::new(3, 1))
            .expect("resize succeeds");
        terminal
            .draw(|frame| render(&view, frame, &mut evaluator))
            .expect("resized draw succeeds");

        assert_eq!(draws.load(Ordering::Relaxed), 2);
        assert_frame_symbols(&terminal, 0, &["a", "b"]);
        assert_frame_symbols(&terminal, 1, &["a", "b", "c"]);
    }

    #[test]
    fn direct_and_retained_evaluators_match_across_origins_and_sizes() {
        let draws = Arc::new(AtomicUsize::new(0));
        let mut retained = Evaluator::retained();

        for (origin, size) in [
            (0, TerminalSize::new(2, 1)),
            (1, TerminalSize::new(2, 1)),
            (1, TerminalSize::new(3, 1)),
        ] {
            let view = viewport_canvas(origin, Arc::clone(&draws));
            let mut retained_terminal = InMemoryTerminal::new(size);
            retained_terminal
                .draw(|frame| render(&view, frame, &mut retained))
                .expect("retained draw succeeds");

            let mut direct = Evaluator::default();
            let mut direct_terminal = InMemoryTerminal::new(size);
            direct_terminal
                .draw(|frame| render(&view, frame, &mut direct))
                .expect("direct draw succeeds");

            assert_eq!(retained_terminal.frames(), direct_terminal.frames());
        }
    }

    #[test]
    fn cursor_clipped_by_nested_viewport_stays_hidden_after_ancestor_offset() {
        let view = View::block(
            BlockStyle::new().width(Length::Cells(3)).padding((0, 1)),
            View::viewport(
                Viewport::horizontal(Projection::new(1, ProjectionBoundary::Preserve)),
                View::row(
                    VerticalAlign::Top,
                    [View::anchor("cursor"), View::text("xy", TextStyle::new())],
                ),
            ),
        );
        let terminal = draw(&view, TerminalSize::new(3, 1));

        assert_eq!(terminal.frames()[0].cursor(), None);
    }

    #[derive(Debug, Clone)]
    struct CountingText {
        id: u8,
        draws: Arc<AtomicUsize>,
    }

    impl PartialEq for CountingText {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl CanvasItem for CountingText {
        fn draw(&self, context: &mut CanvasContext) {
            self.draws.fetch_add(1, Ordering::Relaxed);
            context.text(CanvasPosition::new(0, 0), "abcd", TextStyle::new());
        }
    }

    fn viewport_canvas(origin: i64, draws: Arc<AtomicUsize>) -> View {
        View::viewport(
            Viewport::horizontal(Projection::new(origin, ProjectionBoundary::Preserve)),
            View::block(
                BlockStyle::new()
                    .width(Length::Cells(4))
                    .height(Length::Cells(1)),
                View::canvas(
                    Canvas::new()
                        .extent(Size::new(4, 1))
                        .item(CountingText { id: 1, draws }),
                ),
            ),
        )
    }

    fn assert_frame_symbols(terminal: &InMemoryTerminal, frame: usize, expected: &[&str]) {
        for (column, expected) in expected.iter().enumerate() {
            assert!(matches!(
                terminal.frames()[frame].cell(column, 0),
                Some(InMemoryCell::Grapheme(grapheme)) if grapheme.symbol() == *expected
            ));
        }
    }
}
