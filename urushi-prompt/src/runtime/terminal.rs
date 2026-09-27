//! Terminal-facing contracts and generic prompt-session resource ownership.

use std::io;

use super::{
    error::{IoOperation, RunError},
    form::PromptStart,
    view::PromptView,
};
#[cfg(test)]
pub(crate) use urushi_terminal::EventSource;
#[cfg(test)]
use urushi_terminal::TerminalOutput;
use urushi_terminal::{
    Command, CommandWriter, KeyboardEnhancementQuery, RawModeControl, TerminalBackend,
    TerminalQuery,
};
pub(crate) use urushi_terminal::{Event, KeyCode, KeyEvent, Modifiers as KeyModifiers};

pub(crate) trait Renderer {
    fn draw(
        &mut self,
        output: &mut dyn CommandWriter,
        view: &PromptView,
        start: PromptStart,
        drawing_columns: u16,
    ) -> io::Result<()>;
    fn finish(&mut self, output: &mut dyn CommandWriter, outcome: RenderFinish) -> io::Result<()>;

    /// The terminal width from which the form derives its drawing width.
    ///
    /// The view function chooses the visible window of a value wider than its
    /// drawing area, so the form applies its left edge and optional width cap
    /// before composing anything.
    fn columns(&self) -> u16 {
        80
    }

    fn resize(&mut self, _columns: u16, _rows: u16) {}

    /// Clear the visible primary-buffer viewport and establish a known cursor
    /// origin for the next draw.
    fn clear_viewport(&mut self, output: &mut dyn CommandWriter) -> io::Result<()>;
}

pub(crate) trait TerminalControl:
    RawModeControl + CommandWriter + TerminalQuery + KeyboardEnhancementQuery
{
}

/// Adapts independently scripted input and control halves into one connection.
/// Production backends own both halves directly; tests use this adapter to
/// inject them independently without weakening the runtime contract.
#[cfg(test)]
pub(crate) struct SplitTerminal<'a, E, C> {
    events: &'a mut E,
    control: &'a mut C,
}

#[cfg(test)]
impl<'a, E, C> SplitTerminal<'a, E, C> {
    pub(crate) const fn new(events: &'a mut E, control: &'a mut C) -> Self {
        Self { events, control }
    }
}

#[cfg(test)]
impl<E, C: TerminalOutput> TerminalOutput for SplitTerminal<'_, E, C> {
    fn flush(&mut self) -> io::Result<()> {
        self.control.flush()
    }
}

#[cfg(test)]
impl<E, C: CommandWriter> CommandWriter for SplitTerminal<'_, E, C> {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.control.write_command(command)
    }
}

#[cfg(test)]
impl<E: EventSource, C> EventSource for SplitTerminal<'_, E, C> {
    fn read_event(&mut self) -> io::Result<Event> {
        self.events.read_event()
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.events.poll_event()
    }

    fn poll_event_timeout(&mut self, timeout: std::time::Duration) -> io::Result<Option<Event>> {
        self.events.poll_event_timeout(timeout)
    }
}

#[cfg(test)]
impl<E, C: RawModeControl> RawModeControl for SplitTerminal<'_, E, C> {
    fn is_interactive(&self) -> bool {
        self.control.is_interactive()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        self.control.enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        self.control.disable_raw_mode()
    }
}

#[cfg(test)]
impl<E, C: TerminalQuery> TerminalQuery for SplitTerminal<'_, E, C> {
    fn terminal_size(&mut self) -> io::Result<urushi_terminal::TerminalSize> {
        self.control.terminal_size()
    }

    fn cursor_position(&mut self) -> io::Result<urushi_terminal::Position> {
        self.control.cursor_position()
    }

    fn window_size(&mut self) -> io::Result<urushi_terminal::WindowSize> {
        self.control.window_size()
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        self.control.raw_mode_enabled()
    }
}

#[cfg(test)]
impl<E, C: KeyboardEnhancementQuery> KeyboardEnhancementQuery for SplitTerminal<'_, E, C> {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        self.control.supports_keyboard_enhancement()
    }
}

