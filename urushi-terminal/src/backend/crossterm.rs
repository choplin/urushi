//! Crossterm implementation of Urushi's terminal backend capabilities.

use std::{
    io::{self, IsTerminal, Write},
    time::Duration,
};

use crate::{
    ClearRegion, Color, Command, CommandWriter, CursorAppearance, CursorMove, Event, EventSource,
    FocusChange, KeyCode, KeyEvent, KeyEventState, KeyKind, KeyboardEnhancementFlags,
    KeyboardEnhancementQuery, MediaKeyCode, ModifierKeyCode, Modifiers, MouseButton, MouseEvent,
    MouseKind, PixelSize, Position, RawModeControl, TerminalOutput, TerminalQuery, TerminalSize,
    TerminalStyle, TextAttribute, TextAttributes, Underline, UnderlineStyle, WindowSize,
    command::write_control_string,
};
use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
        EnableFocusChange, EnableMouseCapture, Event as CrosstermEvent,
        KeyCode as CrosstermKeyCode, KeyEventKind, KeyEventState as CrosstermKeyEventState,
        KeyModifiers, KeyboardEnhancementFlags as CrosstermKeyboardEnhancementFlags,
        MediaKeyCode as CrosstermMediaKeyCode, ModifierKeyCode as CrosstermModifierKeyCode,
        MouseButton as CrosstermMouseButton, MouseEventKind, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    queue,
    style::{
        Attribute, Color as CrosstermColor, Print, ResetColor, SetAttribute, SetBackgroundColor,
        SetForegroundColor, SetUnderlineColor,
    },
    terminal::{
        self, BeginSynchronizedUpdate, Clear, ClearType, DisableLineWrap, EnableLineWrap,
        EndSynchronizedUpdate, EnterAlternateScreen, LeaveAlternateScreen, ScrollDown, ScrollUp,
        SetSize, SetTitle,
    },
};

/// The initial terminal backend, implemented with crossterm behind this module.
pub struct CrosstermBackend<W> {
    writer: W,
}

impl<W> CrosstermBackend<W> {
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

impl<W: Write> CommandWriter for CrosstermBackend<W> {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        match command {
            Command::MoveCursor(movement) => write_cursor_move(&mut self.writer, movement),
            Command::SaveCursorPosition => queue!(self.writer, cursor::SavePosition),
            Command::RestoreCursorPosition => queue!(self.writer, cursor::RestorePosition),
            Command::SetCursorVisible(true) => queue!(self.writer, cursor::Show),
            Command::SetCursorVisible(false) => queue!(self.writer, cursor::Hide),
            Command::SetCursorBlinking(true) => queue!(self.writer, cursor::EnableBlinking),
            Command::SetCursorBlinking(false) => queue!(self.writer, cursor::DisableBlinking),
            Command::SetCursorAppearance(appearance) => {
                queue!(self.writer, convert_cursor_appearance(appearance))
            }
            Command::SetAlternateScreen(true) => queue!(self.writer, EnterAlternateScreen),
            Command::SetAlternateScreen(false) => queue!(self.writer, LeaveAlternateScreen),
            Command::SetBracketedPaste(true) => queue!(self.writer, EnableBracketedPaste),
            Command::SetBracketedPaste(false) => queue!(self.writer, DisableBracketedPaste),
            Command::SetFocusReporting(true) => queue!(self.writer, EnableFocusChange),
            Command::SetFocusReporting(false) => queue!(self.writer, DisableFocusChange),
            Command::SetMouseCapture(true) => queue!(self.writer, EnableMouseCapture),
            Command::SetMouseCapture(false) => queue!(self.writer, DisableMouseCapture),
            Command::PushKeyboardEnhancement(flags) => queue!(
                self.writer,
                PushKeyboardEnhancementFlags(convert_keyboard_enhancement_flags(flags))
            ),
            Command::PopKeyboardEnhancement => {
                queue!(self.writer, PopKeyboardEnhancementFlags)
            }
            Command::Clear(region) => queue!(self.writer, Clear(convert_clear_region(region))),
            Command::Scroll(rows) if rows > 0 => {
                queue!(self.writer, ScrollUp(offset(rows.unsigned_abs())?))
            }
            Command::Scroll(rows) if rows < 0 => {
                queue!(self.writer, ScrollDown(offset(rows.unsigned_abs())?))
            }
            Command::Scroll(_) => Ok(()),
            Command::SetSize(size) => queue!(
                self.writer,
                SetSize(coordinate(size.columns())?, coordinate(size.rows())?)
            ),
            Command::SetTitle(title) => queue!(self.writer, SetTitle(title.as_str())),
            Command::SetLineWrap(true) => queue!(self.writer, EnableLineWrap),
            Command::SetLineWrap(false) => queue!(self.writer, DisableLineWrap),
            Command::SetSynchronizedUpdate(true) => queue!(self.writer, BeginSynchronizedUpdate),
            Command::SetSynchronizedUpdate(false) => queue!(self.writer, EndSynchronizedUpdate),
            Command::SetStyle(style) => write_style(&mut self.writer, style),
            Command::ResetStyle => queue!(self.writer, SetAttribute(Attribute::Reset), ResetColor),
            Command::SetHyperlink(Some(link)) => write_hyperlink_start(&mut self.writer, link),
            Command::SetHyperlink(None) => self.writer.write_all(b"\x1b]8;;\x1b\\"),
            Command::ApplicationProgram(payload) => {
                write_control_string(&mut self.writer, b"\x1b_", payload)
            }
            Command::DeviceControl(payload) => {
                write_control_string(&mut self.writer, b"\x1bP", payload)
            }
            Command::LineFeed => self.writer.write_all(b"\n"),
            Command::CarriageReturnLineFeed => self.writer.write_all(b"\r\n"),
            Command::Print(text) => queue!(self.writer, Print(text.as_str())),
        }
    }
}

