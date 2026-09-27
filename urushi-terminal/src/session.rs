//! Acquisition and restoration of interactive terminal state.

use std::{error::Error, fmt, io};

use crate::{Command, CommandWriter, KeyboardEnhancementFlags, KeyboardEnhancementQuery};

/// Process-side terminal modes that cannot be expressed as terminal commands.
///
/// Output modes such as the alternate screen and input reporting are expressed
/// by [`Command`]. Raw mode changes the process terminal driver and therefore
/// remains a separate backend capability.
pub trait RawModeControl {
    fn is_interactive(&self) -> bool;

    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
}

impl<T: RawModeControl + ?Sized> RawModeControl for &mut T {
    fn is_interactive(&self) -> bool {
        T::is_interactive(self)
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        T::enable_raw_mode(self)
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        T::disable_raw_mode(self)
    }
}

/// Terminal modes acquired by a session.
///
/// The default acquires no modes. Prompt and TUI entry points choose their own
/// explicit profiles rather than placing application policy in this crate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionOptions {
    /// Enables raw input mode.
    pub raw_mode: bool,
    /// Uses the alternate screen buffer.
    pub alternate_screen: bool,
    /// Reports a paste as one paste event.
    pub bracketed_paste: bool,
    /// Reports terminal focus changes.
    pub focus_change: bool,
    /// Requests the selected enhanced-keyboard information when supported.
    pub keyboard_enhancement: Option<KeyboardEnhancementFlags>,
    /// Captures mouse input instead of leaving terminal text selection active.
    pub mouse_capture: bool,
    /// Hides the cursor until a frame or restoration shows it.
    pub hide_cursor: bool,
}

/// A session-entry failure and the first restoration failure, if any.
#[derive(Debug)]
pub struct SessionError {
    source: io::Error,
    cleanup: Option<io::Error>,
}

impl SessionError {
    /// Returns the operation failure that prevented entry.
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }

    /// Returns the first additional failure encountered during restoration.
    pub fn cleanup_error(&self) -> Option<&io::Error> {
        self.cleanup.as_ref()
    }

    /// Splits the entry failure from its optional restoration failure.
    pub fn into_parts(self) -> (io::Error, Option<io::Error>) {
        (self.source, self.cleanup)
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to enter terminal session: {}",
            self.source
        )?;
        if let Some(cleanup) = &self.cleanup {
            write!(formatter, "; terminal restoration also failed: {cleanup}")?;
        }
        Ok(())
    }
}

impl Error for SessionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// A guard for every terminal state successfully acquired during entry.
///
/// Call [`restore`](Self::restore) on normal and error returns so restoration
/// errors can be reported. `Drop` is the final guard during panic unwinding.
/// It cannot report restoration failures. The session cannot restore the
/// cursor's pre-entry position, process aborts, `kill -9`, or signals whose
/// handling never unwinds through this value.
pub struct TerminalSession<
    'a,
    C: RawModeControl + CommandWriter + KeyboardEnhancementQuery + ?Sized,
> {
    control: &'a mut C,
    entered: Entered,
    restored: bool,
}

#[derive(Default)]
struct Entered {
    raw_mode: bool,
    alternate_screen: bool,
    bracketed_paste: bool,
    focus_change: bool,
    keyboard_enhancement: bool,
    mouse_capture: bool,
    cursor_hidden: bool,
}

