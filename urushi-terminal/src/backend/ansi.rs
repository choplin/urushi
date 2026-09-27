//! ANSI/ECMA-48 encoding of Urushi terminal commands.

use std::io::{self, Write};

use crate::{
    ClearRegion, Color, Command, CommandWriter, CursorAppearance, CursorMove, TerminalOutput,
    TerminalStyle, TextAttribute, Underline, UnderlineStyle,
};

/// A backend that writes Urushi commands as ANSI, CSI, OSC, and C0 bytes.
///
/// This type owns only the output half of a terminal connection. Input parsing,
/// process-side raw mode, and platform queries are deliberately separate: an
/// interactive backend composes them around the same physical endpoint.
pub struct AnsiWriter<W> {
    writer: W,
}

impl<W> AnsiWriter<W> {
    pub const fn new(writer: W) -> Self {
        Self { writer }
    }

    pub const fn writer(&self) -> &W {
        &self.writer
    }

    pub fn writer_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> TerminalOutput for AnsiWriter<W> {
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W: Write> CommandWriter for AnsiWriter<W> {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        match command {
            Command::MoveCursor(movement) => write_cursor_move(&mut self.writer, movement),
            Command::SaveCursorPosition => self.writer.write_all(b"\x1b7"),
            Command::RestoreCursorPosition => self.writer.write_all(b"\x1b8"),
            Command::SetCursorVisible(enabled) => private_mode(&mut self.writer, 25, enabled),
            Command::SetCursorBlinking(enabled) => private_mode(&mut self.writer, 12, enabled),
            Command::SetCursorAppearance(appearance) => write!(
                self.writer,
                "\x1b[{} q",
                match appearance {
                    CursorAppearance::UserDefault => 0,
                    CursorAppearance::BlinkingBlock => 1,
                    CursorAppearance::SteadyBlock => 2,
                    CursorAppearance::BlinkingUnderline => 3,
                    CursorAppearance::SteadyUnderline => 4,
                    CursorAppearance::BlinkingBar => 5,
                    CursorAppearance::SteadyBar => 6,
                }
            ),
            Command::SetAlternateScreen(enabled) => private_mode(&mut self.writer, 1049, enabled),
            Command::SetBracketedPaste(enabled) => private_mode(&mut self.writer, 2004, enabled),
            Command::SetFocusReporting(enabled) => private_mode(&mut self.writer, 1004, enabled),
            Command::SetMouseCapture(enabled) => write_mouse_capture(&mut self.writer, enabled),
            Command::PushKeyboardEnhancement(flags) => {
                write!(self.writer, "\x1b[>{}u", flags.bits())
            }
            Command::PopKeyboardEnhancement => self.writer.write_all(b"\x1b[<1u"),
            Command::Clear(region) => write!(self.writer, "\x1b[{}", clear_sequence(region)),
            Command::Scroll(rows) if rows > 0 => write!(self.writer, "\x1b[{rows}S"),
            Command::Scroll(rows) if rows < 0 => {
                write!(self.writer, "\x1b[{}T", rows.unsigned_abs())
            }
            Command::Scroll(_) => Ok(()),
            Command::SetSize(size) => {
                write!(self.writer, "\x1b[8;{};{}t", size.rows(), size.columns())
            }
            Command::SetTitle(title) => {
                self.writer.write_all(b"\x1b]0;")?;
                self.writer.write_all(title.as_str().as_bytes())?;
                self.writer.write_all(b"\x1b\\")
            }
            Command::SetLineWrap(enabled) => private_mode(&mut self.writer, 7, enabled),
            Command::SetSynchronizedUpdate(enabled) => {
                private_mode(&mut self.writer, 2026, enabled)
            }
            Command::SetStyle(style) => write_style(&mut self.writer, style),
            Command::ResetStyle => self.writer.write_all(b"\x1b[0m"),
            Command::SetHyperlink(Some(link)) => {
                self.writer.write_all(b"\x1b]8;")?;
                for (index, parameter) in link.parameters.iter().enumerate() {
                    if index != 0 {
                        self.writer.write_all(b":")?;
                    }
                    write_osc_field(&mut self.writer, parameter.key, b":;=")?;
                    self.writer.write_all(b"=")?;
                    write_osc_field(&mut self.writer, parameter.value, b":;=")?;
                }
                self.writer.write_all(b";")?;
                write_osc_field(&mut self.writer, link.uri, b"")?;
                self.writer.write_all(b"\x1b\\")
            }
            Command::SetHyperlink(None) => self.writer.write_all(b"\x1b]8;;\x1b\\"),
            Command::Print(text) => self.writer.write_all(text.as_str().as_bytes()),
            Command::LineFeed => self.writer.write_all(b"\n"),
            Command::CarriageReturnLineFeed => self.writer.write_all(b"\r\n"),
        }
    }
}

fn private_mode(writer: &mut impl Write, mode: u16, enabled: bool) -> io::Result<()> {
    write!(writer, "\x1b[?{mode}{}", if enabled { 'h' } else { 'l' })
}

fn write_mouse_capture(writer: &mut impl Write, enabled: bool) -> io::Result<()> {
    let suffix = if enabled { 'h' } else { 'l' };
    for mode in [1000, 1002, 1003, 1006] {
        write!(writer, "\x1b[?{mode}{suffix}")?;
    }
    Ok(())
}

fn write_cursor_move(writer: &mut impl Write, movement: CursorMove) -> io::Result<()> {
    match movement {
        CursorMove::To(position) => write!(
            writer,
            "\x1b[{};{}H",
            position.row().saturating_add(1),
            position.column().saturating_add(1)
        ),
        CursorMove::ToColumn(column) => write!(writer, "\x1b[{}G", column.saturating_add(1)),
        CursorMove::ToRow(row) => write!(writer, "\x1b[{}d", row.saturating_add(1)),
        CursorMove::By { columns, rows } => {
            write_signed_move(writer, rows, 'B', 'A')?;
            write_signed_move(writer, columns, 'C', 'D')
        }
        CursorMove::ToNextLine(lines) => write!(writer, "\x1b[{lines}E"),
        CursorMove::ToPreviousLine(lines) => write!(writer, "\x1b[{lines}F"),
    }
}

fn write_signed_move(
    writer: &mut impl Write,
    amount: i32,
    positive: char,
    negative: char,
) -> io::Result<()> {
    match amount.cmp(&0) {
        std::cmp::Ordering::Greater => write!(writer, "\x1b[{amount}{positive}"),
        std::cmp::Ordering::Less => write!(writer, "\x1b[{}{negative}", amount.unsigned_abs()),
        std::cmp::Ordering::Equal => Ok(()),
    }
}

const fn clear_sequence(region: ClearRegion) -> &'static str {
    match region {
        ClearRegion::Screen => "2J",
        ClearRegion::ScreenAndScrollback => "3J",
        ClearRegion::BeforeCursor => "1J",
        ClearRegion::AfterCursor => "0J",
        ClearRegion::Line => "2K",
        ClearRegion::AfterCursorInLine => "0K",
    }
}

