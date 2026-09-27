//! Native Unix terminal backend.

use std::{
    collections::VecDeque,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::fd::AsRawFd,
    os::unix::fs::OpenOptionsExt,
    time::{Duration, Instant},
};

use crate::{
    Command, CommandWriter, Event, EventSource, FocusChange, KeyCode, KeyEvent, KeyKind,
    KeyboardEnhancementQuery, Modifiers, MouseButton, MouseEvent, MouseKind, PixelSize, Position,
    RawModeControl, TerminalInfo, TerminalOutput, TerminalQuery, TerminalSize, WindowSize,
    backend::ansi::AnsiWriter,
};

const ESCAPE_TIMEOUT: Duration = Duration::from_millis(10);
const QUERY_TIMEOUT: Duration = Duration::from_secs(2);
const RESIZE_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// One owned connection to the process's controlling terminal.
///
/// Input, output, raw mode, window inspection, protocol replies, and ordinary
/// events all use the same `/dev/tty` endpoint. No external terminal framework
/// participates in this backend.
pub struct NativeTerminal {
    output: AnsiWriter<File>,
    original_termios: Option<libc::termios>,
    input: Vec<u8>,
    input_start: usize,
    pending: VecDeque<Event>,
    cursor_response: Option<Position>,
    last_size: TerminalSize,
}

impl NativeTerminal {
    /// Opens the process's controlling terminal as one input/output endpoint.
    pub fn open() -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_CLOEXEC)
            .open("/dev/tty")?;
        Self::from_file(file)
    }

    fn from_file(file: File) -> io::Result<Self> {
        let last_size = window_size(&file)?.cells();
        Ok(Self {
            output: AnsiWriter::new(file),
            original_termios: None,
            input: Vec::new(),
            input_start: 0,
            pending: VecDeque::new(),
            cursor_response: None,
            last_size,
        })
    }

    /// Inspects the terminal endpoint opened by this backend.
    pub fn terminal_info(&mut self) -> io::Result<TerminalInfo> {
        let size = self.terminal_size()?;
        Ok(TerminalInfo::new(
            size,
            crate::detect_capabilities(
                std::env::var("TERM").ok().as_deref(),
                std::env::var("COLORTERM").ok().as_deref(),
            ),
        ))
    }

    fn file(&self) -> &File {
        self.output.writer()
    }

    fn file_mut(&mut self) -> &mut File {
        self.output.writer_mut()
    }

    fn next_event(&mut self, timeout: Option<Duration>) -> io::Result<Option<Event>> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(event));
        }

        self.next_wire_event(timeout)
    }

    fn next_wire_event(&mut self, timeout: Option<Duration>) -> io::Result<Option<Event>> {
        let started = Instant::now();
        let raw_mode = self.original_termios.is_some() || self.raw_mode_enabled()?;
        loop {
            if let Some(decoded) = decode(self.unread_input(), false, raw_mode)? {
                self.consume_input(decoded.consumed);
                match decoded.item {
                    DecodedItem::Event(event) => return Ok(Some(event)),
                    DecodedItem::Cursor(position) => {
                        self.cursor_response = Some(position);
                        return Ok(None);
                    }
                }
            }

            let remaining = timeout.map(|limit| limit.saturating_sub(started.elapsed()));
            let mut wait = remaining.map_or(RESIZE_POLL_INTERVAL, |value| {
                value.min(RESIZE_POLL_INTERVAL)
            });
            if self.unread_input().first() == Some(&0x1b) {
                wait = wait.min(ESCAPE_TIMEOUT);
            }
            let read = self.read_input(wait)?;
            if !read {
                if let Some(decoded) = decode(self.unread_input(), true, raw_mode)? {
                    self.consume_input(decoded.consumed);
                    match decoded.item {
                        DecodedItem::Event(event) => return Ok(Some(event)),
                        DecodedItem::Cursor(position) => {
                            self.cursor_response = Some(position);
                            return Ok(None);
                        }
                    }
                }
                let size = window_size(self.file())?.cells();
                if size != self.last_size {
                    self.last_size = size;
                    return Ok(Some(Event::Resize(size)));
                }
                if timeout.is_some_and(|limit| started.elapsed() >= limit) {
                    return Ok(None);
                }
            }
        }
    }

    fn unread_input(&self) -> &[u8] {
        &self.input[self.input_start..]
    }

    fn consume_input(&mut self, consumed: usize) {
        self.input_start += consumed;
        if self.input_start == self.input.len() {
            self.input.clear();
            self.input_start = 0;
        } else if self.input_start >= 4096 && self.input_start >= self.input.len() / 2 {
            self.input.copy_within(self.input_start.., 0);
            self.input.truncate(self.input.len() - self.input_start);
            self.input_start = 0;
        }
    }

    fn read_input(&mut self, timeout: Duration) -> io::Result<bool> {
        if !wait_readable(self.file(), timeout)? {
            return Ok(false);
        }
        let mut buffer = [0_u8; 4096];
        match self.file_mut().read(&mut buffer) {
            Ok(0) => Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "controlling terminal reached end of input",
            )),
            Ok(length) => {
                self.input.extend_from_slice(&buffer[..length]);
                Ok(true)
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(error) => Err(error),
        }
    }
}

