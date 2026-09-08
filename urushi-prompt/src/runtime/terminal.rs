//! Terminal-facing contracts and generic prompt-session resource ownership.

use std::io;

use super::{
    error::{IoOperation, RunError},
    form::PromptStart,
    view::PromptView,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    Key(KeyEvent),
    Paste(String),
    Resize { columns: u16, rows: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KeyCode {
    Char(char),
    Enter,
    Escape,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

pub(crate) trait EventSource {
    fn read_event(&mut self) -> io::Result<Event>;

    /// The next event if one is already waiting, without blocking for it.
    ///
    /// This exists so the session can coalesce a burst of resize events into
    /// the size the burst settled on. The default reports nothing waiting,
    /// which makes coalescing a no-op rather than a wrong answer.
    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        Ok(None)
    }
}

pub(crate) trait Renderer {
    fn draw(
        &mut self,
        view: &PromptView,
        start: PromptStart,
        drawing_columns: u16,
    ) -> io::Result<()>;
    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()>;

    /// The terminal width from which the form derives its drawing width.
    ///
    /// The view function chooses the visible window of a value wider than its
    /// drawing area, so the form applies its left edge and optional width cap
    /// before composing anything.
    fn columns(&self) -> u16 {
        80
    }

    fn resize(&mut self, _columns: u16, _rows: u16) {}
}

pub(crate) trait TerminalControl {
    fn is_interactive(&self) -> bool;
    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn enable_bracketed_paste(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn show_cursor(&mut self) -> io::Result<()>;
    fn disable_bracketed_paste(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
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
    terminal: &'a mut T,
    raw_mode: bool,
    bracketed_paste: bool,
    cleaned: bool,
}

impl<'a, R, T> TerminalSession<'a, R, T>
where
    R: Renderer,
    T: TerminalControl,
{
    pub(super) fn enter(renderer: &'a mut R, terminal: &'a mut T) -> Result<Self, RunError> {
        terminal.enable_raw_mode().map_err(|source| RunError::Io {
            operation: IoOperation::EnterTerminal,
            source,
            cleanup: None,
        })?;
        if let Err(source) = terminal.enable_bracketed_paste() {
            let cleanup = terminal.disable_raw_mode().err();
            return Err(RunError::Io {
                operation: IoOperation::EnterTerminal,
                source,
                cleanup,
            });
        }
        Ok(Self {
            renderer,
            terminal,
            raw_mode: true,
            bracketed_paste: true,
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

    pub(super) fn cleanup(&mut self, finish: RenderFinish) -> Option<io::Error> {
        if self.cleaned {
            return None;
        }
        self.cleaned = true;

        let mut first_error = self.renderer.finish(finish).err();
        if let Err(error) = self.terminal.show_cursor() {
            record_first_error(&mut first_error, error);
        }
        if self.bracketed_paste {
            self.bracketed_paste = false;
            if let Err(error) = self.terminal.disable_bracketed_paste() {
                record_first_error(&mut first_error, error);
            }
        }
        if self.raw_mode {
            self.raw_mode = false;
            if let Err(error) = self.terminal.disable_raw_mode() {
                record_first_error(&mut first_error, error);
            }
        }
        if let Err(error) = self.terminal.flush() {
            record_first_error(&mut first_error, error);
        }
        first_error
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
        PromptStyles, RunError, RuntimeField, ViewSpan, field::FieldState, private, test_styles,
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
                PromptLine::spans(vec![ViewSpan::new(
                    self.key.name().to_owned(),
                    &styles.body,
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
    }

    pub(crate) struct RecordingRenderer {
        pub(crate) views: Vec<Option<String>>,
        pub(crate) regions: Vec<(PromptStart, u16)>,
        pub(crate) finishes: Vec<RenderFinish>,
        pub(crate) resizes: Vec<(u16, u16)>,
        pub(crate) columns: u16,
        pub(crate) fail_draw: Option<usize>,
        pub(crate) fail_finish: bool,
    }

    impl Default for RecordingRenderer {
        fn default() -> Self {
            Self {
                views: Vec::new(),
                regions: Vec::new(),
                finishes: Vec::new(),
                resizes: Vec::new(),
                columns: 80,
                fail_draw: None,
                fail_finish: false,
            }
        }
    }

    impl Renderer for RecordingRenderer {
        fn draw(
            &mut self,
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

        fn finish(&mut self, outcome: RenderFinish) -> io::Result<()> {
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

    impl TerminalControl for RecordingTerminal {
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

        fn show_cursor(&mut self) -> io::Result<()> {
            self.calls.push("show_cursor");
            if self.fail_show {
                Err(io::Error::other("show failed"))
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

        fn flush(&mut self) -> io::Result<()> {
            self.calls.push("flush");
            if self.fail_flush {
                Err(io::Error::other("flush failed"))
            } else {
                Ok(())
            }
        }
    }

    pub(crate) fn form(fields: impl IntoIterator<Item = TestField>) -> Form {
        let group = fields
            .into_iter()
            .fold(Group::builder(), |builder, field| builder.field(field))
            .build()
            .expect("test group has fields");
        Form::builder()
            .group(group)
            .build()
            .expect("test form is valid")
    }

    pub(crate) fn enter() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::default(),
        })
    }

    pub(crate) fn back() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::default(),
        })
    }

    pub(crate) fn cancel() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Escape,
            modifiers: KeyModifiers::default(),
        })
    }

    pub(crate) fn ctrl_c() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        })
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
                "show_cursor",
                "disable_raw_mode",
                "flush"
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
                "show_cursor",
                "disable_raw_mode",
                "flush"
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
                "show_cursor",
                "disable_raw_mode",
                "flush"
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
                "show_cursor",
                "disable_raw_mode",
                "flush"
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
