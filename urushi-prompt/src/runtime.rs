use std::{
    any::Any,
    collections::HashMap,
    fmt,
    io::{self, IsTerminal, Write},
    marker::PhantomData,
};

use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as CrosstermEvent,
        KeyCode as CrosstermKeyCode, KeyEventKind,
    },
    execute, queue,
    terminal::{self, Clear, ClearType},
};
use urushi::{ComponentRole, Style, TerminalProfile, Theme};

mod layout;
mod presentation;

use layout::{LaidOutView, RenderedLine};
use presentation::InlinePresentation;

/// A typed name used to retrieve a submitted field value.
pub struct FieldKey<T> {
    name: String,
    marker: PhantomData<fn() -> T>,
}

impl<T> Clone for FieldKey<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            marker: PhantomData,
        }
    }
}

impl<T> PartialEq for FieldKey<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<T> Eq for FieldKey<T> {}

impl<T> std::hash::Hash for FieldKey<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.name, state);
    }
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

impl fmt::Display for IoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EnterTerminal => "enter terminal session",
            Self::ReadEvent => "read terminal event",
            Self::Render => "render prompt",
            Self::Cleanup => "clean up terminal session",
        })
    }
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

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInteractive => formatter
                .write_str("standard input and standard error must be interactive terminals"),
            Self::Io {
                operation,
                source,
                cleanup: Some(cleanup),
            } => write!(
                formatter,
                "failed to {operation}: {source}; terminal cleanup also failed: {cleanup}"
            ),
            Self::Io {
                operation,
                source,
                cleanup: None,
            } => write!(formatter, "failed to {operation}: {source}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotInteractive => None,
            Self::Io { source, .. } => Some(source),
        }
    }
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

impl fmt::Display for FormBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyForm => formatter.write_str("a form must contain at least one group"),
            Self::DuplicateFieldName(name) => {
                write!(formatter, "field name `{name}` is duplicated in the form")
            }
        }
    }
}

impl std::error::Error for FormBuildError {}

/// An invalid group configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupBuildError {
    /// A group must contain at least one field.
    EmptyGroup,
}

impl fmt::Display for GroupBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a prompt group must contain at least one field")
    }
}

impl std::error::Error for GroupBuildError {}

/// A field-specific configuration error reserved for concrete field controls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldConfigError {
    /// A field name cannot be empty.
    EmptyName,
    /// A select-like field must contain an option.
    EmptyOptions,
}

impl fmt::Display for FieldConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyName => "a prompt field name cannot be empty",
            Self::EmptyOptions => "a select field must contain at least one option",
        })
    }
}