fn write_style(writer: &mut impl Write, style: TerminalStyle) -> io::Result<()> {
    let mut sequence = io::Cursor::new([0_u8; 128]);
    sequence.write_all(b"\x1b[0")?;
    if let Some(color) = style.foreground {
        write_color_parameter(&mut sequence, 38, color)?;
    }
    if let Some(color) = style.background {
        write_color_parameter(&mut sequence, 48, color)?;
    }
    for attribute in style.attributes {
        let sgr = match attribute {
            TextAttribute::Bold => 1,
            TextAttribute::Dim => 2,
            TextAttribute::Italic => 3,
            TextAttribute::SlowBlink => 5,
            TextAttribute::RapidBlink => 6,
            TextAttribute::Reversed => 7,
            TextAttribute::Hidden => 8,
            TextAttribute::CrossedOut => 9,
            TextAttribute::Fraktur => 20,
            TextAttribute::Framed => 51,
            TextAttribute::Encircled => 52,
            TextAttribute::Overlined => 53,
        };
        write!(sequence, ";{sgr}")?;
    }
    if let Some(underline) = style.underline {
        write_underline_parameters(&mut sequence, underline)?;
    }
    sequence.write_all(b"m")?;

    let length = sequence.position() as usize;
    writer.write_all(&sequence.get_ref()[..length])
}