impl TerminalOutput for NativeTerminal {
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()
    }
}

impl CommandWriter for NativeTerminal {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.output.write_command(command)
    }
}

impl RawModeControl for NativeTerminal {
    fn is_interactive(&self) -> bool {
        true
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        if self.original_termios.is_some() {
            return Ok(());
        }
        let original = get_termios(self.file())?;
        let mut raw = original;
        // Matches POSIX cfmakeraw while retaining the exact original value for
        // restoration rather than trying to reconstruct it later.
        unsafe { libc::cfmakeraw(&mut raw) };
        set_termios(self.file(), &raw)?;
        self.original_termios = Some(original);
        Ok(())
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        let Some(original) = self.original_termios else {
            return Ok(());
        };
        set_termios(self.file(), &original)?;
        self.original_termios = None;
        Ok(())
    }
}

impl TerminalQuery for NativeTerminal {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        Ok(window_size(self.file())?.cells())
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        self.cursor_response = None;
        self.output.writer_mut().write_all(b"\x1b[6n")?;
        self.output.flush()?;
        let started = Instant::now();
        loop {
            if let Some(position) = self.cursor_response.take() {
                return Ok(position);
            }
            let remaining = QUERY_TIMEOUT.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "terminal did not answer the cursor-position query",
                ));
            }
            if let Some(event) = self.next_wire_event(Some(remaining))? {
                self.pending.push_back(event);
            }
        }
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        window_size(self.file())
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        let mode = get_termios(self.file())?;
        Ok(mode.c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG) == 0)
    }
}

impl KeyboardEnhancementQuery for NativeTerminal {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        // The native decoder currently requests only the portable legacy input
        // protocol, so it must not advertise enhanced event negotiation.
        Ok(false)
    }
}

