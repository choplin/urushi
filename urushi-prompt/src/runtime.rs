use std::{
    any::Any,
    collections::HashMap,
    fmt,
    io::{self, IsTerminal, Write},
    marker::PhantomData,
};

use crossterm::{
    cursor,
    event::{self, Event as CrosstermEvent, KeyCode as CrosstermKeyCode, KeyEventKind},
    execute, terminal,
};
use urushi::{ComponentRole, TerminalProfile, Theme};

/// A typed name used to retrieve a submitted field value.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct FieldKey<T> {
    name: String,
    marker: PhantomData<fn() -> T>,
}

impl<T> FieldKey<T> {
    /// Creates a field key with a caller-chosen name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            marker: PhantomData,
        }
    }

    /// Returns the field name used to identify this key within a form.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl<T> fmt::Debug for FieldKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FieldKey")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

/// Values made available only after a form is submitted.
pub struct FormValues {
    values: HashMap<String, Box<dyn Any>>,
}

impl FormValues {
    /// Returns the submitted value associated with `key` when its type matches.
    pub fn get<T: 'static>(&self, key: &FieldKey<T>) -> Option<&T> {
        self.values
            .get(key.name())
            .and_then(|value| value.downcast_ref())
    }
}

impl fmt::Debug for FormValues {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FormValues")
            .field("field_count", &self.values.len())
            .finish()
    }
}

/// The terminal-visible outcome of a completed form run.
#[derive(Debug)]
pub enum FormOutcome {
    /// Every field was accepted; values can now be read with [`FieldKey`].
    Submitted(FormValues),
    /// The user cancelled without exposing any partial field values.
    Cancelled,
}

/// The I/O boundary at which a prompt run failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IoOperation {
    /// Entering raw-mode terminal handling failed.
    EnterTerminal,
    /// Reading the next input event failed.
    ReadEvent,
    /// Drawing or finishing the inline prompt failed.
    Render,
    /// Cleanup after an otherwise successful or cancelled run failed.
    Cleanup,
}

/// An error that prevents a form from completing normally.
#[derive(Debug)]
#[non_exhaustive]
pub enum RunError {
    /// Standard input and standard error are not both interactive terminals.
    NotInteractive,
    /// A runtime operation failed; cleanup is retained separately when it also failed.
    Io {
        /// The primary operation that failed.
        operation: IoOperation,
        /// The primary I/O failure.
        source: io::Error,
        /// The first cleanup failure, if cleanup was attempted after `source`.
        cleanup: Option<io::Error>,
    },
}

/// An invalid top-level form configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormBuildError {
    /// A form must contain at least one group.
    EmptyForm,
    /// Field names must be unique across all groups.
    DuplicateFieldName(String),
}

/// An invalid group configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupBuildError {
    /// A group must contain at least one field.
    EmptyGroup,
}

/// A field-specific configuration error reserved for concrete field controls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldConfigError {
    /// A field name cannot be empty.
    EmptyName,
    /// A select-like field must contain an option.
    EmptyOptions,
}

/// A crate-provided prompt field.
///
/// The trait is sealed: applications compose fields supplied by this crate
/// rather than coupling a field implementation to terminal lifecycle details.
#[allow(
    private_bounds,
    reason = "Field stays externally sealed while crate sibling field modules implement its runtime seam."
)]
pub trait Field: private::Sealed + 'static {}

impl<T> Field for T where T: private::Sealed + 'static {}

/// A builder for a blocking prompt form.
pub struct FormBuilder {
    groups: Vec<Group>,
}

impl FormBuilder {
    /// Appends `group` in execution order.
    #[must_use]
    pub fn group(mut self, group: Group) -> Self {
        self.groups.push(group);
        self
    }

    /// Validates group presence and field-name uniqueness.
    pub fn build(self) -> Result<Form, FormBuildError> {
        if self.groups.is_empty() {
            return Err(FormBuildError::EmptyForm);
        }

        let mut names = std::collections::HashSet::new();
        for group in &self.groups {
            for field in &group.fields {
                if !names.insert(field.name().to_owned()) {
                    return Err(FormBuildError::DuplicateFieldName(field.name().to_owned()));
                }
            }
        }

        Ok(Form {
            groups: self.groups,
        })
    }
}

/// A blocking, ordered collection of prompt groups.
pub struct Form {
    groups: Vec<Group>,
}