impl<W: Write> TerminalOutput for CrosstermBackend<W> {
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl<W: Write + IsTerminal> RawModeControl for CrosstermBackend<W> {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal() && self.writer.is_terminal()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }
}

impl<W> TerminalQuery for CrosstermBackend<W> {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        let (columns, rows) = terminal::size()?;
        Ok(TerminalSize::new(usize::from(columns), usize::from(rows)))
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        let (column, row) = cursor::position()?;
        Ok(Position::new(usize::from(column), usize::from(row)))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        match terminal::window_size() {
            Ok(size) => Ok(convert_window_size(size)),
            Err(error) if error.kind() == io::ErrorKind::Unsupported => self
                .terminal_size()
                .map(|cells| WindowSize::new(cells, None)),
            Err(error) => Err(error),
        }
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        terminal::is_raw_mode_enabled()
    }
}

impl<W> KeyboardEnhancementQuery for CrosstermBackend<W> {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        terminal::supports_keyboard_enhancement()
    }
}

fn convert_window_size(size: terminal::WindowSize) -> WindowSize {
    let cells = TerminalSize::new(usize::from(size.columns), usize::from(size.rows));
    let pixels = (size.width != 0 && size.height != 0)
        .then(|| PixelSize::new(usize::from(size.width), usize::from(size.height)));
    WindowSize::new(cells, pixels)
}

impl<W> EventSource for CrosstermBackend<W> {
    fn read_event(&mut self) -> io::Result<Event> {
        Ok(convert_event(event::read()?))
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.poll_event_timeout(Duration::ZERO)
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(convert_event(event::read()?)))
        } else {
            Ok(None)
        }
    }
}

fn convert_keyboard_enhancement_flags(
    requested: KeyboardEnhancementFlags,
) -> CrosstermKeyboardEnhancementFlags {
    let mut flags = CrosstermKeyboardEnhancementFlags::empty();
    if requested.contains(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES) {
        flags |= CrosstermKeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES;
    }
    if requested.contains(KeyboardEnhancementFlags::REPORT_EVENT_TYPES) {
        flags |= CrosstermKeyboardEnhancementFlags::REPORT_EVENT_TYPES;
    }
    if requested.contains(KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS) {
        flags |= CrosstermKeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;
    }
    if requested.contains(KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES) {
        flags |= CrosstermKeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES;
    }
    flags
}