impl EventSource for NativeTerminal {
    fn read_event(&mut self) -> io::Result<Event> {
        loop {
            if let Some(event) = self.next_event(None)? {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.next_event(Some(Duration::ZERO))
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.next_event(Some(timeout))
    }
}

impl Drop for NativeTerminal {
    fn drop(&mut self) {
        if let Some(original) = self.original_termios {
            let _restore_error = set_termios(self.file(), &original);
        }
    }
}

fn get_termios(file: &File) -> io::Result<libc::termios> {
    let mut value = MaybeUninit::<libc::termios>::uninit();
    // SAFETY: `value` points to writable storage for one termios value and the
    // file descriptor remains open for the duration of the call.
    if unsafe { libc::tcgetattr(file.as_raw_fd(), value.as_mut_ptr()) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: tcgetattr returned success and therefore initialized value.
        Ok(unsafe { value.assume_init() })
    }
}

fn set_termios(file: &File, value: &libc::termios) -> io::Result<()> {
    // SAFETY: `value` is a valid termios value and the descriptor stays open.
    if unsafe { libc::tcsetattr(file.as_raw_fd(), libc::TCSANOW, value) } == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn window_size(file: &File) -> io::Result<WindowSize> {
    let mut value = MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: `value` is writable winsize storage and the descriptor is open.
    if unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCGWINSZ, value.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: ioctl returned success and initialized the winsize structure.
    let value = unsafe { value.assume_init() };
    let cells = TerminalSize::new(usize::from(value.ws_col), usize::from(value.ws_row));
    let pixels = (value.ws_xpixel != 0 && value.ws_ypixel != 0)
        .then(|| PixelSize::new(usize::from(value.ws_xpixel), usize::from(value.ws_ypixel)));
    Ok(WindowSize::new(cells, pixels))
}

fn wait_readable(file: &File, timeout: Duration) -> io::Result<bool> {
    let mut descriptor = libc::pollfd {
        fd: file.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let milliseconds = if timeout.is_zero() {
        0
    } else {
        timeout.as_millis().clamp(1, i32::MAX as u128) as i32
    };
    // SAFETY: `descriptor` is valid stack storage for one pollfd and its file
    // descriptor remains open for the duration of the call.
    let result = unsafe { libc::poll(&mut descriptor, 1, milliseconds) };
    if result == -1 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::Interrupted {
            Ok(false)
        } else {
            Err(error)
        }
    } else {
        Ok(result > 0)
    }
}

struct Decoded {
    item: DecodedItem,
    consumed: usize,
}

enum DecodedItem {
    Event(Event),
    Cursor(Position),
}

fn decode(input: &[u8], escape_complete: bool, raw_mode: bool) -> io::Result<Option<Decoded>> {
    let Some(&first) = input.first() else {
        return Ok(None);
    };
    if first != 0x1b {
        return decode_plain(input, raw_mode);
    }
    if input.len() == 1 {
        return escape_complete
            .then(|| decoded_key(KeyCode::Escape, Modifiers::NONE, 1))
            .transpose();
    }
    match input[1] {
        b'[' => decode_csi(input, escape_complete),
        b'O' => decode_ss3(input),
        0x1b => decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some),
        _ => {
            let Some(mut decoded) = decode_plain(&input[1..], raw_mode)? else {
                return if escape_complete {
                    decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some)
                } else {
                    Ok(None)
                };
            };
            if let DecodedItem::Event(Event::Key(ref mut key)) = decoded.item {
                key.modifiers = key.modifiers | Modifiers::ALT;
            }
            decoded.consumed += 1;
            Ok(Some(decoded))
        }
    }
}

fn decode_plain(input: &[u8], raw_mode: bool) -> io::Result<Option<Decoded>> {
    let Some(&first) = input.first() else {
        return Ok(None);
    };
    match first {
        b'\r' => return decoded_key(KeyCode::Enter, Modifiers::NONE, 1).map(Some),
        b'\n' if !raw_mode => return decoded_key(KeyCode::Enter, Modifiers::NONE, 1).map(Some),
        b'\t' => return decoded_key(KeyCode::Tab, Modifiers::NONE, 1).map(Some),
        0x7f => return decoded_key(KeyCode::Backspace, Modifiers::NONE, 1).map(Some),
        0 => return decoded_key(KeyCode::Char(' '), Modifiers::CONTROL, 1).map(Some),
        byte @ 1..=26 => {
            return decoded_key(
                KeyCode::Char(char::from(b'a' + byte - 1)),
                Modifiers::CONTROL,
                1,
            )
            .map(Some);
        }
        byte @ 0x1c..=0x1f => {
            return decoded_key(
                KeyCode::Char(char::from(b'4' + byte - 0x1c)),
                Modifiers::CONTROL,
                1,
            )
            .map(Some);
        }
        _ => {}
    }
    let width = utf8_width(first)?;
    if input.len() < width {
        return Ok(None);
    }
    let text = std::str::from_utf8(&input[..width])
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let character = text
        .chars()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "empty UTF-8 input"))?;
    let modifiers = if character.is_uppercase() {
        Modifiers::SHIFT
    } else {
        Modifiers::NONE
    };
    decoded_key(KeyCode::Char(character), modifiers, width).map(Some)
}

