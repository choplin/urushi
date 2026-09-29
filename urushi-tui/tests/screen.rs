use std::io;

use urushi::{Available, TextStyle, View, resolve};
use urushi_terminal::{Command, CommandWriter, Position, TerminalOutput, TerminalSize};
use urushi_tui::Screen;

#[derive(Default)]
struct RecordingWriter {
    printed: Vec<String>,
    cursor_visible: Vec<bool>,
    flushes: usize,
}

impl TerminalOutput for RecordingWriter {
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

impl CommandWriter for RecordingWriter {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        match command {
            Command::Print(text) => self.printed.push(text.as_str().to_owned()),
            Command::SetCursorVisible(visible) => self.cursor_visible.push(visible),
            _ => {}
        }
        Ok(())
    }
}

#[test]
fn caller_owned_loop_draws_without_the_runtime() {
    let resolved =
        resolve(&View::text("漆", TextStyle::new()), Available::size(2, 1)).expect("view resolves");
    let grapheme = &resolved.rows()[0][0];
    let mut screen = Screen::new(RecordingWriter::default(), TerminalSize::new(2, 1))
        .expect("screen size is valid");

    screen
        .draw(|frame| {
            assert_eq!(frame.area().size(), TerminalSize::new(2, 1));
            frame.put(0, 0, grapheme);
            frame.set_cursor(Some(Position::new(0, 0)));
        })
        .expect("frame is presented");

    let writer = screen.into_inner();
    assert_eq!(writer.printed, ["漆"]);
    assert_eq!(writer.cursor_visible, [true]);
    assert_eq!(writer.flushes, 1);
}