fn coordinate(value: usize) -> io::Result<u16> {
    u16::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "terminal coordinate exceeds u16",
        )
    })
}

fn offset(value: u32) -> io::Result<u16> {
    u16::try_from(value)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "terminal offset exceeds u16"))
}

fn write_cursor_move(writer: &mut impl Write, movement: CursorMove) -> io::Result<()> {
    match movement {
        CursorMove::To(position) => queue!(
            writer,
            cursor::MoveTo(coordinate(position.column())?, coordinate(position.row())?)
        ),
        CursorMove::ToColumn(column) => queue!(writer, cursor::MoveToColumn(coordinate(column)?)),
        CursorMove::ToRow(row) => queue!(writer, cursor::MoveToRow(coordinate(row)?)),
        CursorMove::By { columns, rows } => {
            if rows < 0 {
                queue!(writer, cursor::MoveUp(offset(rows.unsigned_abs())?))?;
            } else if rows > 0 {
                queue!(writer, cursor::MoveDown(offset(rows.unsigned_abs())?))?;
            }
            if columns < 0 {
                queue!(writer, cursor::MoveLeft(offset(columns.unsigned_abs())?))
            } else if columns > 0 {
                queue!(writer, cursor::MoveRight(offset(columns.unsigned_abs())?))
            } else {
                Ok(())
            }
        }
        CursorMove::ToNextLine(lines) => {
            queue!(writer, cursor::MoveToNextLine(coordinate(lines)?))
        }
        CursorMove::ToPreviousLine(lines) => {
            queue!(writer, cursor::MoveToPreviousLine(coordinate(lines)?))
        }
    }
}

fn convert_cursor_appearance(appearance: CursorAppearance) -> cursor::SetCursorStyle {
    match appearance {
        CursorAppearance::UserDefault => cursor::SetCursorStyle::DefaultUserShape,
        CursorAppearance::BlinkingBlock => cursor::SetCursorStyle::BlinkingBlock,
        CursorAppearance::SteadyBlock => cursor::SetCursorStyle::SteadyBlock,
        CursorAppearance::BlinkingUnderline => cursor::SetCursorStyle::BlinkingUnderScore,
        CursorAppearance::SteadyUnderline => cursor::SetCursorStyle::SteadyUnderScore,
        CursorAppearance::BlinkingBar => cursor::SetCursorStyle::BlinkingBar,
        CursorAppearance::SteadyBar => cursor::SetCursorStyle::SteadyBar,
    }
}

fn convert_clear_region(region: ClearRegion) -> ClearType {
    match region {
        ClearRegion::Screen => ClearType::All,
        ClearRegion::ScreenAndScrollback => ClearType::Purge,
        ClearRegion::BeforeCursor => ClearType::FromCursorUp,
        ClearRegion::AfterCursor => ClearType::FromCursorDown,
        ClearRegion::Line => ClearType::CurrentLine,
        ClearRegion::AfterCursorInLine => ClearType::UntilNewLine,
    }
}

fn write_style(writer: &mut impl Write, style: TerminalStyle) -> io::Result<()> {
    queue!(writer, SetAttribute(Attribute::Reset), ResetColor)?;
    if let Some(color) = style.foreground {
        queue!(writer, SetForegroundColor(convert_color(color)))?;
    }
    if let Some(color) = style.background {
        queue!(writer, SetBackgroundColor(convert_color(color)))?;
    }
    write_attributes(writer, style.attributes)?;
    write_underline(writer, style.underline)
}