fn decode_ss3(input: &[u8]) -> io::Result<Option<Decoded>> {
    let Some(&code) = input.get(2) else {
        return Ok(None);
    };
    let key = match code {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'P'..=b'S' => KeyCode::Function(code - b'P' + 1),
        _ => return decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some),
    };
    decoded_key(key, Modifiers::NONE, 3).map(Some)
}

fn decode_csi(input: &[u8], escape_complete: bool) -> io::Result<Option<Decoded>> {
    if input.starts_with(b"\x1b[200~") {
        let Some(end) = find_subslice(&input[6..], b"\x1b[201~") else {
            return Ok(None);
        };
        let text = std::str::from_utf8(&input[6..6 + end])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
            .to_owned();
        return Ok(Some(Decoded {
            item: DecodedItem::Event(Event::Paste(text)),
            consumed: 6 + end + 6,
        }));
    }
    let Some(final_index) = input[2..]
        .iter()
        .position(|byte| (0x40..=0x7e).contains(byte))
        .map(|index| index + 2)
    else {
        return if escape_complete {
            decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some)
        } else {
            Ok(None)
        };
    };
    let final_byte = input[final_index];
    let body = &input[2..final_index];
    let consumed = final_index + 1;
    if final_byte == b'R' {
        let (row, column) = two_numbers(body)?;
        return Ok(Some(Decoded {
            item: DecodedItem::Cursor(Position::new(
                column.saturating_sub(1),
                row.saturating_sub(1),
            )),
            consumed,
        }));
    }
    if body.starts_with(b"<") && matches!(final_byte, b'M' | b'm') {
        return decode_mouse(body, final_byte, consumed).map(Some);
    }
    let modifiers = csi_modifiers(body);
    let key = match final_byte {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'H' => KeyCode::Home,
        b'F' => KeyCode::End,
        b'Z' => {
            return decoded_key(KeyCode::BackTab, Modifiers::SHIFT, consumed).map(Some);
        }
        b'I' => {
            return Ok(Some(Decoded {
                item: DecodedItem::Event(Event::Focus(FocusChange::Gained)),
                consumed,
            }));
        }
        b'O' => {
            return Ok(Some(Decoded {
                item: DecodedItem::Event(Event::Focus(FocusChange::Lost)),
                consumed,
            }));
        }
        b'~' => match first_number(body)? {
            1 | 7 => KeyCode::Home,
            2 => KeyCode::Insert,
            3 => KeyCode::Delete,
            4 | 8 => KeyCode::End,
            5 => KeyCode::PageUp,
            6 => KeyCode::PageDown,
            11 => KeyCode::Function(1),
            12 => KeyCode::Function(2),
            13 => KeyCode::Function(3),
            14 => KeyCode::Function(4),
            15 => KeyCode::Function(5),
            17 => KeyCode::Function(6),
            18 => KeyCode::Function(7),
            19 => KeyCode::Function(8),
            20 => KeyCode::Function(9),
            21 => KeyCode::Function(10),
            23 => KeyCode::Function(11),
            24 => KeyCode::Function(12),
            _ => return decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some),
        },
        _ => return decoded_key(KeyCode::Escape, Modifiers::NONE, 1).map(Some),
    };
    decoded_key(key, modifiers, consumed).map(Some)
}

