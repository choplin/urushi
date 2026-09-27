//! Low-level full-screen presentation over terminal command primitives.

use std::io;

use unicode_width::UnicodeWidthStr;
use urushi_terminal::{
    ClearRegion, Command, CommandWriter, CursorMove, Position, TerminalOutput, TerminalSize,
    TerminalStyle, TerminalText,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    origin: Position,
    size: TerminalSize,
}

impl Rect {
    pub const fn new(origin: Position, size: TerminalSize) -> Self {
        Self { origin, size }
    }

    pub const fn from_size(size: TerminalSize) -> Self {
        Self::new(Position::new(0, 0), size)
    }

    pub const fn origin(self) -> Position {
        self.origin
    }

    pub const fn size(self) -> TerminalSize {
        self.size
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell<'a> {
    pub symbol: &'a str,
    pub style: TerminalStyle,
}

/// Writes positioned cells through terminal command primitives.
///
/// The blanket implementation coalesces adjacent cells and repeated styles
/// before it emits backend-independent [`Command`] values.
pub trait CellWriter: TerminalOutput {
    fn draw<'a, I>(&mut self, cells: I) -> io::Result<()>
    where
        I: Iterator<Item = (Position, Cell<'a>)>;

    fn set_cursor(&mut self, cursor: Option<Position>) -> io::Result<()>;

    fn clear(&mut self) -> io::Result<()>;
}

impl<W: CommandWriter + ?Sized> CellWriter for W {
    fn draw<'a, I>(&mut self, cells: I) -> io::Result<()>
    where
        I: Iterator<Item = (Position, Cell<'a>)>,
    {
        let mut cells = cells.peekable();
        if cells.peek().is_none() {
            return Ok(());
        }

        self.write_command(Command::ResetStyle)?;
        let mut style = TerminalStyle::default();
        let mut next_position = None;
        for (position, cell) in cells {
            let text = TerminalText::try_from(cell.symbol)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
            if next_position != Some(position) {
                self.write_command(Command::MoveCursor(CursorMove::To(position)))?;
            }
            if style != cell.style {
                self.write_command(Command::SetStyle(cell.style))?;
                style = cell.style;
            }
            self.write_command(Command::Print(text))?;
            next_position = Some(Position::new(
                position
                    .column()
                    .saturating_add(UnicodeWidthStr::width(cell.symbol)),
                position.row(),
            ));
        }
        self.write_command(Command::ResetStyle)
    }

    fn set_cursor(&mut self, cursor: Option<Position>) -> io::Result<()> {
        self.write_command(Command::SetCursorVisible(cursor.is_some()))?;
        if let Some(position) = cursor {
            self.write_command(Command::MoveCursor(CursorMove::To(position)))?;
        }
        Ok(())
    }

    fn clear(&mut self) -> io::Result<()> {
        self.write_command(Command::Clear(ClearRegion::Screen))?;
        self.write_command(Command::MoveCursor(CursorMove::To(Position::new(0, 0))))
    }
}

/// Provides draw-scoped access without exposing presentation history or commit.
pub trait Frame {
    type Cell: ?Sized;

    fn area(&self) -> Rect;

    fn put(&mut self, column: usize, row: usize, cell: &Self::Cell);

    fn set_cursor(&mut self, at: Option<Position>);
}

/// Full-screen presentation state and transactional frame commit behavior.
pub trait Terminal {
    type Cell: ?Sized;
    type Frame<'a>: Frame<Cell = Self::Cell>;

    fn size(&self) -> TerminalSize;

    fn resize(&mut self, size: TerminalSize) -> io::Result<()>;

    fn draw(&mut self, draw: impl FnOnce(&mut Self::Frame<'_>)) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use urushi::Color;

    #[derive(Debug, PartialEq, Eq)]
    enum Recorded {
        Move(Position),
        Style(TerminalStyle),
        ResetStyle,
        Print(String),
    }

    #[derive(Default)]
    struct RecordingCommands {
        commands: Vec<Recorded>,
    }

    impl TerminalOutput for RecordingCommands {
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl CommandWriter for RecordingCommands {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            match command {
                Command::MoveCursor(CursorMove::To(position)) => {
                    self.commands.push(Recorded::Move(position));
                }
                Command::SetStyle(style) => self.commands.push(Recorded::Style(style)),
                Command::ResetStyle => self.commands.push(Recorded::ResetStyle),
                Command::Print(text) => {
                    self.commands
                        .push(Recorded::Print(text.as_str().to_owned()));
                }
                _ => panic!("unexpected cell command: {command:?}"),
            }
            Ok(())
        }
    }

    #[test]
    fn adjacent_equal_style_cells_share_position_and_style_commands() {
        let style = TerminalStyle {
            foreground: Some(Color::BRIGHT_RED),
            ..TerminalStyle::default()
        };
        let mut writer = RecordingCommands::default();

        CellWriter::draw(
            &mut writer,
            [
                (
                    Position::new(0, 0),
                    Cell {
                        symbol: "界",
                        style,
                    },
                ),
                (Position::new(2, 0), Cell { symbol: "b", style }),
            ]
            .into_iter(),
        )
        .expect("cell draw succeeds");

        assert_eq!(
            writer.commands,
            [
                Recorded::ResetStyle,
                Recorded::Move(Position::new(0, 0)),
                Recorded::Style(style),
                Recorded::Print("界".to_owned()),
                Recorded::Print("b".to_owned()),
                Recorded::ResetStyle,
            ]
        );
    }

    #[test]
    fn cell_text_rejects_terminal_control_data() {
        let mut writer = RecordingCommands::default();

        let error = CellWriter::draw(
            &mut writer,
            [(
                Position::new(0, 0),
                Cell {
                    symbol: "x\n",
                    style: TerminalStyle::default(),
                },
            )]
            .into_iter(),
        )
        .expect_err("control data is rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
