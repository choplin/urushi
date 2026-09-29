//! Resolves one application view into one terminal frame.

use urushi::{Available, StyledGrapheme, View};
use urushi_terminal::Position;

use super::evaluator::Evaluator;
use crate::cell::visit_resolved;
use crate::terminal::Frame;

/// Resolves and draws one view, including its cursor request.
pub(crate) fn render(
    view: &View,
    frame: &mut impl Frame<Cell = StyledGrapheme>,
    evaluator: &mut Evaluator,
) {
    let area = frame.area();
    let size = area.size();
    let resolved = evaluator
        .resolve(view, Available::size(size.columns(), size.rows()))
        .expect("a frame area supplies finite view geometry");
    let origin = area.origin();

    visit_resolved(&resolved, |column, row, grapheme| {
        let Some(column) = origin.column().checked_add(column) else {
            return;
        };
        let Some(row) = origin.row().checked_add(row) else {
            return;
        };
        frame.put(column, row, grapheme);
    });

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
    use urushi_terminal::TerminalSize;

    use super::*;
    use crate::runtime::testing::{InMemoryCell, InMemoryTerminal};
    use crate::terminal::Terminal;

    fn draw(view: &View, size: TerminalSize) -> InMemoryTerminal {
        let mut terminal = InMemoryTerminal::new(size);
        let mut evaluator = Evaluator::default();
        terminal
            .draw(|frame| render(view, frame, &mut evaluator))
            .expect("in-memory draw succeeds");
        terminal
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