fn decode_mouse(body: &[u8], final_byte: u8, consumed: usize) -> io::Result<Decoded> {
    let text = std::str::from_utf8(body.strip_prefix(b"<").unwrap_or(body))
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut parts = text.split(';');
    let flags = parse_number(parts.next())?;
    let column = parse_number(parts.next())?.saturating_sub(1);
    let row = parse_number(parts.next())?.saturating_sub(1);
    let modifiers = mouse_modifiers(flags);
    let kind = if flags & 64 != 0 {
        match flags & 3 {
            0 => MouseKind::ScrollUp,
            1 => MouseKind::ScrollDown,
            2 => MouseKind::ScrollLeft,
            _ => MouseKind::ScrollRight,
        }
    } else if flags & 32 != 0 {
        if flags & 3 == 3 {
            MouseKind::Moved
        } else {
            MouseKind::Drag(mouse_button(flags))
        }
    } else if final_byte == b'm' {
        MouseKind::Up(mouse_button(flags))
    } else {
        MouseKind::Down(mouse_button(flags))
    };
    Ok(Decoded {
        item: DecodedItem::Event(Event::Mouse(MouseEvent {
            kind,
            position: Position::new(column, row),
            modifiers,
        })),
        consumed,
    })
}

fn decoded_key(code: KeyCode, modifiers: Modifiers, consumed: usize) -> io::Result<Decoded> {
    Ok(Decoded {
        item: DecodedItem::Event(Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyKind::Press,
            state: crate::KeyEventState::NONE,
        })),
        consumed,
    })
}

fn csi_modifiers(body: &[u8]) -> Modifiers {
    let value = std::str::from_utf8(body)
        .ok()
        .and_then(|text| text.split(';').nth(1))
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(1)
        .saturating_sub(1);
    let mut modifiers = Modifiers::NONE;
    if value & 1 != 0 {
        modifiers = modifiers | Modifiers::SHIFT;
    }
    if value & 2 != 0 {
        modifiers = modifiers | Modifiers::ALT;
    }
    if value & 4 != 0 {
        modifiers = modifiers | Modifiers::CONTROL;
    }
    modifiers
}

fn mouse_modifiers(flags: usize) -> Modifiers {
    let mut modifiers = Modifiers::NONE;
    if flags & 4 != 0 {
        modifiers = modifiers | Modifiers::SHIFT;
    }
    if flags & 8 != 0 {
        modifiers = modifiers | Modifiers::ALT;
    }
    if flags & 16 != 0 {
        modifiers = modifiers | Modifiers::CONTROL;
    }
    modifiers
}

const fn mouse_button(flags: usize) -> MouseButton {
    match flags & 3 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        _ => MouseButton::Right,
    }
}

