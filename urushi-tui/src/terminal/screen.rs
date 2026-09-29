//! Frame state, diffing, and transactional presentation.

use std::io;

use urushi::StyledGrapheme;
use urushi_terminal::{CommandWriter, Position, TerminalSize};

use super::Rect;
use super::output::CellWriter;
use crate::cell::{Buffer, BufferSizeError, CellWriteError};

/// A synchronous full-screen presentation engine.
///
/// `Screen` owns the working and committed cell buffers, but it does not own a
/// terminal session or any input path. A caller may use it directly in its own
/// loop with any [`CommandWriter`], independently of the optional Urushi TEA
/// runtime and Tokio.
///
/// A draw is transactional with respect to the committed baseline: changed
/// cells, the cursor request, and the writer flush must all succeed before the
/// working frame becomes committed. If output fails after an arbitrary prefix,
/// the next draw clears the physical surface and reconstructs it from a blank
/// baseline.
pub struct Screen<W> {
    writer: W,
    committed: Buffer,
    working: Buffer,
    needs_clear: bool,
}

impl<W: CommandWriter> Screen<W> {
    /// Creates a screen for a terminal surface of `size`.
    pub fn new(writer: W, size: TerminalSize) -> io::Result<Self> {
        let committed = Buffer::new(size).map_err(buffer_size_error)?;
        let working = Buffer::new(size).map_err(buffer_size_error)?;
        Ok(Self {
            writer,
            committed,
            working,
            needs_clear: true,
        })
    }

    /// Returns the current frame size.
    pub const fn size(&self) -> TerminalSize {
        self.working.size()
    }

    /// Returns the command writer used for physical output.
    pub const fn writer(&self) -> &W {
        &self.writer
    }

    /// Consumes the screen and returns its command writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Replaces both frame buffers and invalidates the physical baseline.
    pub fn resize(&mut self, size: TerminalSize) -> io::Result<()> {
        let working = Buffer::new(size).map_err(buffer_size_error)?;
        self.committed.resize(size).map_err(buffer_size_error)?;
        self.working = working;
        self.needs_clear = true;
        Ok(())
    }

    /// Builds and presents one frame synchronously.
    ///
    /// The closure may only change the working cells and cursor request through
    /// its borrowed [`Frame`]. Presentation history and commit remain owned by
    /// the screen.
    pub fn draw(&mut self, draw: impl FnOnce(&mut Frame<'_>)) -> io::Result<()> {
        self.working.reset();
        let mut frame = Frame {
            buffer: &mut self.working,
            area: Rect::from_size(self.committed.size()),
            cursor: None,
        };
        draw(&mut frame);
        let cursor = frame.cursor;

        let blank;
        let baseline = if self.needs_clear {
            blank = Buffer::new(self.working.size()).map_err(buffer_size_error)?;
            &blank
        } else {
            &self.committed
        };
        let changes = baseline.diff(&self.working).map_err(io::Error::other)?;
        let result = (|| {
            if self.needs_clear {
                self.writer.clear()?;
            }
            self.writer.draw(changes)?;
            self.writer.set_cursor(cursor)?;
            self.writer.flush()
        })();

        if let Err(error) = result {
            self.needs_clear = true;
            return Err(error);
        }

        std::mem::swap(&mut self.committed, &mut self.working);
        self.needs_clear = false;
        Ok(())
    }
}

fn buffer_size_error(error: BufferSizeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

/// Draw-scoped access to a screen's working presentation state.
///
/// A frame cannot write commands, flush, or commit. Cells that have zero width
/// or do not fit completely inside the frame are ignored.
pub struct Frame<'a> {
    buffer: &'a mut Buffer,
    area: Rect,
    cursor: Option<Position>,
}

impl Frame<'_> {
    /// Returns the frame's complete drawable area.
    pub const fn area(&self) -> Rect {
        self.area
    }

    /// Places one styled grapheme at a cell position.
    pub fn put(&mut self, column: usize, row: usize, cell: &StyledGrapheme) {
        match self.buffer.write(column, row, cell) {
            Ok(()) | Err(CellWriteError::ZeroWidth) | Err(CellWriteError::OutOfBounds { .. }) => {}
        }
    }