impl<T> TerminalControl for T where
    T: RawModeControl + CommandWriter + TerminalQuery + KeyboardEnhancementQuery + ?Sized
{
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderFinish {
    Submitted,
    Cancelled,
    Error,
    Panicking,
}

pub(super) struct TerminalSession<'a, R: Renderer, T: TerminalControl> {
    pub(super) renderer: &'a mut R,
    terminal: urushi_terminal::TerminalSession<'a, T>,
    cleaned: bool,
}

impl<'a, R, T> TerminalSession<'a, R, T>
where
    R: Renderer,
    T: TerminalControl,
{
    pub(super) fn enter(renderer: &'a mut R, terminal: &'a mut T) -> Result<Self, RunError> {
        let terminal =
            urushi_terminal::TerminalSession::enter(terminal, inline_prompt_session_options())
                .map_err(|error| {
                    let (source, cleanup) = error.into_parts();
                    RunError::Io {
                        operation: IoOperation::EnterTerminal,
                        source,
                        cleanup,
                    }
                })?;
        Ok(Self {
            renderer,
            terminal,
            cleaned: false,
        })
    }

    pub(super) fn fail(&mut self, operation: IoOperation, source: io::Error) -> RunError {
        let cleanup = self.cleanup(RenderFinish::Error);
        RunError::Io {
            operation,
            source,
            cleanup,
        }
    }

    pub(super) fn draw(
        &mut self,
        view: &PromptView,
        start: PromptStart,
        drawing_columns: u16,
    ) -> io::Result<()> {
        self.renderer
            .draw(self.terminal.control_mut(), view, start, drawing_columns)
    }

    pub(super) fn columns(&self) -> u16 {
        self.renderer.columns()
    }

    pub(super) fn resize(&mut self, columns: u16, rows: u16) {
        self.renderer.resize(columns, rows);
    }

    pub(super) fn clear_viewport(&mut self) -> io::Result<()> {
        self.renderer.clear_viewport(self.terminal.control_mut())
    }

    pub(super) fn cleanup(&mut self, finish: RenderFinish) -> Option<io::Error> {
        if self.cleaned {
            return None;
        }
        self.cleaned = true;

        let mut first_error = self
            .renderer
            .finish(self.terminal.control_mut(), finish)
            .err();
        if let Err(error) = self
            .terminal
            .control_mut()
            .write_command(Command::SetCursorVisible(true))
        {
            record_first_error(&mut first_error, error);
        }
        if let Err(error) = self.terminal.restore() {
            record_first_error(&mut first_error, error);
        }
        first_error
    }
}

impl<R, T> TerminalSession<'_, R, T>
where
    R: Renderer,
    T: TerminalBackend,
{
    pub(super) fn read_event(&mut self) -> io::Result<Event> {
        self.terminal.control_mut().read_event()
    }

    pub(super) fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.terminal.control_mut().poll_event()
    }
}

const fn inline_prompt_session_options() -> urushi_terminal::SessionOptions {
    urushi_terminal::SessionOptions {
        raw_mode: true,
        alternate_screen: false,
        bracketed_paste: true,
        focus_change: false,
        keyboard_enhancement: None,
        mouse_capture: false,
        hide_cursor: false,
    }
}

impl<R: Renderer, T: TerminalControl> Drop for TerminalSession<'_, R, T> {
    fn drop(&mut self) {
        if !self.cleaned {
            let _cleanup_error = self.cleanup(RenderFinish::Panicking);
        }
    }
}

