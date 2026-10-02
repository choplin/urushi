use std::{
    io::{self, Stderr},
    time::Duration,
};

use urushi_terminal::{
    ColorLevel, Command, CommandWriter, Event, EventSource, KeyboardEnhancementQuery, Position,
    RawModeControl, TerminalCapabilities, TerminalOutput, TerminalQuery, TerminalSize,
    TextAttributes, UnderlineStyles, WindowSize,
    backend::crossterm::CrosstermBackend,
};

/// Crossterm connection with the capabilities fixed by the VHS capture tape.
///
/// This fixture is not a capability detector. The tape starts a known
/// `xterm-256color`/true-color PTY, so the backend records those properties
/// explicitly while forwarding real input, output, raw-mode, and size handling
/// to Crossterm.
pub struct CaptureTerminal {
    inner: CrosstermBackend<Stderr>,
}

impl CaptureTerminal {
    pub fn stderr() -> Self {
        Self {
            inner: CrosstermBackend::new(io::stderr()),
        }
    }
}

impl TerminalOutput for CaptureTerminal {
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl CommandWriter for CaptureTerminal {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.inner.write_command(command)
    }
}

impl RawModeControl for CaptureTerminal {
    fn is_interactive(&self) -> bool {
        self.inner.is_interactive()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        self.inner.enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        self.inner.disable_raw_mode()
    }
}

impl TerminalQuery for CaptureTerminal {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        self.inner.terminal_size()
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        Ok(Position::new(0, 0))
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        self.inner.raw_mode_enabled()
    }

    fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
        Ok(TerminalCapabilities::none()
            .with_color_level(ColorLevel::TrueColor)
            .with_attributes(TextAttributes::all())
            .with_underline_styles(UnderlineStyles::all())
            .with_underline_colors(true))
    }
}

impl KeyboardEnhancementQuery for CaptureTerminal {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        Ok(false)
    }
}

impl EventSource for CaptureTerminal {
    fn read_event(&mut self) -> io::Result<Event> {
        self.inner.read_event()
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.inner.poll_event()
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.inner.poll_event_timeout(timeout)
    }
}