impl<'a, C> TerminalSession<'a, C>
where
    C: RawModeControl + CommandWriter + KeyboardEnhancementQuery + ?Sized,
{
    /// Acquires the requested modes, restoring partial state if entry fails.
    pub fn enter(control: &'a mut C, options: SessionOptions) -> Result<Self, SessionError> {
        let mut session = Self {
            control,
            entered: Entered::default(),
            restored: false,
        };
        if let Err(source) = session.acquire(options) {
            let cleanup = session.restore().err();
            return Err(SessionError { source, cleanup });
        }
        Ok(session)
    }

    /// Borrows the underlying control while the session remains active.
    pub fn control_mut(&mut self) -> &mut C {
        self.control
    }

    /// Attempts every outstanding restoration in reverse acquisition order.
    ///
    /// The first error is returned after all remaining steps and the final
    /// flush have been attempted. Repeated calls are no-ops.
    pub fn restore(&mut self) -> io::Result<()> {
        if self.restored {
            return Ok(());
        }
        self.restored = true;
        let mut first = None;
        macro_rules! restore_command {
            ($entered:ident, $command:expr) => {
                if self.entered.$entered {
                    self.entered.$entered = false;
                    if let Err(error) = self.control.write_command($command) {
                        record_first(&mut first, error);
                    }
                }
            };
        }
        restore_command!(cursor_hidden, Command::SetCursorVisible(true));
        restore_command!(mouse_capture, Command::SetMouseCapture(false));
        restore_command!(keyboard_enhancement, Command::PopKeyboardEnhancement);
        restore_command!(focus_change, Command::SetFocusReporting(false));
        restore_command!(bracketed_paste, Command::SetBracketedPaste(false));
        restore_command!(alternate_screen, Command::SetAlternateScreen(false));
        if let Err(error) = self.control.flush() {
            record_first(&mut first, error);
        }
        if self.entered.raw_mode {
            self.entered.raw_mode = false;
            if let Err(error) = self.control.disable_raw_mode() {
                record_first(&mut first, error);
            }
        }
        first.map_or(Ok(()), Err)
    }

    fn acquire(&mut self, options: SessionOptions) -> io::Result<()> {
        if options.raw_mode {
            self.entered.raw_mode = true;
            self.control.enable_raw_mode()?;
        }
        if options.alternate_screen {
            self.entered.alternate_screen = true;
            self.control
                .write_command(Command::SetAlternateScreen(true))?;
        }
        if options.bracketed_paste {
            self.entered.bracketed_paste = true;
            self.control
                .write_command(Command::SetBracketedPaste(true))?;
        }
        if options.focus_change {
            self.entered.focus_change = true;
            self.control
                .write_command(Command::SetFocusReporting(true))?;
        }
        if let Some(flags) = options.keyboard_enhancement
            && self.control.supports_keyboard_enhancement()?
        {
            self.entered.keyboard_enhancement = true;
            self.control
                .write_command(Command::PushKeyboardEnhancement(flags))?;
        }
        if options.mouse_capture {
            self.entered.mouse_capture = true;
            self.control.write_command(Command::SetMouseCapture(true))?;
        }
        if options.hide_cursor {
            self.entered.cursor_hidden = true;
            self.control
                .write_command(Command::SetCursorVisible(false))?;
        }
        self.control.flush()
    }
}

impl<C> Drop for TerminalSession<'_, C>
where
    C: RawModeControl + CommandWriter + KeyboardEnhancementQuery + ?Sized,
{
    fn drop(&mut self) {
        if !self.restored {
            let _cleanup_error = self.restore();
        }
    }
}