fn two_numbers(body: &[u8]) -> io::Result<(usize, usize)> {
    let text = std::str::from_utf8(body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut values = text.split(';');
    Ok((parse_number(values.next())?, parse_number(values.next())?))
}

fn first_number(body: &[u8]) -> io::Result<usize> {
    let text = std::str::from_utf8(body)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    parse_number(text.split(';').next())
}

fn parse_number(value: Option<&str>) -> io::Result<usize> {
    value
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing terminal parameter"))?
        .parse()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn utf8_width(first: u8) -> io::Result<usize> {
    match first {
        0x00..=0x7f => Ok(1),
        0xc2..=0xdf => Ok(2),
        0xe0..=0xef => Ok(3),
        0xf0..=0xf4 => Ok(4),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid leading UTF-8 byte from terminal",
        )),
    }
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use std::{os::fd::FromRawFd, time::Duration};

    use super::*;
    use crate::TerminalText;

    fn event(bytes: &[u8]) -> Event {
        let decoded = decode(bytes, true, true)
            .expect("sequence is valid")
            .expect("sequence is complete");
        assert_eq!(decoded.consumed, bytes.len());
        match decoded.item {
            DecodedItem::Event(event) => event,
            DecodedItem::Cursor(_) => panic!("expected an event"),
        }
    }

    #[test]
    fn decoder_handles_text_navigation_modifiers_and_paste() {
        assert_eq!(
            event("漆".as_bytes()),
            Event::Key(KeyEvent::new(KeyCode::Char('漆')))
        );
        assert_eq!(
            event(b"\x1b[1;5A"),
            Event::Key(KeyEvent::new(KeyCode::Up).with_modifiers(Modifiers::CONTROL))
        );
        assert_eq!(
            event(b"\x1b[200~a\nb\x1b[201~"),
            Event::Paste("a\nb".to_owned())
        );
        assert_eq!(
            event(b"\n"),
            Event::Key(KeyEvent::new(KeyCode::Char('j')).with_modifiers(Modifiers::CONTROL))
        );
        let canonical_newline = decode(b"\n", true, false)
            .expect("newline is valid")
            .expect("newline is complete");
        assert!(matches!(
            canonical_newline.item,
            DecodedItem::Event(Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }))
        ));
    }

    #[test]
    fn decoder_separates_cursor_replies_from_events() {
        let decoded = decode(b"\x1b[12;34R", false, true)
            .expect("reply is valid")
            .expect("reply is complete");
        assert!(matches!(decoded.item, DecodedItem::Cursor(Position { .. })));
        if let DecodedItem::Cursor(position) = decoded.item {
            assert_eq!(position, Position::new(33, 11));
        }
    }

    #[test]
    fn native_terminal_owns_command_input_mode_and_query_on_one_tty() {
        let (mut peer, mut terminal) = terminal_pair();

        terminal.enable_raw_mode().expect("raw mode is enabled");
        assert!(terminal.raw_mode_enabled().expect("raw mode is inspected"));

        peer.write_all(b"\x1b[1;5A").expect("input is written");
        assert_eq!(
            terminal
                .poll_event_timeout(Duration::from_millis(100))
                .expect("event is read"),
            Some(Event::Key(
                KeyEvent::new(KeyCode::Up).with_modifiers(Modifiers::CONTROL)
            ))
        );

        terminal
            .write_command(Command::Print(
                TerminalText::try_from("ok").expect("printable text"),
            ))
            .expect("command is written");
        terminal.flush().expect("command is flushed");
        let mut output = [0_u8; 2];
        peer.read_exact(&mut output).expect("command reaches tty");
        assert_eq!(&output, b"ok");

        terminal.disable_raw_mode().expect("raw mode is restored");
        assert!(!terminal.raw_mode_enabled().expect("mode is inspected"));
    }

    #[test]
    fn cursor_query_consumes_its_reply_without_losing_ordinary_events() {
        let (mut peer, mut terminal) = terminal_pair();
        terminal.enable_raw_mode().expect("raw mode is enabled");
        peer.write_all(b"x\x1b[12;34R")
            .expect("ordinary input and cursor reply are written");

        assert_eq!(
            terminal.cursor_position().expect("query is answered"),
            Position::new(33, 11)
        );
        let mut query = [0_u8; 4];
        peer.read_exact(&mut query).expect("query reaches tty");
        assert_eq!(&query, b"\x1b[6n");
        assert_eq!(
            terminal.read_event().expect("queued event is retained"),
            Event::Key(KeyEvent::new(KeyCode::Char('x')))
        );
    }

    fn terminal_pair() -> (File, NativeTerminal) {
        let mut peer_fd = -1;
        let mut terminal_fd = -1;
        // SAFETY: openpty initializes both descriptors on success. The null
        // name and settings pointers request the platform defaults.
        let result = unsafe {
            libc::openpty(
                &mut peer_fd,
                &mut terminal_fd,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        assert_eq!(result, 0, "openpty failed: {}", io::Error::last_os_error());
        // SAFETY: openpty returned fresh owned descriptors and each is wrapped
        // exactly once.
        let peer = unsafe { File::from_raw_fd(peer_fd) };
        // SAFETY: see the ownership argument above for the peer descriptor.
        let terminal_file = unsafe { File::from_raw_fd(terminal_fd) };
        let terminal = NativeTerminal::from_file(terminal_file).expect("pty is a terminal");
        (peer, terminal)
    }
}