    /// Requests a visible cursor position, or hides the cursor with `None`.
    pub fn set_cursor(&mut self, at: Option<Position>) {
        self.cursor = at;
    }
}

#[cfg(feature = "runtime")]
pub(crate) trait RenderFrame {
    fn area(&self) -> Rect;
    fn put(&mut self, column: usize, row: usize, cell: &StyledGrapheme);
    fn set_cursor(&mut self, at: Option<Position>);
}

#[cfg(feature = "runtime")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use urushi::{Available, Color, TextStyle, View, resolve};
    use urushi_terminal::{ClearRegion, Command, CursorMove, TerminalOutput, TerminalStyle};

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Failure {
        Clear,
        Print,
        Cursor,
        Flush,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum Recorded {
        Clear,
        Move(Position),
        CursorVisible(bool),
        Style(TerminalStyle),
        ResetStyle,
        Print(String),
        Flush,
    }

    #[derive(Default)]
    struct RecordingCommands {
        commands: Vec<Recorded>,
        failure: Option<Failure>,
        reset_failure_countdown: Option<usize>,
    }

    impl RecordingCommands {
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

        fn fail_reset_after(&mut self, successful_resets: usize) {
            self.reset_failure_countdown = Some(successful_resets);
        }

        fn clear_count(&self) -> usize {
            self.commands
                .iter()
                .filter(|command| matches!(command, Recorded::Clear))
                .count()
        }
    }

    impl TerminalOutput for RecordingCommands {
        fn flush(&mut self) -> io::Result<()> {
            self.commands.push(Recorded::Flush);
            self.take_failure(Failure::Flush)
        }
    }

    impl CommandWriter for RecordingCommands {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            match command {
                Command::MoveCursor(CursorMove::To(position)) => {
                    self.commands.push(Recorded::Move(position));
                }
                Command::SetCursorVisible(visible) => {
                    self.commands.push(Recorded::CursorVisible(visible));
                    self.take_failure(Failure::Cursor)?;
                }
                Command::SetStyle(style) => self.commands.push(Recorded::Style(style)),
                Command::ResetStyle => {
                    self.commands.push(Recorded::ResetStyle);
                    if let Some(countdown) = &mut self.reset_failure_countdown {
                        if *countdown == 0 {
                            self.reset_failure_countdown = None;
                            return Err(io::Error::other("planned terminal failure"));
                        }
                        *countdown -= 1;
                    }
                }
                Command::Print(text) => {
                    self.commands
                        .push(Recorded::Print(text.as_str().to_owned()));
                    self.take_failure(Failure::Print)?;
                }
                Command::Clear(ClearRegion::Screen) => {
                    self.commands.push(Recorded::Clear);
                    self.take_failure(Failure::Clear)?;
                }
                _ => panic!("unexpected cell command: {command:?}"),
            }
            Ok(())
        }
    }

    fn grapheme(symbol: &str) -> StyledGrapheme {
        resolve(&View::text(symbol, TextStyle::new()), Available::size(2, 1))
            .expect("test view resolves")
            .rows()[0][0]
            .clone()
    }

    fn draw_pair(
        screen: &mut Screen<RecordingCommands>,
        left: &StyledGrapheme,
        right: &StyledGrapheme,
    ) -> io::Result<()> {
        screen.draw(|frame| {
            frame.put(0, 0, left);
            frame.put(1, 0, right);
        })
    }

    #[test]
    fn successful_draw_commits_the_incremental_baseline() {
        let mut screen = Screen::new(RecordingCommands::default(), TerminalSize::new(2, 1))
            .expect("screen size is valid");
        let a = grapheme("a");
        let b = grapheme("b");
        let c = grapheme("c");
        draw_pair(&mut screen, &a, &b).expect("initial frame succeeds");
        let next = screen.writer().commands.len();

        draw_pair(&mut screen, &a, &c).expect("incremental frame succeeds");

        let prints = screen.writer().commands[next..]
            .iter()
            .filter_map(|command| match command {
                Recorded::Print(symbol) => Some(symbol.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(prints, ["c"]);
        assert_eq!(screen.writer().clear_count(), 1);
    }

    #[test]
    fn any_partial_output_failure_forces_a_complete_repair() {
        for failure in [Failure::Print, Failure::Cursor, Failure::Flush] {
            let mut screen = Screen::new(RecordingCommands::default(), TerminalSize::new(2, 1))
                .expect("screen size is valid");
            let a = grapheme("a");
            let b = grapheme("b");
            let c = grapheme("c");
            let d = grapheme("d");
            draw_pair(&mut screen, &a, &b).expect("baseline frame succeeds");

            screen.writer.fail_once(failure);
            assert!(draw_pair(&mut screen, &c, &d).is_err());
            let repair = screen.writer().commands.len();
            draw_pair(&mut screen, &a, &c).expect("repair frame succeeds");

            let repaired = &screen.writer().commands[repair..];
            assert!(repaired.starts_with(&[Recorded::ResetStyle, Recorded::Clear]));
            let prints = repaired
                .iter()
                .filter_map(|command| match command {
                    Recorded::Print(symbol) => Some(symbol.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(prints, ["a", "c"], "{failure:?}");
            assert_eq!(screen.writer().clear_count(), 2, "{failure:?}");
        }
    }

    #[test]
    fn failed_clear_is_retried_before_cells_are_sent() {
        let mut writer = RecordingCommands::default();
        writer.fail_once(Failure::Clear);
        let mut screen =
            Screen::new(writer, TerminalSize::new(1, 1)).expect("screen size is valid");
        let a = grapheme("a");

        assert!(screen.draw(|frame| frame.put(0, 0, &a)).is_err());
        assert!(
            !screen
                .writer()
                .commands
                .iter()
                .any(|command| matches!(command, Recorded::Print(_)))
        );
        screen
            .draw(|frame| frame.put(0, 0, &a))
            .expect("clear and frame are retried");

        assert_eq!(screen.writer().clear_count(), 2);
    }

    #[test]
    fn empty_repair_resets_style_before_clearing() {
        let mut screen = Screen::new(RecordingCommands::default(), TerminalSize::new(1, 1))
            .expect("screen size is valid");
        let styled = resolve(
            &View::text("x", TextStyle::new().background(Color::BLUE)),
            Available::size(1, 1),
        )
        .expect("test view resolves")
        .rows()[0][0]
            .clone();
        screen.draw(|_| {}).expect("baseline frame succeeds");

        screen.writer.fail_reset_after(1);
        assert!(screen.draw(|frame| frame.put(0, 0, &styled)).is_err());
        let repair = screen.writer().commands.len();
        screen.draw(|_| {}).expect("empty repair frame succeeds");

        assert!(
            screen.writer().commands[repair..]
                .starts_with(&[Recorded::ResetStyle, Recorded::Clear])
        );
    }

    #[test]
    fn resize_discards_the_old_diff_baseline() {
        let mut screen = Screen::new(RecordingCommands::default(), TerminalSize::new(2, 1))
            .expect("screen size is valid");
        let a = grapheme("a");
        screen
            .draw(|frame| frame.put(0, 0, &a))
            .expect("initial frame succeeds");

        screen
            .resize(TerminalSize::new(3, 1))
            .expect("new size is valid");
        screen
            .draw(|frame| frame.put(0, 0, &a))
            .expect("resized frame succeeds");

        assert_eq!(screen.size(), TerminalSize::new(3, 1));
        assert_eq!(screen.writer().clear_count(), 2);
    }

    #[test]
    fn frame_exposes_area_and_cursor_without_owning_output() {
        let mut screen = Screen::new(RecordingCommands::default(), TerminalSize::new(2, 1))
            .expect("screen size is valid");
        let wide = grapheme("界");

        screen
            .draw(|frame| {
                assert_eq!(frame.area(), Rect::from_size(TerminalSize::new(2, 1)));
                frame.put(1, 0, &wide);
                frame.set_cursor(Some(Position::new(1, 0)));
            })
            .expect("out-of-bounds cells are ignored");

        assert!(
            screen
                .writer()
                .commands
                .contains(&Recorded::CursorVisible(true))
        );
        assert!(
            screen
                .writer()
                .commands
                .contains(&Recorded::Move(Position::new(1, 0)))
        );
    }
}