fn record_first(first: &mut Option<io::Error>, error: io::Error) {
    if first.is_none() {
        *first = Some(error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Position, TerminalOutput, TerminalQuery, TerminalSize, WindowSize};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const ACQUIRE: [&str; 8] = [
        "enable_raw",
        "enter_alternate",
        "enable_paste",
        "enable_focus",
        "enable_keyboard",
        "enable_mouse",
        "hide_cursor",
        "flush",
    ];
    const RESTORE: [&str; 8] = [
        "show_cursor",
        "disable_mouse",
        "disable_keyboard",
        "disable_focus",
        "disable_paste",
        "leave_alternate",
        "flush",
        "disable_raw",
    ];

    #[derive(Default)]
    struct RecordingControl {
        calls: Vec<&'static str>,
        fail_acquire: Option<&'static str>,
        fail_restore: Vec<&'static str>,
    }

    impl RecordingControl {
        fn call(&mut self, name: &'static str) -> io::Result<()> {
            self.calls.push(name);
            if self.fail_acquire == Some(name) || self.fail_restore.contains(&name) {
                Err(io::Error::other(name))
            } else {
                Ok(())
            }
        }
    }

    impl RawModeControl for RecordingControl {
        fn is_interactive(&self) -> bool {
            true
        }
        fn enable_raw_mode(&mut self) -> io::Result<()> {
            self.call("enable_raw")
        }
        fn disable_raw_mode(&mut self) -> io::Result<()> {
            self.call("disable_raw")
        }
    }

    impl CommandWriter for RecordingControl {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            let name = match command {
                Command::SetAlternateScreen(true) => "enter_alternate",
                Command::SetAlternateScreen(false) => "leave_alternate",
                Command::SetBracketedPaste(true) => "enable_paste",
                Command::SetBracketedPaste(false) => "disable_paste",
                Command::SetFocusReporting(true) => "enable_focus",
                Command::SetFocusReporting(false) => "disable_focus",
                Command::PushKeyboardEnhancement(_) => "enable_keyboard",
                Command::PopKeyboardEnhancement => "disable_keyboard",
                Command::SetMouseCapture(true) => "enable_mouse",
                Command::SetMouseCapture(false) => "disable_mouse",
                Command::SetCursorVisible(false) => "hide_cursor",
                Command::SetCursorVisible(true) => "show_cursor",
                _ => panic!("unexpected session command: {command:?}"),
            };
            self.call(name)
        }
    }

    impl TerminalQuery for RecordingControl {
        fn terminal_size(&mut self) -> io::Result<TerminalSize> {
            Ok(TerminalSize::new(80, 24))
        }

        fn cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize::new(TerminalSize::new(80, 24), None))
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    impl KeyboardEnhancementQuery for RecordingControl {
        fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
            Ok(true)
        }
    }

    impl TerminalOutput for RecordingControl {
        fn flush(&mut self) -> io::Result<()> {
            self.call("flush")
        }
    }

    fn all_modes() -> SessionOptions {
        SessionOptions {
            raw_mode: true,
            alternate_screen: true,
            bracketed_paste: true,
            focus_change: true,
            keyboard_enhancement: Some(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES),
            mouse_capture: true,
            hide_cursor: true,
        }
    }

    #[test]
    fn explicit_restore_reverses_every_acquired_mode() {
        let mut control = RecordingControl::default();
        let mut session =
            TerminalSession::enter(&mut control, all_modes()).expect("entry succeeds");
        session.restore().expect("restoration succeeds");
        drop(session);

        assert_eq!(
            control.calls,
            [ACQUIRE.as_slice(), RESTORE.as_slice()].concat()
        );
    }

    #[test]
    fn every_partial_entry_failure_restores_the_mode_that_may_have_been_written() {
        for (failed_index, failed) in ACQUIRE.into_iter().enumerate() {
            let mut control = RecordingControl {
                fail_acquire: Some(failed),
                ..RecordingControl::default()
            };
            let error = match TerminalSession::enter(&mut control, all_modes()) {
                Ok(_) => panic!("planned acquisition should fail"),
                Err(error) => error,
            };
            assert_eq!(error.source_error().to_string(), failed);

            let mut expected = ACQUIRE[..=failed_index].to_vec();
            if failed == "flush" {
                expected.extend_from_slice(&RESTORE);
            } else {
                let inverse_start = RESTORE.len() - failed_index - 2;
                expected.extend_from_slice(&RESTORE[inverse_start..]);
            }
            assert_eq!(control.calls, expected, "failed at {failed}");
        }
    }

    #[test]
    fn restoration_reports_the_first_error_and_attempts_every_step() {
        let mut control = RecordingControl::default();
        let mut session =
            TerminalSession::enter(&mut control, all_modes()).expect("entry succeeds");
        session.control_mut().fail_restore = vec!["show_cursor", "disable_focus", "flush"];
        let error = session
            .restore()
            .expect_err("restoration failure is reported");
        assert_eq!(error.to_string(), "show_cursor");
        drop(session);

        assert_eq!(
            control.calls,
            [ACQUIRE.as_slice(), RESTORE.as_slice()].concat()
        );
    }

    #[test]
    fn panic_unwinding_runs_the_same_reverse_restoration() {
        let mut control = RecordingControl::default();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _session =
                TerminalSession::enter(&mut control, all_modes()).expect("entry succeeds");
            panic!("planned panic");
        }));

        assert!(panic.is_err());
        assert_eq!(
            control.calls,
            [ACQUIRE.as_slice(), RESTORE.as_slice()].concat()
        );
    }
}
