//! Resolves one application view into one terminal frame.

use urushi::{Available, StyledGrapheme, View, resolve};
use urushi_terminal::Position;

use crate::terminal::Frame;

/// Resolves and draws one view, including its cursor request.
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the public runtime entry point")
)]
pub(crate) fn render(view: &View, frame: &mut impl Frame<Cell = StyledGrapheme>) {
    let area = frame.area();
    let size = area.size();
    let resolved = resolve(view, Available::size(size.columns(), size.rows()))
        .expect("a frame area supplies finite view geometry");
    let origin = area.origin();

    for (row, graphemes) in resolved.rows().iter().enumerate() {
        let Some(row) = origin.row().checked_add(row) else {
            break;
        };
        let mut column = origin.column();
        for grapheme in graphemes {
            frame.put(column, row, grapheme);
            let Some(next) = column.checked_add(grapheme.width()) else {
                break;
            };
            column = next;
        }
    }

    let cursor = resolved
        .anchors()
        .iter()
        .find(|anchor| anchor.is_empty())
        .filter(|anchor| anchor.is_within_resolved_view())
        .and_then(|anchor| {
            let column = usize::try_from(anchor.x()).ok()?;
            let row = usize::try_from(anchor.y()).ok()?;
            Some(Position::new(
                origin.column().checked_add(column)?,
                origin.row().checked_add(row)?,
            ))
        });
    frame.set_cursor(cursor);
}

#[cfg(test)]
mod tests {
    use urushi::{Align, BlockStyle, Length, TextStyle, VerticalAlign};
    use urushi_terminal::TerminalSize;

    use super::*;
    use crate::runtime::testing::{InMemoryCell, InMemoryTerminal};
    use crate::terminal::Terminal;

    fn draw(view: &View, size: TerminalSize) -> InMemoryTerminal {
        let mut terminal = InMemoryTerminal::new(size);
        terminal
            .draw(|frame| render(view, frame))
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
}