fn record_first_error(first_error: &mut Option<io::Error>, error: io::Error) {
    if first_error.is_none() {
        *first_error = Some(error);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::runtime::{
        FieldAction, FieldEntry, FieldPresentation, Form, FormOutcome, IoOperation, PromptLine,
        PromptStyles, RunError, RuntimeField, TextSpan, field::FieldState, private, test_styles,
    };
    use crate::{FieldKey, Group};
    use std::{
        any::Any,
        collections::VecDeque,
        panic::{AssertUnwindSafe, catch_unwind},
    };
    #[derive(Clone)]
    pub(crate) struct TestField {
        key: FieldKey<String>,
        value: String,
        panic_on_event: bool,
    }

    impl TestField {
        pub(crate) fn new(name: &str, value: &str) -> Self {
            Self {
                key: FieldKey::new(name),
                value: value.to_owned(),
                panic_on_event: false,
            }
        }

        pub(crate) fn panicking(name: &str) -> Self {
            Self {
                panic_on_event: true,
                ..Self::new(name, "value")
            }
        }
    }

    impl private::Sealed for TestField {
        fn into_entry(self: Box<Self>) -> FieldEntry {
            FieldEntry {
                name: self.key.name().to_owned(),
                state: FieldState::Active,
                field: self,
            }
        }
    }

    impl RuntimeField for TestField {
        fn event(&mut self, event: Event) -> FieldAction {
            assert!(!self.panic_on_event, "test field panic");
            match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Enter | KeyCode::Tab,
                    ..
                }) => FieldAction::Accept,
                _ => FieldAction::Stay,
            }
        }

        fn take_value(&mut self) -> Box<dyn Any> {
            Box::new(self.value.clone())
        }

        fn view(&self, styles: &PromptStyles, _focused: bool, _width: usize) -> FieldPresentation {
            FieldPresentation::new(
                PromptLine::spans(vec![TextSpan::new(
                    self.key.name().to_owned(),
                    styles.body.clone(),
                )])
                .view,
            )
        }
    }

    #[derive(Default)]
    pub(crate) struct ScriptedEvents {
        events: VecDeque<io::Result<Event>>,
        /// How many leading events the source reports as already waiting when
        /// it is polled. Everything past them is reachable only by a blocking
        /// read, which is what separates a burst from what the user typed
        /// after it.
        waiting: usize,
    }

    impl ScriptedEvents {
        pub(crate) fn new(events: impl IntoIterator<Item = io::Result<Event>>) -> Self {
            Self {
                events: events.into_iter().collect(),
                waiting: 0,
            }
        }

        /// The same script, with its first `waiting` events arriving as one
        /// burst.
        pub(crate) fn arriving_together(mut self, waiting: usize) -> Self {
            self.waiting = waiting;
            self
        }

        fn take(&mut self) -> io::Result<Event> {
            self.waiting = self.waiting.saturating_sub(1);
            self.events
                .pop_front()
                .unwrap_or_else(|| Err(io::Error::other("event script exhausted")))
        }
    }

    impl EventSource for ScriptedEvents {
        fn read_event(&mut self) -> io::Result<Event> {
            self.take()
        }

        fn poll_event(&mut self) -> io::Result<Option<Event>> {
            if self.waiting == 0 {
                return Ok(None);
            }
            self.take().map(Some)
        }

        fn poll_event_timeout(
            &mut self,
            _timeout: std::time::Duration,
        ) -> io::Result<Option<Event>> {
            self.poll_event()
        }
    }

    pub(crate) struct RecordingRenderer {
        pub(crate) views: Vec<Option<String>>,
        pub(crate) regions: Vec<(PromptStart, u16)>,
        pub(crate) finishes: Vec<RenderFinish>,
        pub(crate) resizes: Vec<(u16, u16)>,
        pub(crate) viewport_clears: usize,
        pub(crate) columns: u16,
        pub(crate) fail_draw: Option<usize>,
        pub(crate) fail_clear_viewport: bool,
        pub(crate) fail_finish: bool,
    }

    impl Default for RecordingRenderer {
        fn default() -> Self {
            Self {
                views: Vec::new(),
                regions: Vec::new(),
                finishes: Vec::new(),
                resizes: Vec::new(),
                viewport_clears: 0,
                columns: 80,
                fail_draw: None,
                fail_clear_viewport: false,
                fail_finish: false,
            }
        }
    }

    impl Renderer for RecordingRenderer {
        fn draw(
            &mut self,
            _output: &mut dyn CommandWriter,
            view: &PromptView,
            start: PromptStart,
            drawing_columns: u16,
        ) -> io::Result<()> {
            self.views.push(view.active_name());
            self.regions.push((start, drawing_columns));
            if self.fail_draw == Some(self.views.len()) {
                return Err(io::Error::other("draw failed"));
            }
            Ok(())
        }

        fn finish(
            &mut self,
            _output: &mut dyn CommandWriter,
            outcome: RenderFinish,
        ) -> io::Result<()> {
            self.finishes.push(outcome);
            if self.fail_finish {
                return Err(io::Error::other("finish failed"));
            }
            Ok(())
        }

        fn resize(&mut self, columns: u16, rows: u16) {
            self.resizes.push((columns, rows));
            self.columns = columns.max(1);
        }

        fn clear_viewport(&mut self, _output: &mut dyn CommandWriter) -> io::Result<()> {
            self.viewport_clears += 1;
            if self.fail_clear_viewport {
                Err(io::Error::other("clear viewport failed"))
            } else {
                Ok(())
            }
        }

        fn columns(&self) -> u16 {
            self.columns
        }
    }

    #[derive(Default)]
    pub(crate) struct RecordingTerminal {
        pub(crate) calls: Vec<&'static str>,
        pub(crate) interactive: bool,
        pub(crate) fail_enable: bool,
        pub(crate) fail_show: bool,
        pub(crate) fail_disable: bool,
        pub(crate) fail_flush: bool,
    }

    impl RecordingTerminal {
        pub(crate) fn interactive() -> Self {
            Self {
                interactive: true,
                ..Self::default()
            }
        }
    }

    impl RawModeControl for RecordingTerminal {
        fn is_interactive(&self) -> bool {
            self.interactive
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            self.calls.push("enable_raw_mode");
            if self.fail_enable {
                Err(io::Error::other("enable failed"))
            } else {
                Ok(())
            }
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            self.calls.push("disable_raw_mode");
            if self.fail_disable {
                Err(io::Error::other("disable failed"))
            } else {
                Ok(())
            }
        }
    }

    impl CommandWriter for RecordingTerminal {
        fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
            if command == Command::SetCursorVisible(true) {
                self.calls.push("show_cursor");
                if self.fail_show {
                    return Err(io::Error::other("show failed"));
                }
            }
            Ok(())
        }
    }

    impl TerminalQuery for RecordingTerminal {
        fn terminal_size(&mut self) -> io::Result<urushi_terminal::TerminalSize> {
            Ok(urushi_terminal::TerminalSize::new(80, 24))
        }

        fn cursor_position(&mut self) -> io::Result<urushi_terminal::Position> {
            Ok(urushi_terminal::Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<urushi_terminal::WindowSize> {
            Ok(urushi_terminal::WindowSize::new(
                urushi_terminal::TerminalSize::new(80, 24),
                None,
            ))
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    impl urushi_terminal::KeyboardEnhancementQuery for RecordingTerminal {
        fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    impl urushi_terminal::TerminalOutput for RecordingTerminal {
        fn flush(&mut self) -> io::Result<()> {
            self.calls.push("flush");
            if self.fail_flush && self.calls.contains(&"show_cursor") {
                Err(io::Error::other("flush failed"))
            } else {
                Ok(())
            }
        }
    }

    pub(crate) fn form(fields: impl IntoIterator<Item = TestField>) -> Form {
        form_with_resize_policy(fields, crate::InlineResizePolicy::default())
    }

    pub(crate) fn form_with_resize_policy(
        fields: impl IntoIterator<Item = TestField>,
        policy: crate::InlineResizePolicy,
    ) -> Form {
        let group = fields
            .into_iter()
            .fold(Group::builder(), |builder, field| builder.field(field))
            .build()
            .expect("test group has fields");
        Form::builder()
            .inline_resize_policy(policy)
            .group(group)
            .build()
            .expect("test form is valid")
    }

    pub(crate) fn enter() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Enter))
    }

    pub(crate) fn back() -> Event {
        Event::Key(KeyEvent::new(KeyCode::BackTab))
    }

    pub(crate) fn cancel() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Escape))
    }

    pub(crate) fn ctrl_c() -> Event {
        Event::Key(KeyEvent::new(KeyCode::Char('c')).with_modifiers(KeyModifiers::CONTROL))
    }

    #[test]
    fn non_interactive_does_not_change_terminal_state() {
        let mut events = ScriptedEvents::default();
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::default();
        assert!(matches!(
            form([TestField::new("field", "value")]).run_with(
                &mut events,
                &mut renderer,
                &mut terminal,
                &test_styles(),
            ),
            Err(RunError::NotInteractive)
        ));
        assert!(terminal.calls.is_empty());
    }

    #[test]
    fn read_and_draw_errors_keep_the_primary_error_and_clean_up_once() {
        let mut events = ScriptedEvents::new([Err(io::Error::other("read failed"))]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();
        let result = form([TestField::new("field", "value")]).run_with(
            &mut events,
            &mut renderer,
            &mut terminal,
            &test_styles(),
        );
        assert_io_operation(result, IoOperation::ReadEvent);
        assert_eq!(renderer.finishes, vec![RenderFinish::Error]);
        assert_eq!(
            terminal.calls,
            [
                "enable_raw_mode",
                "flush",
                "show_cursor",
                "flush",
                "disable_raw_mode"
            ]
        );

        let mut events = ScriptedEvents::default();
        let mut renderer = RecordingRenderer {
            fail_draw: Some(1),
            ..RecordingRenderer::default()
        };
        let mut terminal = RecordingTerminal::interactive();
        let result = form([TestField::new("field", "value")]).run_with(
            &mut events,
            &mut renderer,
            &mut terminal,
            &test_styles(),
        );
        assert_io_operation(result, IoOperation::Render);
        assert_eq!(renderer.finishes, vec![RenderFinish::Error]);
        assert_eq!(
            terminal.calls,
            [
                "enable_raw_mode",
                "flush",
                "show_cursor",
                "flush",
                "disable_raw_mode"
            ]
        );
    }

    #[test]
    fn finish_and_cleanup_errors_do_not_skip_remaining_cleanup_steps() {
        let mut events = ScriptedEvents::new([Ok(enter())]);
        let mut renderer = RecordingRenderer {
            fail_finish: true,
            ..RecordingRenderer::default()
        };
        let mut terminal = RecordingTerminal {
            fail_disable: true,
            fail_flush: true,
            ..RecordingTerminal::interactive()
        };
        let result = form([TestField::new("field", "value")]).run_with(
            &mut events,
            &mut renderer,
            &mut terminal,
            &test_styles(),
        );
        assert_io_operation(result, IoOperation::Cleanup);
        assert_eq!(renderer.finishes, vec![RenderFinish::Submitted]);
        assert_eq!(
            terminal.calls,
            [
                "enable_raw_mode",
                "flush",
                "show_cursor",
                "flush",
                "disable_raw_mode"
            ]
        );
    }

    #[test]
    fn panic_runs_guard_cleanup_once_without_replacing_the_panic() {
        let mut events = ScriptedEvents::new([Ok(enter())]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _result = form([TestField::panicking("field")]).run_with(
                &mut events,
                &mut renderer,
                &mut terminal,
                &test_styles(),
            );
        }));
        assert!(panic.is_err());
        assert_eq!(renderer.finishes, vec![RenderFinish::Panicking]);
        assert_eq!(
            terminal.calls,
            [
                "enable_raw_mode",
                "flush",
                "show_cursor",
                "flush",
                "disable_raw_mode"
            ]
        );
    }
    pub(crate) fn assert_io_operation(
        result: Result<FormOutcome, RunError>,
        expected: IoOperation,
    ) {
        match result {
            Err(RunError::Io { operation, .. }) => assert_eq!(operation, expected),
            _ => panic!("expected I/O error"),
        }
    }
}