fn write_underline(writer: &mut impl Write, underline: Option<Underline>) -> io::Result<()> {
    let Some(underline) = underline else {
        return Ok(());
    };
    let attribute = match underline.get_style() {
        UnderlineStyle::Single => Attribute::Underlined,
        UnderlineStyle::Double => Attribute::DoubleUnderlined,
        UnderlineStyle::Curly => Attribute::Undercurled,
        UnderlineStyle::Dotted => Attribute::Underdotted,
        UnderlineStyle::Dashed => Attribute::Underdashed,
    };
    queue!(writer, SetAttribute(attribute))?;
    if let Some(color) = underline.get_color() {
        queue!(writer, SetUnderlineColor(convert_color(color)))?;
    }
    Ok(())
}

fn write_hyperlink_start(
    writer: &mut impl Write,
    link: crate::TerminalHyperlink<'_>,
) -> io::Result<()> {
    writer.write_all(b"\x1b]8;")?;
    for (index, parameter) in link.parameters.iter().enumerate() {
        if index != 0 {
            writer.write_all(b":")?;
        }
        write_osc_field(writer, parameter.key, b":;=")?;
        writer.write_all(b"=")?;
        write_osc_field(writer, parameter.value, b":;=")?;
    }
    writer.write_all(b";")?;
    write_osc_field(writer, link.uri, b"")?;
    writer.write_all(b"\x1b\\")
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

fn convert_color(color: Color) -> CrosstermColor {
    match color {
        Color::Ansi(0) => CrosstermColor::Black,
        Color::Ansi(1) => CrosstermColor::DarkRed,
        Color::Ansi(2) => CrosstermColor::DarkGreen,
        Color::Ansi(3) => CrosstermColor::DarkYellow,
        Color::Ansi(4) => CrosstermColor::DarkBlue,
        Color::Ansi(5) => CrosstermColor::DarkMagenta,
        Color::Ansi(6) => CrosstermColor::DarkCyan,
        Color::Ansi(7) => CrosstermColor::Grey,
        Color::Ansi(8) => CrosstermColor::DarkGrey,
        Color::Ansi(9) => CrosstermColor::Red,
        Color::Ansi(10) => CrosstermColor::Green,
        Color::Ansi(11) => CrosstermColor::Yellow,
        Color::Ansi(12) => CrosstermColor::Blue,
        Color::Ansi(13) => CrosstermColor::Magenta,
        Color::Ansi(14) => CrosstermColor::Cyan,
        Color::Ansi(15) => CrosstermColor::White,
        Color::Ansi(index) | Color::Ansi256(index) => CrosstermColor::AnsiValue(index),
        Color::Rgb(red, green, blue) => CrosstermColor::Rgb {
            r: red,
            g: green,
            b: blue,
        },
    }
}

fn write_attributes(writer: &mut impl Write, attributes: TextAttributes) -> io::Result<()> {
    for attribute in attributes {
        let attribute = match attribute {
            TextAttribute::Bold => Attribute::Bold,
            TextAttribute::Dim => Attribute::Dim,
            TextAttribute::Italic => Attribute::Italic,
            TextAttribute::SlowBlink => Attribute::SlowBlink,
            TextAttribute::RapidBlink => Attribute::RapidBlink,
            TextAttribute::Reversed => Attribute::Reverse,
            TextAttribute::Hidden => Attribute::Hidden,
            TextAttribute::CrossedOut => Attribute::CrossedOut,
            TextAttribute::Fraktur => Attribute::Fraktur,
            TextAttribute::Framed => Attribute::Framed,
            TextAttribute::Encircled => Attribute::Encircled,
            TextAttribute::Overlined => Attribute::OverLined,
        };
        queue!(writer, SetAttribute(attribute))?;
    }
    Ok(())
}

fn convert_event(event: CrosstermEvent) -> Event {
    match event {
        CrosstermEvent::Key(key) => Event::Key(KeyEvent {
            code: convert_key_code(key.code),
            modifiers: convert_modifiers(key.modifiers),
            kind: match key.kind {
                KeyEventKind::Press => KeyKind::Press,
                KeyEventKind::Repeat => KeyKind::Repeat,
                KeyEventKind::Release => KeyKind::Release,
            },
            state: convert_key_state(key.state),
        }),
        CrosstermEvent::Mouse(mouse) => Event::Mouse(MouseEvent {
            kind: match mouse.kind {
                MouseEventKind::Down(button) => MouseKind::Down(convert_mouse_button(button)),
                MouseEventKind::Up(button) => MouseKind::Up(convert_mouse_button(button)),
                MouseEventKind::Drag(button) => MouseKind::Drag(convert_mouse_button(button)),
                MouseEventKind::Moved => MouseKind::Moved,
                MouseEventKind::ScrollDown => MouseKind::ScrollDown,
                MouseEventKind::ScrollUp => MouseKind::ScrollUp,
                MouseEventKind::ScrollLeft => MouseKind::ScrollLeft,
                MouseEventKind::ScrollRight => MouseKind::ScrollRight,
            },
            position: Position::new(usize::from(mouse.column), usize::from(mouse.row)),
            modifiers: convert_modifiers(mouse.modifiers),
        }),
        CrosstermEvent::Paste(text) => Event::Paste(text),
        CrosstermEvent::FocusGained => Event::Focus(FocusChange::Gained),
        CrosstermEvent::FocusLost => Event::Focus(FocusChange::Lost),
        CrosstermEvent::Resize(columns, rows) => {
            Event::Resize(TerminalSize::new(usize::from(columns), usize::from(rows)))
        }
    }
}

fn convert_key_code(code: CrosstermKeyCode) -> KeyCode {
    match code {
        CrosstermKeyCode::Char(character) => KeyCode::Char(character),
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => KeyCode::BackTab,
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::Esc => KeyCode::Escape,
        CrosstermKeyCode::Insert => KeyCode::Insert,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        CrosstermKeyCode::PageUp => KeyCode::PageUp,
        CrosstermKeyCode::PageDown => KeyCode::PageDown,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::F(number) => KeyCode::Function(number),
        CrosstermKeyCode::Null => KeyCode::Null,
        CrosstermKeyCode::CapsLock => KeyCode::CapsLock,
        CrosstermKeyCode::ScrollLock => KeyCode::ScrollLock,
        CrosstermKeyCode::NumLock => KeyCode::NumLock,
        CrosstermKeyCode::PrintScreen => KeyCode::PrintScreen,
        CrosstermKeyCode::Pause => KeyCode::Pause,
        CrosstermKeyCode::Menu => KeyCode::Menu,
        CrosstermKeyCode::KeypadBegin => KeyCode::KeypadBegin,
        CrosstermKeyCode::Media(code) => KeyCode::Media(convert_media_key(code)),
        CrosstermKeyCode::Modifier(code) => KeyCode::Modifier(convert_modifier_key(code)),
    }
}

fn convert_modifiers(modifiers: KeyModifiers) -> Modifiers {
    let mut converted = Modifiers::NONE;
    for (source, target) in [
        (KeyModifiers::SHIFT, Modifiers::SHIFT),
        (KeyModifiers::CONTROL, Modifiers::CONTROL),
        (KeyModifiers::ALT, Modifiers::ALT),
        (KeyModifiers::SUPER, Modifiers::SUPER),
        (KeyModifiers::HYPER, Modifiers::HYPER),
        (KeyModifiers::META, Modifiers::META),
    ] {
        if modifiers.contains(source) {
            converted = converted | target;
        }
    }
    converted
}

fn convert_key_state(state: CrosstermKeyEventState) -> KeyEventState {
    let mut converted = KeyEventState::NONE;
    for (source, target) in [
        (CrosstermKeyEventState::KEYPAD, KeyEventState::KEYPAD),
        (CrosstermKeyEventState::CAPS_LOCK, KeyEventState::CAPS_LOCK),
        (CrosstermKeyEventState::NUM_LOCK, KeyEventState::NUM_LOCK),
    ] {
        if state.contains(source) {
            converted = converted | target;
        }
    }
    converted
}

fn convert_media_key(code: CrosstermMediaKeyCode) -> MediaKeyCode {
    match code {
        CrosstermMediaKeyCode::Play => MediaKeyCode::Play,
        CrosstermMediaKeyCode::Pause => MediaKeyCode::Pause,
        CrosstermMediaKeyCode::PlayPause => MediaKeyCode::PlayPause,
        CrosstermMediaKeyCode::Reverse => MediaKeyCode::Reverse,
        CrosstermMediaKeyCode::Stop => MediaKeyCode::Stop,
        CrosstermMediaKeyCode::FastForward => MediaKeyCode::FastForward,
        CrosstermMediaKeyCode::Rewind => MediaKeyCode::Rewind,
        CrosstermMediaKeyCode::TrackNext => MediaKeyCode::TrackNext,
        CrosstermMediaKeyCode::TrackPrevious => MediaKeyCode::TrackPrevious,
        CrosstermMediaKeyCode::Record => MediaKeyCode::Record,
        CrosstermMediaKeyCode::LowerVolume => MediaKeyCode::LowerVolume,
        CrosstermMediaKeyCode::RaiseVolume => MediaKeyCode::RaiseVolume,
        CrosstermMediaKeyCode::MuteVolume => MediaKeyCode::MuteVolume,
    }
}

fn convert_modifier_key(code: CrosstermModifierKeyCode) -> ModifierKeyCode {
    match code {
        CrosstermModifierKeyCode::LeftShift => ModifierKeyCode::LeftShift,
        CrosstermModifierKeyCode::LeftControl => ModifierKeyCode::LeftControl,
        CrosstermModifierKeyCode::LeftAlt => ModifierKeyCode::LeftAlt,
        CrosstermModifierKeyCode::LeftSuper => ModifierKeyCode::LeftSuper,
        CrosstermModifierKeyCode::LeftHyper => ModifierKeyCode::LeftHyper,
        CrosstermModifierKeyCode::LeftMeta => ModifierKeyCode::LeftMeta,
        CrosstermModifierKeyCode::RightShift => ModifierKeyCode::RightShift,
        CrosstermModifierKeyCode::RightControl => ModifierKeyCode::RightControl,
        CrosstermModifierKeyCode::RightAlt => ModifierKeyCode::RightAlt,
        CrosstermModifierKeyCode::RightSuper => ModifierKeyCode::RightSuper,
        CrosstermModifierKeyCode::RightHyper => ModifierKeyCode::RightHyper,
        CrosstermModifierKeyCode::RightMeta => ModifierKeyCode::RightMeta,
        CrosstermModifierKeyCode::IsoLevel3Shift => ModifierKeyCode::IsoLevel3Shift,
        CrosstermModifierKeyCode::IsoLevel5Shift => ModifierKeyCode::IsoLevel5Shift,
    }
}

fn convert_mouse_button(button: CrosstermMouseButton) -> MouseButton {
    match button {
        CrosstermMouseButton::Left => MouseButton::Left,
        CrosstermMouseButton::Right => MouseButton::Right,
        CrosstermMouseButton::Middle => MouseButton::Middle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent as CrosstermKeyEvent, MouseEvent as CrosstermMouseEvent};

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    #[test]
    fn key_kind_code_and_modifiers_are_normalized() {
        let event = convert_event(CrosstermEvent::Key(CrosstermKeyEvent::new_with_kind(
            CrosstermKeyCode::Char('x'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
            KeyEventKind::Release,
        )));

        assert_eq!(
            event,
            Event::Key(KeyEvent {
                code: KeyCode::Char('x'),
                modifiers: Modifiers::CONTROL.union(Modifiers::ALT),
                kind: KeyKind::Release,
                state: KeyEventState::NONE,
            })
        );
    }

    #[test]
    fn enhanced_keyboard_information_is_preserved() {
        let event = convert_event(CrosstermEvent::Key(
            CrosstermKeyEvent::new_with_kind_and_state(
                CrosstermKeyCode::Modifier(CrosstermModifierKeyCode::RightSuper),
                KeyModifiers::SHIFT
                    | KeyModifiers::SUPER
                    | KeyModifiers::HYPER
                    | KeyModifiers::META,
                KeyEventKind::Repeat,
                CrosstermKeyEventState::KEYPAD
                    | CrosstermKeyEventState::CAPS_LOCK
                    | CrosstermKeyEventState::NUM_LOCK,
            ),
        ));

        assert_eq!(
            event,
            Event::Key(KeyEvent {
                code: KeyCode::Modifier(ModifierKeyCode::RightSuper),
                modifiers: Modifiers::SHIFT
                    .union(Modifiers::SUPER)
                    .union(Modifiers::HYPER)
                    .union(Modifiers::META),
                kind: KeyKind::Repeat,
                state: KeyEventState::KEYPAD | KeyEventState::CAPS_LOCK | KeyEventState::NUM_LOCK,
            })
        );
        assert_eq!(
            convert_key_code(CrosstermKeyCode::Media(CrosstermMediaKeyCode::TrackNext)),
            KeyCode::Media(MediaKeyCode::TrackNext)
        );
    }

    #[test]
    fn semantic_commands_cover_terminal_control_without_exposing_crossterm() {
        let mut backend = CrosstermBackend::new(Vec::new());
        backend
            .write_command(Command::MoveCursor(CursorMove::By {
                columns: -2,
                rows: 3,
            }))
            .expect("relative cursor move encodes");
        backend
            .write_command(Command::Clear(ClearRegion::ScreenAndScrollback))
            .expect("scrollback clear encodes");
        backend
            .write_command(Command::SetSynchronizedUpdate(true))
            .expect("synchronized update encodes");
        backend
            .write_command(Command::SetSynchronizedUpdate(false))
            .expect("synchronized update end encodes");
        backend
            .write_command(Command::SetHyperlink(Some(crate::TerminalHyperlink {
                uri: "https://example.invalid/\u{1b}",
                parameters: &[crate::HyperlinkParameter {
                    key: "id:unsafe",
                    value: "value",
                }],
            })))
            .expect("hyperlink encodes");
        backend
            .write_command(Command::ApplicationProgram(
                crate::ControlString::try_from("Ga=T;AAAA").expect("valid APC payload"),
            ))
            .expect("application command encodes");

        let output = backend.into_inner();
        assert!(contains(&output, b"\x1b[3B\x1b[2D"));
        assert!(contains(&output, b"\x1b[3J"));
        assert!(contains(&output, b"id%3Aunsafe=value"));
        assert!(contains(&output, b"https://example.invalid/%1B"));
        assert!(contains(&output, b"\x1b_Ga=T;AAAA\x1b\\"));
    }

    #[test]
    fn paste_focus_resize_and_mouse_do_not_leak_crossterm_types() {
        assert_eq!(
            convert_event(CrosstermEvent::Paste("text".to_owned())),
            Event::Paste("text".to_owned())
        );
        assert_eq!(
            convert_event(CrosstermEvent::FocusLost),
            Event::Focus(FocusChange::Lost)
        );
        assert_eq!(
            convert_event(CrosstermEvent::Resize(120, 40)),
            Event::Resize(TerminalSize::new(120, 40))
        );
        assert_eq!(
            convert_event(CrosstermEvent::Mouse(CrosstermMouseEvent {
                kind: MouseEventKind::Drag(CrosstermMouseButton::Left),
                column: 7,
                row: 9,
                modifiers: KeyModifiers::SHIFT,
            })),
            Event::Mouse(MouseEvent {
                kind: MouseKind::Drag(MouseButton::Left),
                position: Position::new(7, 9),
                modifiers: Modifiers::SHIFT,
            })
        );
    }

    #[test]
    fn absent_pixel_geometry_does_not_erase_cell_geometry() {
        let size = convert_window_size(terminal::WindowSize {
            columns: 80,
            rows: 24,
            width: 0,
            height: 0,
        });
        assert_eq!(size.cells(), TerminalSize::new(80, 24));
        assert_eq!(size.pixels(), None);

        let size = convert_window_size(terminal::WindowSize {
            columns: 80,
            rows: 24,
            width: 1600,
            height: 960,
        });
        assert_eq!(size.pixels(), Some(PixelSize::new(1600, 960)));
    }
}