impl Form {
    /// Starts building a form.
    pub fn builder() -> FormBuilder {
        FormBuilder { groups: Vec::new() }
    }

    /// Runs this form using the process terminal input and standard error.
    ///
    /// The supplied theme and terminal profile establish the stable rendering
    /// boundary for future field renderers. This runtime does not create a
    /// second palette or expose a terminal backend in its public API.
    pub fn run<E>(
        self,
        theme: &Theme<E>,
        profile: &TerminalProfile,
    ) -> Result<FormOutcome, RunError> {
        let _theme = theme;
        let _profile = profile;
        let mut events = CrosstermEventSource;
        let mut renderer = CrosstermRenderer;
        let mut terminal = CrosstermTerminalControl;
        self.run_with(&mut events, &mut renderer, &mut terminal)
    }

    pub(crate) fn run_with<S, R, T>(
        mut self,
        events: &mut S,
        renderer: &mut R,
        terminal: &mut T,
    ) -> Result<FormOutcome, RunError>
    where
        S: EventSource,
        R: Renderer,
        T: TerminalControl,
    {
        if !terminal.is_interactive() {
            return Err(RunError::NotInteractive);
        }

        let mut session = TerminalSession::enter(renderer, terminal)?;
        let mut state = FormState::Running { group: 0, field: 0 };

        loop {
            if let Err(source) = session.renderer.draw(&self.view(&state)) {
                return Err(session.fail(IoOperation::Render, source));
            }

            let event = match events.read_event() {
                Ok(event) => event,
                Err(source) => return Err(session.fail(IoOperation::ReadEvent, source)),
            };

            match self.reduce(&mut state, event) {
                ReducerResult::Running => {}
                ReducerResult::Submitted => {
                    let cleanup = session.cleanup(RenderFinish::Submitted);
                    if let Some(source) = cleanup {
                        return Err(RunError::Io {
                            operation: IoOperation::Cleanup,
                            source,
                            cleanup: None,
                        });
                    }
                    return Ok(FormOutcome::Submitted(self.into_values()));
                }
                ReducerResult::Cancelled => {
                    let cleanup = session.cleanup(RenderFinish::Cancelled);
                    if let Some(source) = cleanup {
                        return Err(RunError::Io {
                            operation: IoOperation::Cleanup,
                            source,
                            cleanup: None,
                        });
                    }
                    return Ok(FormOutcome::Cancelled);
                }
            }
        }
    }

    fn reduce(&mut self, state: &mut FormState, event: Event) -> ReducerResult {
        let FormState::Running { group, field } = *state else {
            unreachable!("the runtime exits immediately after a terminal form state");
        };

        let action = match event {
            Event::Key(KeyEvent {
                code: KeyCode::Escape,
                ..
            })
            | Event::Key(KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers { control: true, .. },
            }) => FieldAction::Cancel,
            Event::Key(KeyEvent {
                code: KeyCode::BackTab,
                ..
            }) => FieldAction::Back,
            event => self.groups[group].fields[field].event(event),
        };

        match action {
            FieldAction::Stay => ReducerResult::Running,
            FieldAction::Cancel => {
                *state = FormState::Cancelled;
                ReducerResult::Cancelled
            }
            FieldAction::Back => {
                if let Some((previous_group, previous_field)) = self.previous_position(group, field)
                {
                    self.groups[previous_group].fields[previous_field].activate();
                    *state = FormState::Running {
                        group: previous_group,
                        field: previous_field,
                    };
                }
                ReducerResult::Running
            }
            FieldAction::Accept => {
                self.groups[group].fields[field].accept();
                if let Some((next_group, next_field)) = self.next_position(group, field) {
                    self.groups[next_group].fields[next_field].activate();
                    *state = FormState::Running {
                        group: next_group,
                        field: next_field,
                    };
                    ReducerResult::Running
                } else {
                    *state = FormState::Submitted;
                    ReducerResult::Submitted
                }
            }
        }
    }

    fn next_position(&self, group: usize, field: usize) -> Option<(usize, usize)> {
        if field + 1 < self.groups[group].fields.len() {
            Some((group, field + 1))
        } else if group + 1 < self.groups.len() {
            Some((group + 1, 0))
        } else {
            None
        }
    }

    fn previous_position(&self, group: usize, field: usize) -> Option<(usize, usize)> {
        if field > 0 {
            Some((group, field - 1))
        } else if group > 0 {
            let previous_group = group - 1;
            Some((previous_group, self.groups[previous_group].fields.len() - 1))
        } else {
            None
        }
    }

    fn view(&self, state: &FormState) -> PromptView {
        let FormState::Running { group, field } = *state else {
            return PromptView {
                lines: Vec::new(),
                cursor: None,
            };
        };
        self.groups[group].fields[field].view()
    }

    fn into_values(mut self) -> FormValues {
        let mut values = HashMap::new();
        for group in &mut self.groups {
            for field in &mut group.fields {
                values.insert(field.name().to_owned(), field.take_value());
            }
        }
        FormValues { values }
    }
}

