//! Transactional Ratatui buffer presentation over an Urushi terminal backend.

use std::io;

use ::ratatui::{
    buffer::{Buffer, Cell as RatatuiCell},
    layout::Rect as RatatuiRect,
    style::Modifier as RatatuiModifier,
};
use urushi::{StyledGrapheme, Underline};
use urushi_terminal::{Position, TerminalSize, TerminalStyle};

use super::{
    CellWriteMode,
    style::{terminal_attributes, terminal_color},
    widget::write_grapheme,
};
use crate::terminal::{Cell, CellWriter, Frame, Rect, Terminal};

/// A terminal that uses Ratatui buffers for cell diffing and a cell writer for
/// physical terminal I/O.
///
/// A frame becomes the new committed baseline only after drawing its cells,
/// applying its cursor state, and flushing all succeed. If any step fails, the
/// prior baseline remains. Because an arbitrary output prefix may already have
/// reached the terminal, the next draw clears the physical surface and sends a
/// complete replacement before it can commit.
pub struct RatatuiTerminal<W> {
    writer: W,
    size: TerminalSize,
    committed: Buffer,
    working: Buffer,
    needs_clear: bool,
}

impl<W: CellWriter> RatatuiTerminal<W> {
    pub fn new(writer: W, size: TerminalSize) -> io::Result<Self> {
        let area = ratatui_area(size)?;
        Ok(Self {
            writer,
            size,
            committed: Buffer::empty(area),
            working: Buffer::empty(area),
            needs_clear: true,
        })
    }

    pub const fn writer(&self) -> &W {
        &self.writer
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: CellWriter> Terminal for RatatuiTerminal<W> {
    type Cell = StyledGrapheme;
    type Frame<'a> = RatatuiFrame<'a>;

    fn size(&self) -> TerminalSize {
        self.size
    }

    fn resize(&mut self, size: TerminalSize) -> io::Result<()> {
        let area = ratatui_area(size)?;
        self.size = size;
        self.committed = Buffer::empty(area);
        self.working = Buffer::empty(area);
        self.needs_clear = true;
        Ok(())
    }

    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()> {
        let area = ratatui_area(self.size)?;
        self.working.reset();
        let mut frame = RatatuiFrame {
            buffer: &mut self.working,
            area: Rect::from_size(self.size),
            cursor: None,
        };
        draw(&mut frame);
        let cursor = frame.cursor;

        let blank;
        let baseline = if self.needs_clear {
            blank = Buffer::empty(area);
            &blank
        } else {
            &self.committed
        };
        let changes = baseline.diff(&self.working);
        let result = (|| {
            if self.needs_clear {
                self.writer.clear()?;
            }
            self.writer
                .draw(changes.into_iter().map(|(column, row, cell)| {
                    (
                        Position::new(usize::from(column), usize::from(row)),
                        terminal_cell(cell),
                    )
                }))?;
            self.writer.set_cursor(cursor)?;
            self.writer.flush()
        })();

        if let Err(error) = result {
            // A failed write, cursor command, or flush may already have
            // changed an arbitrary prefix of the physical surface. Its state
            // is now unknowable; the next frame must clear and redraw from a
            // blank baseline even when it differs from this failed frame.
            self.needs_clear = true;
            return Err(error);
        }

        std::mem::swap(&mut self.committed, &mut self.working);
        self.needs_clear = false;
        Ok(())
    }
}

/// Draw-scoped access to the working Ratatui buffer.
pub struct RatatuiFrame<'a> {
    buffer: &'a mut Buffer,
    area: Rect,
    cursor: Option<Position>,
}

impl RatatuiFrame<'_> {
    /// Returns the working Ratatui buffer for native Ratatui widgets.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}

impl Frame for RatatuiFrame<'_> {
    type Cell = StyledGrapheme;

    fn area(&self) -> Rect {
        self.area
    }

    fn put(&mut self, column: usize, row: usize, cell: &Self::Cell) {
        let (Ok(column), Ok(row)) = (u16::try_from(column), u16::try_from(row)) else {
            return;
        };
        let clip = self.buffer.area;
        write_grapheme(cell, column, row, clip, self.buffer, CellWriteMode::Replace);
    }

    fn set_cursor(&mut self, at: Option<Position>) {
        self.cursor = at;
    }
}