fn write_underline_parameters(writer: &mut impl Write, underline: Underline) -> io::Result<()> {
    let sgr = match underline.get_style() {
        UnderlineStyle::Single => "4",
        UnderlineStyle::Double => "4:2",
        UnderlineStyle::Curly => "4:3",
        UnderlineStyle::Dotted => "4:4",
        UnderlineStyle::Dashed => "4:5",
    };
    write!(writer, ";{sgr}")?;
    if let Some(color) = underline.get_color() {
        write_color_parameter(writer, 58, color)?;
    }
    Ok(())
}

fn write_color_parameter(writer: &mut impl Write, channel: u8, color: Color) -> io::Result<()> {
    match color {
        Color::Ansi(index @ 0..=7) => {
            let base = if channel == 38 {
                30
            } else if channel == 48 {
                40
            } else {
                channel
            };
            if channel == 58 {
                write!(writer, ";58;5;{index}")
            } else {
                write!(writer, ";{}", base + index)
            }
        }
        Color::Ansi(index @ 8..=15) if channel != 58 => {
            let base = if channel == 38 { 90 } else { 100 };
            write!(writer, ";{}", base + index - 8)
        }
        Color::Ansi(index) | Color::Ansi256(index) => {
            write!(writer, ";{channel};5;{index}")
        }
        Color::Rgb(red, green, blue) => {
            write!(writer, ";{channel};2;{red};{green};{blue}")
        }
    }
}

fn write_osc_field(writer: &mut impl Write, value: &str, separators: &[u8]) -> io::Result<()> {
    for byte in value.bytes() {
        if byte < 0x20 || byte == 0x7f || separators.contains(&byte) {
            write!(writer, "%{byte:02X}")?;
        } else {
            writer.write_all(&[byte])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HyperlinkParameter, Position, TerminalHyperlink, TerminalText, TextAttributes};

    #[derive(Default)]
    struct CountingWriter {
        bytes: Vec<u8>,
        writes: usize,
    }

    impl Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn commands_encode_without_crossterm_types() {
        let mut output = AnsiWriter::new(Vec::new());
        output
            .write_command(Command::MoveCursor(CursorMove::To(Position::new(3, 2))))
            .expect("move encodes");
        output
            .write_command(Command::SetAlternateScreen(true))
            .expect("mode encodes");
        output
            .write_command(Command::Print(
                TerminalText::try_from("漆").expect("printable text"),
            ))
            .expect("text encodes");

        assert_eq!(output.into_inner(), b"\x1b[3;4H\x1b[?1049h\xe6\xbc\x86");
    }

    #[test]
    fn hyperlink_fields_cannot_break_the_osc_structure() {
        let parameters = [HyperlinkParameter {
            key: "id:semicolon",
            value: "a=b",
        }];
        let mut output = AnsiWriter::new(Vec::new());
        output
            .write_command(Command::SetHyperlink(Some(TerminalHyperlink {
                uri: "https://example.test/a;b",
                parameters: &parameters,
            })))
            .expect("hyperlink encodes");

        assert_eq!(
            output.into_inner(),
            b"\x1b]8;id%3Asemicolon=a%3Db;https://example.test/a;b\x1b\\"
        );
    }

    #[test]
    fn complete_style_is_one_sgr_sequence_and_one_output_write() {
        let style = TerminalStyle {
            foreground: Some(Color::Rgb(1, 2, 3)),
            background: Some(Color::Rgb(4, 5, 6)),
            attributes: TextAttributes::all(),
            underline: Some(Underline::new(UnderlineStyle::Dashed).color((7, 8, 9))),
        };
        let mut output = AnsiWriter::new(CountingWriter::default());

        output
            .write_command(Command::SetStyle(style))
            .expect("style encodes");

        let writer = output.into_inner();
        assert_eq!(writer.writes, 1);
        assert_eq!(
            writer.bytes,
            b"\x1b[0;38;2;1;2;3;48;2;4;5;6;1;2;3;5;6;7;8;9;20;51;52;53;4:5;58;2;7;8;9m"
        );
    }
}