impl std::error::Error for FieldConfigError {}

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
    /// The supplied theme and terminal profile are resolved once for the
    /// inline renderer; fields only emit semantic component roles.
    pub fn run<E>(
        self,
        theme: &Theme<E>,
        profile: &TerminalProfile,
    ) -> Result<FormOutcome, RunError> {
        let mut events = CrosstermEventSource;
        let mut renderer =
            CrosstermRenderer::stderr(theme, profile, terminal::size().unwrap_or((80, 24)));
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
        self.groups[0].fields[0].activate();

        loop {
            if let Err(source) = session.renderer.draw(&self.view(&state)) {
                return Err(session.fail(IoOperation::Render, source));
            }

            let event = match events.read_event() {
                Ok(event) => event,
                Err(source) => return Err(session.fail(IoOperation::ReadEvent, source)),
            };
            if let Event::Resize { columns, rows } = &event {
                session.renderer.resize(*columns, *rows);
            }

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
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers { control: true, .. },
            }) => FieldAction::Cancel,
            Event::Key(KeyEvent {
                code: KeyCode::BackTab,
                ..
            }) => FieldAction::Back,
            Event::Key(KeyEvent {
                code: KeyCode::Tab, ..
            }) if self.next_position(group, field).is_none()
                && !self.groups[group].fields[field].captures_tab() =>
            {
                FieldAction::Stay
            }
            event @ Event::Key(KeyEvent {
                code: KeyCode::Escape,
                ..
            }) => match self.groups[group].fields[field].event(event) {
                FieldAction::Stay => FieldAction::Cancel,
                action => action,
            },
            event => self.groups[group].fields[field].event(event),
        };

        match action {
            FieldAction::Stay | FieldAction::Handled => ReducerResult::Running,
            FieldAction::Cancel => {
                *state = FormState::Cancelled;
                ReducerResult::Cancelled
            }
            FieldAction::Back => {
                if let Some((previous_group, previous_field)) = self.previous_position(group, field)
                {
                    self.groups[group].fields[field].deactivate();
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

        let mut lines = Vec::new();
        let mut cursor = None;
        let mut footer = None;
        if let Some(title) = &self.groups[group].title {
            lines.push(ViewLine {
                spans: vec![ViewSpan {
                    text: title.clone(),
                    role: ComponentRole::PromptQuestion,
                }],
            });
        }
        if let Some(description) = &self.groups[group].description {
            lines.push(ViewLine {
                spans: vec![ViewSpan {
                    text: description.clone(),
                    role: ComponentRole::Muted,
                }],
            });
        }
        if !lines.is_empty() {
            lines.push(ViewLine { spans: Vec::new() });
        }
        for (index, entry) in self.groups[group].fields.iter().enumerate() {
            let focused = index == field;
            let mut field_view = entry.view();
            if field_view.lines.last().is_some_and(is_help_line) {
                let help = field_view
                    .lines
                    .pop()
                    .expect("a detected help line is present");
                if focused {
                    footer = Some(help);
                }
            }

            if !focused {
                field_view.cursor = None;
                for line in &mut field_view.lines {
                    line.spans.retain(|span| {
                        span.role != ComponentRole::PromptCursor || span.text != " "
                    });
                    for span in &mut line.spans {
                        match span.role {
                            ComponentRole::PromptCursor => {
                                span.role = ComponentRole::PromptAnswer;
                            }
                            ComponentRole::PromptQuestion => {
                                span.role = ComponentRole::Muted;
                            }
                            _ => {}
                        }
                    }
                }
            }

            let row_offset = lines.len();
            for mut line in field_view.lines {
                line.spans.insert(
                    0,
                    ViewSpan {
                        text: if focused { "┃ " } else { "  " }.to_owned(),
                        role: if focused {
                            ComponentRole::Accent
                        } else {
                            ComponentRole::Body
                        },
                    },
                );
                lines.push(line);
            }
            if focused {
                cursor = field_view.cursor.map(|field_cursor| ViewCursor {
                    row: (row_offset + usize::from(field_cursor.row)).min(usize::from(u16::MAX))
                        as u16,
                    column: field_cursor.column.saturating_add(2),
                });
            }
            if index + 1 < self.groups[group].fields.len() {
                lines.push(ViewLine { spans: Vec::new() });
            }
        }

        if let Some(help) = footer {
            lines.push(ViewLine { spans: Vec::new() });
            lines.push(ViewLine {
                spans: vec![
                    ViewSpan {
                        text: "  ".to_owned(),
                        role: ComponentRole::Body,
                    },
                    ViewSpan {
                        text: help
                            .spans
                            .into_iter()
                            .map(|span| span.text)
                            .collect::<String>(),
                        role: ComponentRole::PromptHelp,
                    },
                ],
            });
        }

        PromptView { lines, cursor }
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
    title: Option<String>,
    description: Option<String>,
}

impl Group {
    /// Starts building a group.
    pub fn builder() -> GroupBuilder {
        GroupBuilder {
            fields: Vec::new(),
            title: None,
            description: None,
        }
    }
}

/// A builder for a non-empty prompt group.
pub struct GroupBuilder {
    fields: Vec<FieldEntry>,
    title: Option<String>,
    description: Option<String>,
}

impl GroupBuilder {
    /// Sets a heading rendered above this group.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets supporting text rendered below the group heading.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

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
            title: self.title,
            description: self.description,
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
}

pub(crate) trait Renderer {
    fn draw(&mut self, view: &PromptView) -> io::Result<()>;
    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()>;

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
    Handled,
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
        self.field.focus();
        self.state = FieldState::Active;
    }

    fn deactivate(&mut self) {
        self.field.blur();
    }

    fn accept(&mut self) {
        self.field.blur();
        self.state = FieldState::Accepted;
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        self.field.take_value()
    }

    fn view(&self) -> PromptView {
        self.field.view()
    }

    fn captures_tab(&self) -> bool {
        self.field.captures_tab()
    }
}

pub(crate) trait RuntimeField {
    fn event(&mut self, event: Event) -> FieldAction;
    fn take_value(&mut self) -> Box<dyn Any>;
    fn view(&self) -> PromptView;

    fn validation_error(&self) -> Option<&str> {
        None
    }

    fn focus(&mut self) {}

    fn blur(&mut self) {}

    fn captures_tab(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptView {
    pub lines: Vec<ViewLine>,
    pub cursor: Option<ViewCursor>,
}

impl PromptView {
    #[cfg(test)]
    fn active_name(&self) -> Option<&str> {
        self.lines
            .iter()
            .find(|line| line.spans.first().is_some_and(|span| span.text == "┃ "))?
            .spans
            .get(1)
            .map(|span| span.text.as_str())
    }
}

fn is_help_line(line: &ViewLine) -> bool {
    !line.spans.is_empty()
        && line
            .spans
            .iter()
            .all(|span| span.role == ComponentRole::PromptHelp)
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
    bracketed_paste: bool,
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
                CrosstermEvent::Paste(text) => return Ok(Event::Paste(text)),
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

struct CrosstermRenderer<W> {
    writer: W,
    styles: PromptStyles,
    presentation: InlinePresentation,
    columns: u16,
    rows: u16,
}

impl CrosstermRenderer<io::Stderr> {
    fn stderr<E>(theme: &Theme<E>, profile: &TerminalProfile, size: (u16, u16)) -> Self {
        Self::new(theme, profile, io::stderr(), size)
    }
}

impl<W: Write> CrosstermRenderer<W> {
    fn new<E>(theme: &Theme<E>, profile: &TerminalProfile, writer: W, size: (u16, u16)) -> Self {
        Self {
            writer,
            styles: PromptStyles::resolve(theme, profile),
            presentation: InlinePresentation::default(),
            columns: size.0.max(1),
            rows: size.1.max(1),
        }
    }

    fn layout(&self, view: &PromptView) -> LaidOutView {
        layout::lay_out(self.columns, self.rows, view)
    }

    fn clear_owned_rows(&mut self, rows: u16) -> io::Result<()> {
        if !self.presentation.origin_saved || rows == 0 {
            return Ok(());
        }

        queue!(self.writer, cursor::RestorePosition)?;
        for row in 0..rows {
            if row == 0 {
                queue!(self.writer, Clear(ClearType::UntilNewLine))?;
            } else {
                queue!(
                    self.writer,
                    cursor::MoveToColumn(0),
                    Clear(ClearType::CurrentLine)
                )?;
            }
            if row + 1 < rows {
                queue!(self.writer, cursor::MoveDown(1))?;
            }
        }
        queue!(self.writer, cursor::RestorePosition)?;
        Ok(())
    }

    fn reserve_owned_rows(&mut self, rows: u16) -> io::Result<()> {
        let rows = rows.max(1);
        if !self.presentation.origin_saved {
            queue!(self.writer, cursor::SavePosition)?;
            self.presentation.origin_saved = true;
            self.presentation.reserved_rows = 1;
        }
        if rows <= self.presentation.reserved_rows {
            return Ok(());
        }

        queue!(self.writer, cursor::RestorePosition)?;
        if self.presentation.reserved_rows > 1 {
            queue!(
                self.writer,
                cursor::MoveDown(self.presentation.reserved_rows - 1)
            )?;
        }
        for _ in self.presentation.reserved_rows..rows {
            self.writer.write_all(b"\n")?;
        }
        if rows > 1 {
            queue!(self.writer, cursor::MoveUp(rows - 1))?;
        }
        queue!(self.writer, cursor::SavePosition)?;
        self.presentation.reserved_rows = rows;
        Ok(())
    }

    fn draw_line(&mut self, line: &RenderedLine) -> io::Result<()> {
        for span in &line.spans {
            self.writer
                .write_all(self.styles.style(span.role).render(&span.text).as_bytes())?;
        }
        Ok(())
    }
}

impl<W: Write> Renderer for CrosstermRenderer<W> {
    fn draw(&mut self, view: &PromptView) -> io::Result<()> {
        let layout = self.layout(view);
        let rows_to_touch = self
            .presentation
            .previous_lines
            .len()
            .max(layout.lines.len())
            .min(usize::from(self.rows)) as u16;
        // Claim every row this redraw can touch before issuing a fallible
        // terminal write. Error cleanup can then erase a partially drawn view.
        self.presentation.previous_rows = rows_to_touch;

        queue!(self.writer, cursor::Hide)?;
        self.reserve_owned_rows(layout.lines.len() as u16)?;

        for row in 0..usize::from(rows_to_touch) {
            let previous = self.presentation.previous_lines.get(row);
            let current = layout.lines.get(row);
            if previous == current {
                continue;
            }

            queue!(self.writer, cursor::RestorePosition)?;
            if row > 0 {
                queue!(
                    self.writer,
                    cursor::MoveDown(row.min(usize::from(u16::MAX)) as u16),
                    cursor::MoveToColumn(0)
                )?;
            }
            queue!(
                self.writer,
                Clear(if row == 0 {
                    ClearType::UntilNewLine
                } else {
                    ClearType::CurrentLine
                })
            )?;
            if let Some(line) = current {
                self.draw_line(line)?;
            }
        }
        self.presentation.previous_lines = layout.lines;
        self.presentation.previous_rows = self.presentation.previous_lines.len() as u16;

        queue!(self.writer, cursor::RestorePosition)?;
        if let Some(cursor) = layout.cursor {
            if cursor.row == 0 {
                queue!(self.writer, cursor::MoveRight(cursor.column))?;
            } else {
                queue!(
                    self.writer,
                    cursor::MoveDown(cursor.row),
                    cursor::MoveToColumn(cursor.column)
                )?;
            }
            queue!(self.writer, cursor::Show)?;
        }
        self.writer.flush()
    }

    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()> {
        match outcome {
            RenderFinish::Submitted => {
                if self.presentation.origin_saved {
                    queue!(self.writer, cursor::RestorePosition)?;
                    if self.presentation.previous_rows > 1 {
                        queue!(
                            self.writer,
                            cursor::MoveDown(self.presentation.previous_rows - 1)
                        )?;
                    }
                    // Relative cursor movement stops at the terminal boundary.
                    // A real line feed scrolls there, preserving the final prompt
                    // row and placing subsequent output below the inline region.
                    self.writer.write_all(b"\r\n")?;
                    queue!(self.writer, cursor::Show)?;
                }
            }
            RenderFinish::Cancelled | RenderFinish::Error | RenderFinish::Panicking => {
                self.clear_owned_rows(self.presentation.previous_rows)?;
                self.presentation.previous_lines.clear();
                if self.presentation.origin_saved {
                    queue!(self.writer, cursor::RestorePosition)?;
                }
            }
        }
        self.writer.flush()
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
    }
}

struct PromptStyles {
    body: Style,
    muted: Style,
    accent: Style,
    question: Style,
    answer: Style,
    placeholder: Style,
    cursor: Style,
    option: Style,
    option_selected: Style,
    button: Style,
    button_focused: Style,
    help: Style,
    error: Style,
}

impl PromptStyles {
    fn resolve<E>(theme: &Theme<E>, profile: &TerminalProfile) -> Self {
        Self {
            body: profile.resolve_style(theme.style(ComponentRole::Body)),
            muted: profile.resolve_style(theme.style(ComponentRole::Muted)),
            accent: profile.resolve_style(theme.style(ComponentRole::Accent)),
            question: profile.resolve_style(theme.style(ComponentRole::PromptQuestion)),
            answer: profile.resolve_style(theme.style(ComponentRole::PromptAnswer)),
            placeholder: profile.resolve_style(theme.style(ComponentRole::PromptPlaceholder)),
            cursor: profile.resolve_style(theme.style(ComponentRole::PromptCursor)),
            option: profile.resolve_style(theme.style(ComponentRole::PromptOption)),
            option_selected: profile
                .resolve_style(theme.style(ComponentRole::PromptOptionSelected)),
            button: profile.resolve_style(theme.style(ComponentRole::PromptButton)),
            button_focused: profile.resolve_style(theme.style(ComponentRole::PromptButtonFocused)),
            help: profile.resolve_style(theme.style(ComponentRole::PromptHelp)),
            error: profile.resolve_style(theme.style(ComponentRole::PromptError)),
        }
    }

    fn style(&self, role: ComponentRole) -> &Style {
        match role {
            ComponentRole::Body => &self.body,
            ComponentRole::Muted => &self.muted,
            ComponentRole::Accent => &self.accent,
            ComponentRole::PromptQuestion => &self.question,
            ComponentRole::PromptAnswer => &self.answer,
            ComponentRole::PromptPlaceholder => &self.placeholder,
            ComponentRole::PromptCursor => &self.cursor,
            ComponentRole::PromptOption => &self.option,
            ComponentRole::PromptOptionSelected => &self.option_selected,
            ComponentRole::PromptButton => &self.button,
            ComponentRole::PromptButtonFocused => &self.button_focused,
            ComponentRole::PromptHelp => &self.help,
            ComponentRole::PromptError => &self.error,
            ComponentRole::Success
            | ComponentRole::Warning
            | ComponentRole::Error
            | ComponentRole::Panel
            | ComponentRole::PanelFocused => &self.body,
        }
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

    fn enable_bracketed_paste(&mut self) -> io::Result<()> {
        execute!(io::stderr(), EnableBracketedPaste)
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        execute!(io::stderr(), cursor::Show)
    }

    fn disable_bracketed_paste(&mut self) -> io::Result<()> {
        execute!(io::stderr(), DisableBracketedPaste)
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
    use crate::runtime::layout::{clip_line, wrap_line};
    use crate::{Confirm, ConfirmAnswer, ConfirmSource, Input, Select, SelectOption};
    use urushi::{AnsiPolicy, Color, ColorProfile, ComponentStyles, SemanticTokens};

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
    fn public_errors_are_contextual_standard_errors() {
        assert_eq!(
            FormBuildError::DuplicateFieldName("name".to_owned()).to_string(),
            "field name `name` is duplicated in the form"
        );
        assert_eq!(
            GroupBuildError::EmptyGroup.to_string(),
            "a prompt group must contain at least one field"
        );
        assert_eq!(
            FieldConfigError::EmptyOptions.to_string(),
            "a select field must contain at least one option"
        );

        let error = RunError::Io {
            operation: IoOperation::Render,
            source: io::Error::other("draw failed"),
            cleanup: Some(io::Error::other("restore failed")),
        };
        assert_eq!(
            error.to_string(),
            "failed to render prompt: draw failed; terminal cleanup also failed: restore failed"
        );
        assert!(std::error::Error::source(&error).is_some());
    }

    #[test]
    fn field_keys_compare_and_hash_without_value_trait_bounds() {
        struct OpaqueValue;

        let first = FieldKey::<OpaqueValue>::new("answer");
        let second = first.clone();
        assert!(first == second);

        let mut keys = std::collections::HashSet::new();
        keys.insert(first);
        assert!(keys.contains(&second));
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
    fn tab_does_not_submit_the_last_field() {
        let confirmation_key = FieldKey::<ConfirmAnswer>::new("confirmation");
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Confirm::new(confirmation_key.clone(), "Continue?", Some(true))
                            .expect("confirm is valid"),
                    )
                    .build()
                    .expect("group is valid"),
            )
            .build()
            .expect("form is valid");
        let mut events = ScriptedEvents::new([
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::default(),
            })),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::default(),
            })),
        ]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        let outcome = form
            .run_with(&mut events, &mut renderer, &mut terminal)
            .expect("form submits after an explicit answer");
        let FormOutcome::Submitted(values) = outcome else {
            panic!("expected submitted form");
        };
        assert_eq!(
            values.get(&confirmation_key),
            Some(&ConfirmAnswer {
                value: false,
                source: ConfirmSource::Explicit,
            })
        );
        assert_eq!(renderer.views.len(), 2);
    }

    #[test]
    fn validated_input_select_and_confirm_submit_typed_values() {
        let name_key = FieldKey::new("name");
        let language_key = FieldKey::new("language");
        let confirmation_key = FieldKey::<ConfirmAnswer>::new("confirmation");
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Input::new(name_key.clone(), "Name", "")
                            .expect("input is valid")
                            .required(),
                    )
                    .field(
                        Select::new(
                            language_key.clone(),
                            "Language",
                            vec![
                                SelectOption::new("Japanese", "ja"),
                                SelectOption::new("English", "en"),
                            ],
                        )
                        .expect("select is valid"),
                    )
                    .field(
                        Confirm::new(confirmation_key.clone(), "Continue?", Some(false))
                            .expect("confirm is valid"),
                    )
                    .build()
                    .expect("group is valid"),
            )
            .build()
            .expect("form is valid");
        let mut events = ScriptedEvents::new([
            Ok(enter()),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('名'),
                modifiers: KeyModifiers::default(),
            })),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::default(),
            })),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::default(),
            })),
            Ok(enter()),
            Ok(Event::Key(KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::default(),
            })),
            Ok(enter()),
        ]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        let outcome = form
            .run_with(&mut events, &mut renderer, &mut terminal)
            .expect("form submits");
        let FormOutcome::Submitted(values) = outcome else {
            panic!("expected submitted values");
        };
        assert_eq!(values.get(&name_key), Some(&"名".to_owned()));
        assert_eq!(values.get(&language_key), Some(&"en"));
        assert_eq!(
            values.get(&confirmation_key),
            Some(&ConfirmAnswer {
                value: true,
                source: ConfirmSource::Explicit,
            })
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

    fn test_theme() -> Theme<()> {
        let tokens = SemanticTokens {
            text: Color::Rgb(1, 2, 3),
            text_muted: Color::Rgb(4, 5, 6),
            background: Color::Rgb(7, 8, 9),
            surface: Color::Rgb(10, 11, 12),
            accent: Color::Rgb(13, 14, 15),
            accent_text: Color::Rgb(16, 17, 18),
            success: Color::Rgb(19, 20, 21),
            warning: Color::Rgb(22, 23, 24),
            error: Color::Rgb(25, 26, 27),
            border: Color::Rgb(28, 29, 30),
        };
        let components = ComponentStyles::from_tokens(&tokens)
            .with_style(ComponentRole::PromptQuestion, Style::new().bold())
            .with_style(ComponentRole::PromptCursor, Style::new().underline());
        Theme::new(tokens, components, ())
    }

    fn renderer_view(lines: Vec<ViewLine>, cursor: Option<ViewCursor>) -> PromptView {
        PromptView { lines, cursor }
    }

    fn view_line(text: &str, role: ComponentRole) -> ViewLine {
        ViewLine {
            spans: vec![ViewSpan {
                text: text.to_owned(),
                role,
            }],
        }
    }

    struct PrefixThenFailWriter {
        bytes: Vec<u8>,
        trigger: Vec<u8>,
        failed: bool,
        fail_next_write: bool,
    }

    impl PrefixThenFailWriter {
        fn new(trigger: &str) -> Self {
            Self {
                bytes: Vec::new(),
                trigger: trigger.as_bytes().to_vec(),
                failed: false,
                fail_next_write: false,
            }
        }
    }

    impl io::Write for PrefixThenFailWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_next_write {
                self.fail_next_write = false;
                return Err(io::Error::other("planned partial draw failure"));
            }
            if !self.failed && bytes == self.trigger {
                self.failed = true;
                self.fail_next_write = true;
                self.bytes.push(bytes[0]);
                return Ok(1);
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn inline_renderer_uses_resolved_theme_styles_and_preserves_mid_line_origin() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![ViewLine {
                    spans: vec![
                        ViewSpan {
                            text: "質問".to_owned(),
                            role: ComponentRole::PromptQuestion,
                        },
                        ViewSpan {
                            text: "＊".to_owned(),
                            role: ComponentRole::PromptCursor,
                        },
                    ],
                }],
                Some(ViewCursor { row: 0, column: 2 }),
            ))
            .expect("renderer writes to a buffer");

        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.contains("\x1b[1m質問\x1b[0m"));
        assert!(output.contains("\x1b[4m＊\x1b[0m"));
        assert!(output.contains("\x1b[?25l\x1b7\x1b8\x1b[K"), "{output:?}");
        assert!(output.contains("\x1b8\x1b[2C"), "{output:?}");
        assert!(!output.contains("\x1b[2J"));
        assert!(!output.contains("?1049"));
    }

    #[test]
    fn first_draw_reserves_rows_before_saving_the_render_origin() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("first", ComponentRole::PromptQuestion),
                    view_line("second", ComponentRole::PromptAnswer),
                    view_line("third", ComponentRole::PromptHelp),
                ],
                None,
            ))
            .expect("renderer reserves and draws three rows");

        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(
            output.contains("\x1b[?25l\x1b7\x1b8\n\n\x1b[2A\x1b7"),
            "{output:?}"
        );
        assert_eq!(renderer.presentation.reserved_rows, 3);
    }

    #[test]
    fn unchanged_frames_only_reposition_the_cursor() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (20, 4));
        let view = renderer_view(
            vec![view_line("stable", ComponentRole::PromptAnswer)],
            Some(ViewCursor { row: 0, column: 2 }),
        );
        renderer.draw(&view).expect("first frame renders");
        let first_frame_bytes = renderer.writer.len();
        renderer.draw(&view).expect("unchanged frame renders");

        let update = String::from_utf8(renderer.writer[first_frame_bytes..].to_vec())
            .expect("renderer writes UTF-8 commands");
        assert!(!update.contains("stable"), "{update:?}");
        assert!(!update.contains("\x1b[K"), "{update:?}");
        assert!(!update.contains("\x1b[2K"), "{update:?}");
        assert!(update.contains("\x1b8\x1b[2C"), "{update:?}");
    }

    #[test]
    fn submitted_prompt_finishes_with_a_scrolling_line_feed() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("answer", ComponentRole::PromptAnswer),
                    view_line("help", ComponentRole::PromptHelp),
                ],
                None,
            ))
            .expect("prompt renders");
        let before_finish = renderer.writer.len();
        renderer
            .finish(RenderFinish::Submitted)
            .expect("submitted prompt finishes");

        let finish = String::from_utf8(renderer.writer[before_finish..].to_vec())
            .expect("renderer writes UTF-8 commands");
        assert!(finish.contains("\x1b8\x1b[1B\r\n"), "{finish:?}");
        assert!(!finish.contains("\x1b[2E"), "{finish:?}");
    }

    #[test]
    fn changing_a_selection_preserves_unrelated_rendered_rows() {
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .title("Settings")
                    .description("Review the values.")
                    .field(Input::new(FieldKey::new("name"), "Name", "value").expect("input"))
                    .field(
                        Select::new(
                            FieldKey::new("choice"),
                            "Choice",
                            vec![SelectOption::new("One", 1), SelectOption::new("Two", 2)],
                        )
                        .expect("select"),
                    )
                    .field(
                        Confirm::new(FieldKey::new("confirm"), "Continue?", Some(true))
                            .expect("confirm"),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (80, 10));
        let before = renderer.layout(&form.view(&state));
        assert_eq!(
            form.reduce(
                &mut state,
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    modifiers: KeyModifiers::default(),
                })
            ),
            ReducerResult::Running
        );
        let after = renderer.layout(&form.view(&state));
        let unchanged = before
            .lines
            .iter()
            .zip(&after.lines)
            .filter(|(before, after)| before == after)
            .count();
        assert!(
            unchanged >= 8,
            "only selection rows should change: {unchanged}"
        );
    }

    #[test]
    fn error_cleanup_clears_rows_claimed_before_a_partial_first_draw() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let writer = PrefixThenFailWriter::new("first");
        let mut renderer = CrosstermRenderer::new(&theme, &profile, writer, (20, 4));
        let view = renderer_view(
            vec![
                view_line("first", ComponentRole::PromptQuestion),
                view_line("second", ComponentRole::PromptOption),
                view_line("third", ComponentRole::PromptError),
            ],
            None,
        );

        assert!(renderer.draw(&view).is_err());
        assert_eq!(renderer.presentation.previous_rows, 3);
        renderer
            .finish(RenderFinish::Error)
            .expect("error cleanup succeeds after one draw failure");

        let output =
            String::from_utf8(renderer.writer.bytes).expect("renderer writes UTF-8 commands");
        assert!(output.contains('f'));
        assert!(output.matches("\x1b[2K").count() >= 2, "{output:?}");
        assert!(output.contains("\x1b[K"), "{output:?}");
        assert!(output.ends_with("\x1b8"), "{output:?}");
    }

    #[test]
    fn error_cleanup_clears_growth_beyond_previous_rows_after_partial_draw() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let mut renderer = CrosstermRenderer::new(
            &theme,
            &profile,
            PrefixThenFailWriter::new("growth"),
            (20, 4),
        );
        renderer
            .draw(&renderer_view(
                vec![view_line("short", ComponentRole::PromptQuestion)],
                None,
            ))
            .expect("initial draw succeeds");
        assert_eq!(renderer.presentation.previous_rows, 1);

        let growth = renderer_view(
            vec![
                view_line("growth", ComponentRole::PromptQuestion),
                view_line("second", ComponentRole::PromptOption),
                view_line("third", ComponentRole::PromptError),
            ],
            None,
        );
        assert!(renderer.draw(&growth).is_err());
        assert_eq!(renderer.presentation.previous_rows, 3);
        renderer
            .finish(RenderFinish::Error)
            .expect("growth cleanup succeeds after one draw failure");

        let output =
            String::from_utf8(renderer.writer.bytes).expect("renderer writes UTF-8 commands");
        assert!(output.matches("\x1b[2K").count() >= 2, "{output:?}");
        assert!(output.contains("\x1b[K"), "{output:?}");
        assert!(output.ends_with("\x1b8"), "{output:?}");
    }

    #[test]
    fn inline_renderer_clears_stale_rows_and_clamps_cjk_cursor_in_narrow_viewports() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (4, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    ViewLine {
                        spans: vec![
                            ViewSpan {
                                text: "名前 ".to_owned(),
                                role: ComponentRole::PromptQuestion,
                            },
                            ViewSpan {
                                text: "あいうえ".to_owned(),
                                role: ComponentRole::PromptAnswer,
                            },
                        ],
                    },
                    view_line("validation message", ComponentRole::PromptError),
                ],
                Some(ViewCursor { row: 0, column: 11 }),
            ))
            .expect("narrow draw succeeds");
        let layout = renderer.layout(&renderer_view(
            vec![view_line("名前 あいうえ", ComponentRole::PromptAnswer)],
            Some(ViewCursor { row: 0, column: 11 }),
        ));
        assert!(layout.lines.len() <= 2);
        assert!(layout.cursor.is_some_and(|cursor| cursor.column < 4));

        renderer.resize(3, 1);
        renderer
            .draw(&renderer_view(
                vec![view_line("短い", ComponentRole::PromptQuestion)],
                None,
            ))
            .expect("redraw succeeds");
        assert_eq!(renderer.presentation.previous_rows, 1);
        renderer
            .finish(RenderFinish::Cancelled)
            .expect("cancel cleanup succeeds");

        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.matches("\x1b[2K").count() >= 1);
        assert!(output.contains("\x1b[K"), "{output:?}");
        assert!(!output.contains("\x1b[2J"));
    }

    #[test]
    fn narrow_viewports_omit_wide_scalars_without_losing_cursor_bounds() {
        let cjk_line = view_line("あ", ComponentRole::PromptCursor);
        assert!(wrap_line(&cjk_line, 1)[0].spans.is_empty());
        assert!(clip_line(&cjk_line, 0, 1).0.spans.is_empty());

        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        for columns in [0, 1] {
            let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (columns, 1));
            let view = renderer_view(
                vec![cjk_line.clone()],
                Some(ViewCursor { row: 0, column: 0 }),
            );
            let layout = renderer.layout(&view);
            assert_eq!(layout.lines.len(), 1);
            assert!(layout.lines[0].spans.is_empty());
            assert_eq!(layout.cursor, Some(ViewCursor { row: 0, column: 0 }));

            renderer.draw(&view).expect("narrow draw succeeds");
            let output =
                String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
            assert!(!output.contains('あ'));
        }
    }

    #[test]
    fn short_viewports_never_replace_the_active_field_with_help() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        for rows in 1..=4 {
            let renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (40, rows));
            let view = renderer_view(
                vec![
                    view_line("previous question", ComponentRole::Muted),
                    view_line("previous answer", ComponentRole::PromptAnswer),
                    view_line("", ComponentRole::Body),
                    view_line("┃ current question", ComponentRole::Accent),
                    ViewLine {
                        spans: vec![
                            ViewSpan {
                                text: "┃ ".to_owned(),
                                role: ComponentRole::Accent,
                            },
                            ViewSpan {
                                text: "› current answer".to_owned(),
                                role: ComponentRole::PromptCursor,
                            },
                        ],
                    },
                    view_line("", ComponentRole::Body),
                    view_line("enter continue", ComponentRole::PromptHelp),
                ],
                Some(ViewCursor { row: 4, column: 18 }),
            );

            let layout = renderer.layout(&view);

            assert_eq!(layout.lines.len(), usize::from(rows));
            assert!(layout.cursor.is_some_and(|cursor| cursor.row < rows));
            assert!(
                layout
                    .lines
                    .iter()
                    .any(|line| line.has_role(ComponentRole::PromptCursor)),
                "active input missing at {rows} rows"
            );
            assert_eq!(
                layout
                    .lines
                    .iter()
                    .any(|line| line.has_role(ComponentRole::PromptHelp)),
                rows >= 3,
                "unexpected help visibility at {rows} rows"
            );
        }
    }

    #[test]
    fn one_row_viewports_show_the_actionable_choice() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (40, 1));
        let view = renderer_view(
            vec![
                view_line("┃ choose a language", ComponentRole::Accent),
                view_line("┃   Japanese", ComponentRole::PromptOption),
                ViewLine {
                    spans: vec![
                        ViewSpan {
                            text: "┃ ".to_owned(),
                            role: ComponentRole::Accent,
                        },
                        ViewSpan {
                            text: "› English".to_owned(),
                            role: ComponentRole::PromptOptionSelected,
                        },
                    ],
                },
                view_line("", ComponentRole::Body),
                view_line("↑/↓ select", ComponentRole::PromptHelp),
            ],
            None,
        );

        let layout = renderer.layout(&view);

        assert_eq!(layout.lines.len(), 1);
        assert!(layout.lines[0].has_role(ComponentRole::PromptOptionSelected));
        assert!(!layout.lines[0].has_role(ComponentRole::PromptHelp));

        let confirm_view = renderer_view(
            vec![
                view_line("┃ continue?", ComponentRole::Accent),
                view_line("┃", ComponentRole::Accent),
                ViewLine {
                    spans: vec![
                        ViewSpan {
                            text: "┃ ".to_owned(),
                            role: ComponentRole::Accent,
                        },
                        ViewSpan {
                            text: "  Yes  ".to_owned(),
                            role: ComponentRole::PromptButtonFocused,
                        },
                        ViewSpan {
                            text: "   No  ".to_owned(),
                            role: ComponentRole::PromptButton,
                        },
                    ],
                },
                view_line("y/n answer", ComponentRole::PromptHelp),
            ],
            None,
        );

        let confirm_layout = renderer.layout(&confirm_view);

        assert_eq!(confirm_layout.lines.len(), 1);
        assert!(confirm_layout.lines[0].has_role(ComponentRole::PromptButtonFocused));
        assert!(!confirm_layout.lines[0].has_role(ComponentRole::PromptHelp));
    }

    #[test]
    fn inline_renderer_applies_terminal_profile_before_writing_styles() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let mut renderer = CrosstermRenderer::new(&theme, &profile, Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![view_line("plain", ComponentRole::PromptQuestion)],
                None,
            ))
            .expect("draw succeeds");
        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(!output.contains("\x1b[1m"));
        assert!(output.contains("plain"));
    }

    fn assert_io_operation(result: Result<FormOutcome, RunError>, expected: IoOperation) {
        match result {
            Err(RunError::Io { operation, .. }) => assert_eq!(operation, expected),
            _ => panic!("expected I/O error"),
        }
    }
}