fn ratatui_area(size: TerminalSize) -> io::Result<RatatuiRect> {
    let width = u16::try_from(size.columns())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "terminal width exceeds u16"))?;
    let height = u16::try_from(size.rows())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "terminal height exceeds u16"))?;
    Ok(RatatuiRect::new(0, 0, width, height))
}

fn terminal_cell(cell: &RatatuiCell) -> Cell<'_> {
    Cell {
        symbol: cell.symbol(),
        style: TerminalStyle {
            foreground: terminal_color(cell.fg),
            background: terminal_color(cell.bg),
            attributes: terminal_attributes(cell.modifier),
            underline: cell
                .modifier
                .contains(RatatuiModifier::UNDERLINED)
                .then_some(Underline::default()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::ratatui::style::{Color as RatatuiColor, Style};
    use urushi::{Color, TextAttribute};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Failure {
        Clear,
        Draw,
        Cursor,
        Flush,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct DrawnCell {
        position: Position,
        symbol: String,
        style: TerminalStyle,
    }

    struct RecordingBackend {
        failure: Option<Failure>,
        clears: usize,
        draws: Vec<Vec<DrawnCell>>,
        cursors: Vec<Option<Position>>,
        flushes: usize,
    }

    impl RecordingBackend {
        fn new() -> Self {
            Self {
                failure: None,
                clears: 0,
                draws: Vec::new(),
                cursors: Vec::new(),
                flushes: 0,
            }
        }

        fn fail_once(&mut self, failure: Failure) {
            self.failure = Some(failure);
        }

        fn take_failure(&mut self, failure: Failure) -> io::Result<()> {
            if self.failure == Some(failure) {
                self.failure = None;
                Err(io::Error::other("planned terminal failure"))
            } else {
                Ok(())
            }
        }
    }

    impl CellWriter for RecordingBackend {
        fn draw<'a, I>(&mut self, cells: I) -> io::Result<()>
        where
            I: Iterator<Item = (Position, Cell<'a>)>,
        {
            let fail_after_prefix = self.failure == Some(Failure::Draw);
            let mut drawn = Vec::new();
            for (position, cell) in cells {
                drawn.push(DrawnCell {
                    position,
                    symbol: cell.symbol.to_owned(),
                    style: cell.style,
                });
                if fail_after_prefix {
                    self.failure = None;
                    self.draws.push(drawn);
                    return Err(io::Error::other("planned terminal failure"));
                }
            }
            self.draws.push(drawn);
            Ok(())
        }

        fn set_cursor(&mut self, cursor: Option<Position>) -> io::Result<()> {
            self.cursors.push(cursor);
            self.take_failure(Failure::Cursor)
        }

        fn clear(&mut self) -> io::Result<()> {
            self.clears += 1;
            self.take_failure(Failure::Clear)
        }
    }

    impl urushi_terminal::TerminalOutput for RecordingBackend {
        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            self.take_failure(Failure::Flush)
        }
    }

    fn draw_symbol(
        terminal: &mut RatatuiTerminal<RecordingBackend>,
        symbol: &str,
    ) -> io::Result<()> {
        terminal.draw(|frame| {
            frame
                .buffer_mut()
                .cell_mut((0, 0))
                .expect("test cell is in bounds")
                .set_symbol(symbol);
        })
    }

    fn draw_pair(
        terminal: &mut RatatuiTerminal<RecordingBackend>,
        left: &str,
        right: &str,
    ) -> io::Result<()> {
        terminal.draw(|frame| {
            frame
                .buffer_mut()
                .cell_mut((0, 0))
                .expect("left test cell is in bounds")
                .set_symbol(left);
            frame
                .buffer_mut()
                .cell_mut((1, 0))
                .expect("right test cell is in bounds")
                .set_symbol(right);
        })
    }

    #[test]
    fn failed_output_stage_keeps_the_previous_diff_baseline() {
        for failure in [Failure::Draw, Failure::Cursor, Failure::Flush] {
            let mut terminal =
                RatatuiTerminal::new(RecordingBackend::new(), TerminalSize::new(2, 1))
                    .expect("valid terminal size");
            draw_symbol(&mut terminal, "a").expect("baseline frame succeeds");

            terminal.writer.fail_once(failure);
            assert!(draw_symbol(&mut terminal, "b").is_err());
            draw_symbol(&mut terminal, "b").expect("failed frame is retried");

            let draws = &terminal.writer().draws;
            assert_eq!(draws[draws.len() - 2][0].symbol, "b", "{failure:?}");
            assert_eq!(draws[draws.len() - 1][0].symbol, "b", "{failure:?}");
            assert_eq!(terminal.writer().clears, 2, "{failure:?}");
        }
    }

    #[test]
    fn frame_after_partial_failure_repairs_cells_absent_from_its_normal_diff() {
        for failure in [Failure::Draw, Failure::Cursor, Failure::Flush] {
            let mut terminal =
                RatatuiTerminal::new(RecordingBackend::new(), TerminalSize::new(2, 1))
                    .expect("valid terminal size");
            draw_pair(&mut terminal, "a", " ").expect("baseline frame succeeds");

            terminal.writer.fail_once(failure);
            assert!(draw_pair(&mut terminal, "b", "d").is_err());
            draw_pair(&mut terminal, "a", "c").expect("different next frame repairs the surface");

            let repaired = terminal.writer().draws.last().expect("repair draw exists");
            assert!(
                repaired
                    .iter()
                    .any(|cell| cell.position == Position::new(0, 0) && cell.symbol == "a"),
                "unchanged baseline cell was not repaired after {failure:?}: {repaired:?}"
            );
            assert!(
                repaired
                    .iter()
                    .any(|cell| cell.position == Position::new(1, 0) && cell.symbol == "c"),
                "new cell was not drawn after {failure:?}: {repaired:?}"
            );
            assert_eq!(terminal.writer().clears, 2, "{failure:?}");
        }
    }

    #[test]
    fn failed_initial_clear_is_retried_before_any_cells_are_sent() {
        let mut backend = RecordingBackend::new();
        backend.fail_once(Failure::Clear);
        let mut terminal =
            RatatuiTerminal::new(backend, TerminalSize::new(2, 1)).expect("valid terminal size");

        assert!(draw_symbol(&mut terminal, "a").is_err());
        assert!(terminal.writer().draws.is_empty());
        draw_symbol(&mut terminal, "a").expect("clear and frame are retried");

        assert_eq!(terminal.writer().clears, 2);
        assert_eq!(terminal.writer().draws.len(), 1);
    }

    #[test]
    fn ratatui_cells_are_converted_at_the_backend_boundary() {
        let mut terminal = RatatuiTerminal::new(RecordingBackend::new(), TerminalSize::new(1, 1))
            .expect("valid terminal size");
        terminal
            .draw(|frame| {
                frame
                    .buffer_mut()
                    .cell_mut((0, 0))
                    .expect("test cell is in bounds")
                    .set_symbol("x")
                    .set_style(
                        Style::new()
                            .fg(RatatuiColor::LightRed)
                            .bg(RatatuiColor::Indexed(17))
                            .add_modifier(RatatuiModifier::BOLD | RatatuiModifier::UNDERLINED),
                    );
                frame.set_cursor(Some(Position::new(0, 0)));
            })
            .expect("frame succeeds");

        let cell = &terminal.writer().draws[0][0];
        assert_eq!(cell.position, Position::new(0, 0));
        assert_eq!(cell.symbol, "x");
        assert_eq!(cell.style.foreground, Some(Color::BRIGHT_RED));
        assert_eq!(cell.style.background, Some(Color::Ansi256(17)));
        assert!(cell.style.attributes.contains(TextAttribute::Bold));
        assert_eq!(cell.style.underline, Some(Underline::default()));
        assert_eq!(terminal.writer().cursors, [Some(Position::new(0, 0))]);
    }

    #[test]
    fn resize_invalidates_the_physical_surface_until_a_frame_commits() {
        let mut terminal = RatatuiTerminal::new(RecordingBackend::new(), TerminalSize::new(2, 1))
            .expect("valid terminal size");
        draw_symbol(&mut terminal, "a").expect("baseline frame succeeds");
        terminal
            .resize(TerminalSize::new(3, 1))
            .expect("new size is representable");

        draw_symbol(&mut terminal, "b").expect("resized frame succeeds");

        assert_eq!(terminal.size(), TerminalSize::new(3, 1));
        assert_eq!(terminal.writer().clears, 2);
    }
}
