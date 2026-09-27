//! Backend-independent terminal output commands.

use std::{error::Error, fmt, io};

use crate::{KeyboardEnhancementFlags, Position, TerminalSize, TerminalStyle};

/// Printable terminal text containing no control characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerminalText<'a>(&'a str);

impl<'a> TerminalText<'a> {
    pub const fn as_str(self) -> &'a str {
        self.0
    }
}

impl<'a> TryFrom<&'a str> for TerminalText<'a> {
    type Error = InvalidTerminalText;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        match text
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            Some((byte_offset, _)) => Err(InvalidTerminalText { byte_offset }),
            None => Ok(Self(text)),
        }
    }
}

/// The location of a control character rejected from terminal text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct InvalidTerminalText {
    byte_offset: usize,
}

impl InvalidTerminalText {
    pub const fn byte_offset(self) -> usize {
        self.byte_offset
    }
}

impl fmt::Display for InvalidTerminalText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "terminal text contains a control character at byte {}",
            self.byte_offset
        )
    }
}

impl Error for InvalidTerminalText {}

/// A terminal output sink whose queued operations can be made visible.
pub trait TerminalOutput {
    fn flush(&mut self) -> io::Result<()>;
}

impl<T: TerminalOutput + ?Sized> TerminalOutput for &mut T {
    fn flush(&mut self) -> io::Result<()> {
        T::flush(self)
    }
}

/// A cursor movement expressed independently of an escape-sequence API.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorMove {
    To(Position),
    ToColumn(usize),
    ToRow(usize),
    By { columns: i32, rows: i32 },
    ToNextLine(usize),
    ToPreviousLine(usize),
}

/// The cursor appearance requested from the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CursorAppearance {
    UserDefault,
    BlinkingBlock,
    SteadyBlock,
    BlinkingUnderline,
    SteadyUnderline,
    BlinkingBar,
    SteadyBar,
}

/// A logical region of the terminal buffer to clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClearRegion {
    Screen,
    ScreenAndScrollback,
    BeforeCursor,
    AfterCursor,
    Line,
    AfterCursorInLine,
}

/// One OSC 8 hyperlink parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HyperlinkParameter<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

/// A hyperlink attached to subsequently printed text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerminalHyperlink<'a> {
    pub uri: &'a str,
    pub parameters: &'a [HyperlinkParameter<'a>],
}

/// One semantic terminal control or text-output operation.
///
/// Commands model terminal behavior rather than a backend library's command
/// types. A surface owns when commands run; a backend owns how they reach the
/// terminal. Their conventional wire forms belong to the ANSI family: CSI
/// sequences control cursor, modes, erasure, and SGR style; OSC strings carry
/// titles and hyperlinks; printable UTF-8 and C0 line endings are raw bytes.
/// An adapter may use native platform operations instead of those encodings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Command<'a> {
    /// Moves the cursor using a CSI cursor-positioning or movement sequence.
    MoveCursor(CursorMove),
    /// Saves the cursor position using the backend's ANSI/DEC save sequence.
    SaveCursorPosition,
    /// Restores the cursor position using the backend's ANSI/DEC restore sequence.
    RestoreCursorPosition,
    /// Shows or hides the cursor using a CSI private-mode sequence.
    SetCursorVisible(bool),
    /// Enables or disables cursor blinking using a CSI private-mode sequence.
    SetCursorBlinking(bool),
    /// Selects a cursor shape using a CSI cursor-style sequence.
    SetCursorAppearance(CursorAppearance),
    /// Selects the primary or alternate screen using a CSI private-mode sequence.
    SetAlternateScreen(bool),
    /// Enables or disables bracketed-paste reporting using a CSI private-mode sequence.
    SetBracketedPaste(bool),
    /// Enables or disables terminal-focus reporting using a CSI private-mode sequence.
    SetFocusReporting(bool),
    /// Enables or disables mouse reporting using CSI private-mode sequences.
    SetMouseCapture(bool),
    /// Pushes enhanced-keyboard flags using the terminal's CSI protocol.
    PushKeyboardEnhancement(KeyboardEnhancementFlags),
    /// Pops the most recently pushed enhanced-keyboard flags using CSI.
    PopKeyboardEnhancement,
    /// Clears terminal content using a CSI erase sequence.
    Clear(ClearRegion),
    /// Scrolls by signed rows. Positive values move content upward.
    ///
    /// Backends normally lower this to CSI scroll-up or scroll-down sequences.
    Scroll(i32),
    /// Requests a terminal size using a CSI window-manipulation sequence.
    SetSize(TerminalSize),
    /// Sets the terminal title using an OSC control string.
    SetTitle(TerminalText<'a>),
    /// Enables or disables automatic line wrapping using a CSI private-mode sequence.
    SetLineWrap(bool),
    /// Begins or ends a synchronized update using a CSI private-mode sequence.
    SetSynchronizedUpdate(bool),
    /// Applies a complete physical text style using CSI SGR sequences.
    SetStyle(TerminalStyle),
    /// Restores the terminal's default text style using CSI SGR sequences.
    ResetStyle,
    /// Starts or ends a hyperlink using an OSC 8 control string.
    SetHyperlink(Option<TerminalHyperlink<'a>>),
    /// Writes the UTF-8 text bytes without interpreting them as terminal control data.
    Print(TerminalText<'a>),
    /// Writes the raw C0 line-feed byte without a carriage return.
    LineFeed,
    /// Writes raw C0 carriage-return and line-feed bytes.
    CarriageReturnLineFeed,
}

/// Writes backend-independent terminal commands in caller-selected order.
pub trait CommandWriter: TerminalOutput {
    /// Queues or writes one command.
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()>;
}

impl<T: CommandWriter + ?Sized> CommandWriter for &mut T {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        T::write_command(self, command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_text_keeps_commands_out_of_printable_payloads() {
        assert_eq!(
            TerminalText::try_from("漆")
                .expect("text is printable")
                .as_str(),
            "漆"
        );
        assert_eq!(
            TerminalText::try_from("safe\u{1b}[2J")
                .expect_err("escape is rejected")
                .byte_offset(),
            4
        );
        assert!(TerminalText::try_from("line\nfeed").is_err());
    }
}