/// A group of fields executed in builder insertion order.
pub struct Group {
    fields: Vec<FieldEntry>,
}

impl Group {
    /// Starts building a group.
    pub fn builder() -> GroupBuilder {
        GroupBuilder { fields: Vec::new() }
    }
}

/// A builder for a non-empty prompt group.
pub struct GroupBuilder {
    fields: Vec<FieldEntry>,
}

impl GroupBuilder {
    /// Appends a crate-provided field in execution order.
    #[must_use]
    pub fn field<F>(mut self, field: F) -> Self
    where
        F: Field + 'static,
    {
        self.fields
            .push(private::Sealed::into_entry(Box::new(field)));
        self
    }

    /// Validates that this group contains at least one field.
    pub fn build(self) -> Result<Group, GroupBuildError> {
        if self.fields.is_empty() {
            return Err(GroupBuildError::EmptyGroup);
        }
        Ok(Group {
            fields: self.fields,
        })
    }
}

pub(crate) mod private {
    pub(crate) trait Sealed {
        fn into_entry(self: Box<Self>) -> super::FieldEntry;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    Key(KeyEvent),
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
}

pub(crate) trait Renderer {
    fn draw(&mut self, view: &PromptView) -> io::Result<()>;
    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()>;
}

pub(crate) trait TerminalControl {
    fn is_interactive(&self) -> bool;
    fn enable_raw_mode(&mut self) -> io::Result<()>;
    fn show_cursor(&mut self) -> io::Result<()>;
    fn disable_raw_mode(&mut self) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormState {
    Running { group: usize, field: usize },
    Submitted,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldState {
    Active,
    Invalid { message: String },
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldAction {
    Stay,
    Accept,
    Back,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReducerResult {
    Running,
    Submitted,
    Cancelled,
}

pub(crate) struct FieldEntry {
    name: String,
    state: FieldState,
    field: Box<dyn RuntimeField>,
}

impl FieldEntry {
    pub(crate) fn new(name: String, field: Box<dyn RuntimeField>) -> Self {
        Self {
            name,
            state: FieldState::Active,
            field,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn event(&mut self, event: Event) -> FieldAction {
        let action = self.field.event(event);
        self.state = match self.field.validation_error() {
            Some(message) => FieldState::Invalid {
                message: message.to_owned(),
            },
            None => FieldState::Active,
        };
        action
    }

    fn activate(&mut self) {
        self.state = FieldState::Active;
    }

    fn accept(&mut self) {
        self.state = FieldState::Accepted;
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        self.field.take_value()
    }

    fn view(&self) -> PromptView {
        self.field.view()
    }
}

pub(crate) trait RuntimeField {
    fn event(&mut self, event: Event) -> FieldAction;
    fn take_value(&mut self) -> Box<dyn Any>;
    fn view(&self) -> PromptView;

    fn validation_error(&self) -> Option<&str> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptView {
    pub lines: Vec<ViewLine>,
    pub cursor: Option<ViewCursor>,
}

impl PromptView {
    fn active_name(&self) -> Option<&str> {
        self.lines
            .first()?
            .spans
            .first()
            .map(|span| span.text.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewLine {
    pub spans: Vec<ViewSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewSpan {
    pub text: String,
    pub role: ComponentRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewCursor {
    pub row: u16,
    pub column: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderFinish {
    Submitted,
    Cancelled,
    Error,
    Panicking,
}

struct TerminalSession<'a, R: Renderer, T: TerminalControl> {
    renderer: &'a mut R,
    terminal: &'a mut T,
    raw_mode: bool,
    cleaned: bool,
}

impl<'a, R, T> TerminalSession<'a, R, T>
where
    R: Renderer,
    T: TerminalControl,
{
    fn enter(renderer: &'a mut R, terminal: &'a mut T) -> Result<Self, RunError> {
        terminal.enable_raw_mode().map_err(|source| RunError::Io {
            operation: IoOperation::EnterTerminal,
            source,
            cleanup: None,
        })?;
        Ok(Self {
            renderer,
            terminal,
            raw_mode: true,
            cleaned: false,
        })
    }

    fn fail(&mut self, operation: IoOperation, source: io::Error) -> RunError {
        let cleanup = self.cleanup(RenderFinish::Error);
        RunError::Io {
            operation,
            source,
            cleanup,
        }
    }

    fn cleanup(&mut self, finish: RenderFinish) -> Option<io::Error> {
        if self.cleaned {
            return None;
        }
        self.cleaned = true;

        let mut first_error = self.renderer.finish(finish).err();
        if let Err(error) = self.terminal.show_cursor() {
            record_first_error(&mut first_error, error);
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

struct CrosstermEventSource;

impl EventSource for CrosstermEventSource {
    fn read_event(&mut self) -> io::Result<Event> {
        loop {
            match event::read()? {
                CrosstermEvent::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    if let Some(event) = translate_key(key.code, key.modifiers) {
                        return Ok(Event::Key(event));
                    }
                }
                CrosstermEvent::Resize(columns, rows) => {
                    return Ok(Event::Resize { columns, rows });
                }
                _ => {}
            }
        }
    }
}

fn translate_key(
    code: CrosstermKeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> Option<KeyEvent> {
    let code = match code {
        CrosstermKeyCode::Char(character) => KeyCode::Char(character),
        CrosstermKeyCode::Enter => KeyCode::Enter,
        CrosstermKeyCode::Esc => KeyCode::Escape,
        CrosstermKeyCode::Tab => KeyCode::Tab,
        CrosstermKeyCode::BackTab => KeyCode::BackTab,
        CrosstermKeyCode::Backspace => KeyCode::Backspace,
        CrosstermKeyCode::Delete => KeyCode::Delete,
        CrosstermKeyCode::Left => KeyCode::Left,
        CrosstermKeyCode::Right => KeyCode::Right,
        CrosstermKeyCode::Up => KeyCode::Up,
        CrosstermKeyCode::Down => KeyCode::Down,
        CrosstermKeyCode::Home => KeyCode::Home,
        CrosstermKeyCode::End => KeyCode::End,
        _ => return None,
    };
    Some(KeyEvent {
        code,
        modifiers: KeyModifiers {
            shift: modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            control: modifiers.contains(crossterm::event::KeyModifiers::CONTROL),
            alt: modifiers.contains(crossterm::event::KeyModifiers::ALT),
        },
    })
}

struct CrosstermRenderer;

impl Renderer for CrosstermRenderer {
    fn draw(&mut self, view: &PromptView) -> io::Result<()> {
        let _active_name = view.active_name();
        Ok(())
    }

    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()> {
        let _outcome = outcome;
        Ok(())
    }
}

struct CrosstermTerminalControl;

impl TerminalControl for CrosstermTerminalControl {
    fn is_interactive(&self) -> bool {
        io::stdin().is_terminal() && io::stderr().is_terminal()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(io::stderr(), cursor::Show)
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        terminal::disable_raw_mode()
    }

    fn flush(&mut self) -> io::Result<()> {
        io::stderr().flush()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    use super::*;

    #[derive(Clone)]
    struct TestField {
        key: FieldKey<String>,
        value: String,
        panic_on_event: bool,
    }

    impl TestField {
        fn new(name: &str, value: &str) -> Self {
            Self {
                key: FieldKey::new(name),
                value: value.to_owned(),
                panic_on_event: false,
            }
        }

        fn panicking(name: &str) -> Self {
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

        fn view(&self) -> PromptView {
            PromptView {
                lines: vec![ViewLine {
                    spans: vec![ViewSpan {
                        text: self.key.name().to_owned(),
                        role: ComponentRole::Body,
                    }],
                }],
                cursor: None,
            }
        }
    }

    #[derive(Default)]
    struct ScriptedEvents {
        events: VecDeque<io::Result<Event>>,
    }

    impl ScriptedEvents {
        fn new(events: impl IntoIterator<Item = io::Result<Event>>) -> Self {
            Self {
                events: events.into_iter().collect(),
            }
        }
    }

    impl EventSource for ScriptedEvents {
        fn read_event(&mut self) -> io::Result<Event> {
            self.events
                .pop_front()
                .unwrap_or_else(|| Err(io::Error::other("event script exhausted")))
        }
    }

    #[derive(Default)]
    struct RecordingRenderer {
        views: Vec<Option<String>>,
        finishes: Vec<RenderFinish>,
        fail_draw: Option<usize>,
        fail_finish: bool,
    }

    impl Renderer for RecordingRenderer {
        fn draw(&mut self, view: &PromptView) -> io::Result<()> {
            self.views.push(view.active_name().map(str::to_owned));
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
    }

    #[derive(Default)]
    struct RecordingTerminal {
        calls: Vec<&'static str>,
        interactive: bool,
        fail_enable: bool,
        fail_show: bool,
        fail_disable: bool,
        fail_flush: bool,
    }

    impl RecordingTerminal {
        fn interactive() -> Self {
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

    fn form(fields: impl IntoIterator<Item = TestField>) -> Form {
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

    fn enter() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::default(),
        })
    }

    fn back() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::default(),
        })
    }

    fn cancel() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Escape,
            modifiers: KeyModifiers::default(),
        })
    }

    fn ctrl_c() -> Event {
        Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers {
                control: true,
                ..KeyModifiers::default()
            },
        })
    }

    #[test]
    fn builder_rejects_empty_and_duplicate_configuration() {
        assert!(matches!(
            Form::builder().build(),
            Err(FormBuildError::EmptyForm)
        ));
        assert!(matches!(
            Group::builder().build(),
            Err(GroupBuildError::EmptyGroup)
        ));

        let first = Group::builder()
            .field(TestField::new("same", "one"))
            .build()
            .expect("group is valid");
        let second = Group::builder()
            .field(TestField::new("same", "two"))
            .build()
            .expect("group is valid");
        assert!(matches!(
            Form::builder().group(first).group(second).build(),
            Err(FormBuildError::DuplicateFieldName(name)) if name == "same"
        ));
    }

    #[test]
    fn submit_back_and_cancel_follow_the_form_reducer() {
        let mut events = ScriptedEvents::new([Ok(enter()), Ok(back()), Ok(enter()), Ok(enter())]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();
        let outcome = form([
            TestField::new("first", "one"),
            TestField::new("second", "two"),
        ])
        .run_with(&mut events, &mut renderer, &mut terminal)
        .expect("form submits");

        let FormOutcome::Submitted(values) = outcome else {
            panic!("expected submitted form");
        };
        assert_eq!(values.get(&FieldKey::new("first")), Some(&"one".to_owned()));
        assert_eq!(
            values.get(&FieldKey::new("second")),
            Some(&"two".to_owned())
        );
        assert_eq!(
            renderer.views,
            vec![
                Some("first".to_owned()),
                Some("second".to_owned()),
                Some("first".to_owned()),
                Some("second".to_owned())
            ]
        );
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

        let mut events = ScriptedEvents::new([Ok(cancel())]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();
        assert!(matches!(
            form([TestField::new("first", "one")]).run_with(
                &mut events,
                &mut renderer,
                &mut terminal
            ),
            Ok(FormOutcome::Cancelled)
        ));
        assert_eq!(renderer.finishes, vec![RenderFinish::Cancelled]);
        assert_eq!(
            terminal.calls,
            [
                "enable_raw_mode",
                "show_cursor",
                "disable_raw_mode",
                "flush"
            ]
        );

        let mut events = ScriptedEvents::new([Ok(ctrl_c())]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();
        assert!(matches!(
            form([TestField::new("first", "one")]).run_with(
                &mut events,
                &mut renderer,
                &mut terminal
            ),
            Ok(FormOutcome::Cancelled)
        ));
        assert_eq!(renderer.finishes, vec![RenderFinish::Cancelled]);
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
    fn non_interactive_does_not_change_terminal_state() {
        let mut events = ScriptedEvents::default();
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::default();
        assert!(matches!(
            form([TestField::new("field", "value")]).run_with(
                &mut events,
                &mut renderer,
                &mut terminal
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

    fn assert_io_operation(result: Result<FormOutcome, RunError>, expected: IoOperation) {
        match result {
            Err(RunError::Io { operation, .. }) => assert_eq!(operation, expected),
            _ => panic!("expected I/O error"),
        }
    }
}
