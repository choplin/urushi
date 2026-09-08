use std::{
    any::Any,
    collections::HashMap,
    fmt,
    io::{self, IsTerminal, Write},
    marker::PhantomData,
    time::Duration,
};

use crossterm::{
    cursor,
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as CrosstermEvent,
        KeyCode as CrosstermKeyCode, KeyEventKind,
    },
    execute,
    terminal::{self},
};
use urushi::{
    BlockStyle, ComponentRole, Key, Overflow, PrintableText, TerminalProfile, TextStyle, Theme,
    VerticalAlign, View,
};

mod crossterm_executor;
pub(crate) mod frame;
mod inline_plan;
mod presentation;
pub(crate) mod resolve;

use inline_plan::InlineRenderPlan;
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

/// Where a prompt establishes the left edge of its owned region.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum PromptStart {
    /// Starts on a new line at column zero.
    ///
    /// This is the default and always emits a carriage return and line feed
    /// before the first frame.
    #[default]
    NewLine,
    /// Starts at column zero of the cursor's current line, overwriting it.
    CurrentLine,
    /// Starts at the cursor's current position, whose column is supplied by
    /// the caller rather than queried from the terminal. If the terminal later
    /// becomes narrower than that column, the prompt uses its last column.
    CurrentPosition { column: u16 },
}

impl PromptStart {
    const fn column(self) -> u16 {
        match self {
            Self::NewLine | Self::CurrentLine => 0,
            Self::CurrentPosition { column } => column,
        }
    }

    fn within(self, terminal_columns: u16) -> Self {
        match self {
            Self::CurrentPosition { column } => Self::CurrentPosition {
                column: column.min(terminal_columns.max(1) - 1),
            },
            other => other,
        }
    }
}

/// A builder for a blocking prompt form.
pub struct FormBuilder {
    groups: Vec<Group>,
    start: PromptStart,
    width: Option<u16>,
}

impl FormBuilder {
    /// Chooses where the prompt starts.
    ///
    /// The default is [`PromptStart::NewLine`]. The chosen left edge applies
    /// to every row in the prompt region.
    #[must_use]
    pub fn start(mut self, start: PromptStart) -> Self {
        self.start = start;
        self
    }

    /// Limits the prompt's drawing width in terminal cells.
    ///
    /// By default the prompt uses the terminal width remaining after its left
    /// edge. A supplied width is capped at that same available width. Zero is
    /// normalized to the renderer's one-cell minimum.
    #[must_use]
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

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
            start: self.start,
            width: self.width,
        })
    }
}

/// A blocking, ordered collection of prompt groups.
pub struct Form {
    groups: Vec<Group>,
    start: PromptStart,
    width: Option<u16>,
}

impl Form {
    /// Starts building a form.
    pub fn builder() -> FormBuilder {
        FormBuilder {
            groups: Vec::new(),
            start: PromptStart::default(),
            width: None,
        }
    }

    /// Runs this form using the process terminal input and standard error.
    ///
    /// The supplied theme and terminal profile are resolved once into the
    /// prompt's styles, and fields build their view from those resolved
    /// values; no component role reaches the renderer.
    pub fn run(self, theme: &Theme, profile: &TerminalProfile) -> Result<FormOutcome, RunError> {
        let mut events = CrosstermEventSource;
        let mut renderer = CrosstermRenderer::stderr(terminal::size().unwrap_or((80, 24)));
        let mut terminal = CrosstermTerminalControl;
        let styles = PromptStyles::resolve(theme, profile);
        self.run_with(&mut events, &mut renderer, &mut terminal, &styles)
    }

    pub(crate) fn run_with<S, R, T>(
        mut self,
        events: &mut S,
        renderer: &mut R,
        terminal: &mut T,
        styles: &PromptStyles,
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

        // An event drained while coalescing a resize burst, held over to the
        // next iteration rather than dropped.
        let mut deferred: Option<Event> = None;

        loop {
            let terminal_columns = session.renderer.columns();
            let start = self.start.within(terminal_columns);
            let drawing_columns = self.drawing_width(terminal_columns);
            if let Err(source) = session.renderer.draw(
                &self.view(&state, styles, drawing_columns),
                start,
                drawing_columns,
            ) {
                return Err(session.fail(IoOperation::Render, source));
            }

            let mut event = match deferred.take() {
                Some(event) => event,
                None => match events.read_event() {
                    Ok(event) => event,
                    Err(source) => return Err(session.fail(IoOperation::ReadEvent, source)),
                },
            };
            if matches!(event, Event::Resize { .. }) {
                // Dragging a window edge emits a resize per intermediate size.
                // Each one loses the region, so acting on every one multiplies
                // the residue left behind. Only the size the burst settled on
                // is acted upon, which is the layer above's responsibility: the
                // plan stage sees one geometry at a time and cannot enforce it.
                loop {
                    match events.poll_event() {
                        Ok(Some(waiting @ Event::Resize { .. })) => event = waiting,
                        Ok(Some(waiting)) => {
                            deferred = Some(waiting);
                            break;
                        }
                        Ok(None) => break,
                        Err(source) => return Err(session.fail(IoOperation::ReadEvent, source)),
                    }
                }
            }
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

    fn drawing_width(&self, terminal_columns: u16) -> u16 {
        let start = self.start.within(terminal_columns);
        let available = terminal_columns.saturating_sub(start.column()).max(1);
        self.width.unwrap_or(available).max(1).min(available)
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

    fn view(&self, state: &FormState, styles: &PromptStyles, columns: u16) -> PromptView {
        let FormState::Running { group, field } = *state else {
            return PromptView {
                lines: Vec::new(),
                cursor: None,
            };
        };

        // Every field row sits beside a marker gutter, so the width a field
        // lays itself out in is the terminal's less that gutter.
        let field_width = usize::from(columns.max(1)).saturating_sub(GUTTER);

        let mut lines = Vec::new();
        let mut footer = None;
        if let Some(title) = &self.groups[group].title {
            lines.push(PromptLine::spans(vec![ViewSpan::new(
                title.clone(),
                &styles.question,
            )]));
        }
        if let Some(description) = &self.groups[group].description {
            lines.push(PromptLine::spans(vec![ViewSpan::new(
                description.clone(),
                &styles.muted,
            )]));
        }
        if !lines.is_empty() {
            lines.push(PromptLine::blank());
        }
        for (index, entry) in self.groups[group].fields.iter().enumerate() {
            let focused = index == field;
            let mut field_view = entry.view(styles, focused, field_width);
            if focused {
                footer = field_view.help.take().map(PromptLine::new);
            }
            lines.push(PromptLine::field(field_view, focused));
            if index + 1 < self.groups[group].fields.len() {
                lines.push(PromptLine::blank());
            }
        }

        if let Some(help) = footer {
            lines.push(PromptLine::blank());
            lines.push(
                PromptLine::new(gutter_view(styles, false, help.view)).with_kind(LineKind::Help),
            );
        }

        PromptView {
            lines,
            cursor: None,
        }
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

    fn view(&self, styles: &PromptStyles, focused: bool, width: usize) -> FieldPresentation {
        self.field.view(styles, focused, width)
    }

    fn captures_tab(&self) -> bool {
        self.field.captures_tab()
    }
}

pub(crate) trait RuntimeField {
    fn event(&mut self, event: Event) -> FieldAction;
    fn take_value(&mut self) -> Box<dyn Any>;
    /// Builds this field's view with theme and profile already resolved.
    ///
    /// `focused` is passed rather than post-processed out of the result: once
    /// a span carries a resolved style, an unfocused field's rows cannot be
    /// derived from a focused field's rows without guessing which role a
    /// style came from.
    ///
    /// `width` is the cells this field will be laid out in. A field that
    /// carries a cursor needs it: the visible window of a value that is wider
    /// than the terminal is chosen here, before a `View` exists, so that
    /// resolution can be called with the real available area.
    fn view(&self, styles: &PromptStyles, focused: bool, width: usize) -> FieldPresentation;

    fn validation_error(&self) -> Option<&str> {
        None
    }

    fn focus(&mut self) {}

    fn blur(&mut self) {}

    fn captures_tab(&self) -> bool {
        false
    }
}

/// The width a marker gutter takes from the terminal, in cells.
pub(crate) const GUTTER: usize = 2;

/// The marker beside the field the form has focused.
const FOCUS_MARKER: &str = "┃ ";

/// The marker beside every other field, which keeps their text aligned with
/// the focused field's.
const BLANK_MARKER: &str = "  ";

/// A prompt's group context and field presentations in source order.
///
/// Ordinary context occupies one entry per logical line. Each field occupies
/// exactly one entry whose `view` is its complete body and whose prompt-owned
/// region table names anchors inside that body. Resolve can therefore lay a
/// field out once without teaching the generic [`View`] model what a question,
/// control, or validation error means.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PromptView {
    pub lines: Vec<PromptLine>,
    /// Where the terminal cursor sits: `row` is an index into `lines`, and
    /// `column` a cell column inside that line's own rectangle.
    pub cursor: Option<ViewCursor>,
}

impl PromptView {
    /// The focused field's name, read back off the built view.
    #[cfg(test)]
    fn active_name(&self) -> Option<String> {
        let line = self.lines.iter().find(|line| line.active)?;
        let resolved = urushi::resolve(&line.view, urushi::Available::NONE);
        let text: String = resolved
            .rows()
            .first()?
            .iter()
            .map(urushi::StyledGrapheme::symbol)
            .collect();
        Some(
            text.trim_start_matches(FOCUS_MARKER)
                .trim_start_matches(BLANK_MARKER)
                .trim_end()
                .to_owned(),
        )
    }
}

/// One logical line of a prompt, and what the frame stage must know about it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PromptLine {
    pub view: View,
    pub kind: LineKind,
    /// Whether this line belongs to the field the form has focused.
    pub active: bool,
    pub regions: Vec<FieldRegion>,
    pub cursor: Option<Key>,
    pub field: bool,
}

impl PromptLine {
    pub(crate) fn new(view: View) -> Self {
        Self {
            view,
            kind: LineKind::Content,
            active: false,
            regions: Vec::new(),
            cursor: None,
            field: false,
        }
    }

    pub(crate) fn field(presentation: FieldPresentation, active: bool) -> Self {
        Self {
            view: presentation.body,
            kind: LineKind::Content,
            active,
            regions: presentation.regions,
            cursor: presentation.cursor,
            field: true,
        }
    }

    /// A line laid out as one horizontal flow of styled runs.
    pub(crate) fn spans(spans: Vec<ViewSpan>) -> Self {
        Self::new(line_view(spans))
    }

    /// A row that occupies its height and nothing else.
    pub(crate) fn blank() -> Self {
        Self::spans(Vec::new())
    }

    #[must_use]
    pub(crate) fn with_kind(mut self, kind: LineKind) -> Self {
        self.kind = kind;
        self
    }
}

/// Composes styled runs into one line of a prompt.
///
/// A line is always a [`View::Row`], never a bare text leaf: a leaf resolves
/// to the whole available width and pads what it does not use, which would
/// make every row a full-width write. A row resolves to the width its children
/// actually take.
pub(crate) fn line_view(spans: Vec<ViewSpan>) -> View {
    if spans.is_empty() {
        // An empty row has no height at all, and a blank line is a row that
        // draws nothing rather than a line that does not exist.
        return View::row(VerticalAlign::Top, [View::text("", TextStyle::new())]);
    }
    View::row(
        VerticalAlign::Top,
        spans
            .into_iter()
            .map(|span| View::text(span.text, span.style)),
    )
}

/// Composes a line and places the prompt cursor anchor at one display column.
pub(crate) fn line_view_with_cursor(spans: Vec<ViewSpan>, column: usize) -> View {
    let mut before: Vec<ViewSpan> = Vec::new();
    let mut after: Vec<ViewSpan> = Vec::new();
    let mut width = 0;
    for span in spans {
        for grapheme in PrintableText::new(span.text.as_str()).graphemes() {
            let target = if width < column {
                &mut before
            } else {
                &mut after
            };
            match target.last_mut() {
                Some(previous) if previous.style == span.style => {
                    previous.text.push_str(grapheme.as_str());
                }
                _ => target.push(ViewSpan::new(grapheme.as_str(), &span.style)),
            }
            width = width.saturating_add(grapheme.width());
        }
    }
    let children = before
        .into_iter()
        .map(|span| View::text(span.text, span.style))
        .chain([View::anchor(cursor_key())])
        .chain(
            after
                .into_iter()
                .map(|span| View::text(span.text, span.style)),
        );
    View::row(VerticalAlign::Top, children)
}

/// Composes styled runs into a line that is cut, not reflowed, when it is
/// wider than the terminal.
pub(crate) fn clipped_line_view(spans: Vec<ViewSpan>) -> View {
    if spans.is_empty() {
        return line_view(spans);
    }
    View::row(
        VerticalAlign::Top,
        spans.into_iter().map(|span| {
            View::block(
                BlockStyle::new().overflow(Overflow::clip()),
                View::text(span.text, span.style),
            )
        }),
    )
}

/// A run of fixed width, placed beside content that may reflow.
///
/// Sizing it in cells is what pins it: a row shrinks its `Fill` children
/// first, then its auto children, and only then the ones that stated a size.
/// An unpinned marker would be reflowed away with the text it marks.
pub(crate) fn fixed_view(width: usize, spans: Vec<ViewSpan>) -> View {
    View::block(
        BlockStyle::new().width(width.min(usize::from(u16::MAX)) as u16),
        line_view(spans),
    )
}

/// Places `inner` beside the marker that shows whether its field is focused.
///
/// The marker is a column of its own rather than the first run of the line's
/// text, because a line wider than the terminal reflows: as text the marker
/// would be reflowed with it, and rows that continue a wrapped line would
/// start under the marker instead of beside it.
pub(crate) fn gutter_view(styles: &PromptStyles, focused: bool, inner: View) -> View {
    let (marker, style) = if focused {
        (FOCUS_MARKER, &styles.accent)
    } else {
        (BLANK_MARKER, &styles.body)
    };
    View::row(
        VerticalAlign::Top,
        [
            fixed_view(GUTTER, vec![ViewSpan::new(marker, style)]),
            inner,
        ],
    )
}

/// Windows `spans` so that the cell at `cursor` stays visible in `width`
/// cells, returning the visible runs and the cursor's column inside them.
///
/// This is the prompt's horizontal scroll, and it is a text-layer operation:
/// cutting an already-composed run of text at a column is not part of the box
/// model. Doing it here is what lets Resolve be called with the terminal's
/// real width and leaves the Frame stage with no horizontal concern at all.
pub(crate) fn window_spans(
    spans: Vec<ViewSpan>,
    cursor: usize,
    width: usize,
) -> (Vec<ViewSpan>, usize) {
    if width == 0 {
        return (Vec::new(), 0);
    }
    let offset = cursor.saturating_sub(width - 1);
    let mut windowed: Vec<ViewSpan> = Vec::new();
    let mut seen = 0;
    let mut start = None;
    let mut used = 0;
    for span in &spans {
        for grapheme in PrintableText::new(span.text.as_str()).graphemes() {
            let grapheme_width = grapheme.width();
            if seen + grapheme_width <= offset {
                seen += grapheme_width;
                continue;
            }
            // A grapheme wider than the window can never be shown whole, and
            // half of one is not a cell a terminal can draw.
            if grapheme_width > width {
                seen += grapheme_width;
                continue;
            }
            let start = *start.get_or_insert(seen);
            if used + grapheme_width > width {
                return (windowed, cursor.saturating_sub(start));
            }
            match windowed.last_mut() {
                Some(last) if last.style == span.style => last.text.push_str(grapheme.as_str()),
                _ => windowed.push(ViewSpan::new(grapheme.as_str(), &span.style)),
            }
            used += grapheme_width;
            seen += grapheme_width;
        }
    }
    (windowed, cursor.saturating_sub(start.unwrap_or(seen)))
}

/// What the frame policy must know about a row beyond the text it draws.
///
/// Spans carry a resolved [`TextStyle`], so the semantics a row is selected,
/// windowed, or reinstated by cannot be recovered from them: a terminal
/// profile may collapse two roles onto the same style. The classification
/// therefore lives on the line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum LineKind {
    /// An ordinary row with no policy meaning.
    #[default]
    Content,
    /// A selectable row: a select option or a confirm button.
    Choice {
        /// Whether this is the row the field's own selection sits on.
        focused: bool,
    },
    /// The validation-error row, reinstated when scrolled out of view.
    Error,
    /// The help row, clipped rather than wrapped and reinstated when scrolled
    /// out of view.
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldRegionKind {
    Question,
    Description,
    Control,
    Focus,
    Error,
}

impl FieldRegionKind {
    fn key(self) -> Key {
        match self {
            Self::Question => Key::from("prompt-question"),
            Self::Description => Key::from("prompt-description"),
            Self::Control => Key::from("prompt-control"),
            Self::Focus => Key::from("prompt-focus"),
            Self::Error => Key::from("prompt-error"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FieldRegion {
    pub kind: FieldRegionKind,
    pub key: Key,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct FieldPresentation {
    pub body: View,
    pub help: Option<View>,
    pub regions: Vec<FieldRegion>,
    pub cursor: Option<Key>,
}

impl FieldPresentation {
    pub(crate) fn new(body: View) -> Self {
        Self {
            body,
            help: None,
            regions: Vec::new(),
            cursor: None,
        }
    }

    #[must_use]
    pub(crate) fn with_help(mut self, help: View) -> Self {
        self.help = Some(help);
        self
    }

    #[must_use]
    pub(crate) fn with_regions(
        mut self,
        regions: impl IntoIterator<Item = FieldRegionKind>,
    ) -> Self {
        self.regions = regions
            .into_iter()
            .map(|kind| FieldRegion {
                kind,
                key: kind.key(),
            })
            .collect();
        self
    }

    #[must_use]
    pub(crate) fn with_cursor(mut self) -> Self {
        self.cursor = Some(cursor_key());
        self
    }

    #[cfg(test)]
    pub(crate) fn rows(&self) -> Vec<frame::FramedRow> {
        urushi::resolve(&self.body, urushi::Available::NONE)
            .rows()
            .iter()
            .map(|row| frame::FramedRow::aggregate(row))
            .collect()
    }
}

pub(crate) fn region(kind: FieldRegionKind, view: View) -> View {
    View::anchor_block(kind.key(), BlockStyle::new(), view)
}

pub(crate) fn cursor_key() -> Key {
    Key::from("prompt-cursor")
}

pub(crate) fn field_line_view(styles: &PromptStyles, focused: bool, inner: View) -> View {
    gutter_view(styles, focused, inner)
}

/// A run of text before it becomes a [`View::Text`] leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewSpan {
    pub text: String,
    /// The style as it will be emitted: resolved against both the theme and
    /// the terminal profile.
    pub style: TextStyle,
}

impl ViewSpan {
    pub(crate) fn new(text: impl Into<String>, style: &TextStyle) -> Self {
        Self {
            text: text.into(),
            style: style.clone(),
        }
    }
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
            if let Some(event) = translate_event(event::read()?) {
                return Ok(event);
            }
        }
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        while event::poll(Duration::ZERO)? {
            if let Some(event) = translate_event(event::read()?) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }
}

/// The prompt's own event, for the crossterm events it has a use for.
fn translate_event(event: CrosstermEvent) -> Option<Event> {
    match event {
        CrosstermEvent::Key(key)
            if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
        {
            translate_key(key.code, key.modifiers).map(Event::Key)
        }
        CrosstermEvent::Resize(columns, rows) => Some(Event::Resize { columns, rows }),
        CrosstermEvent::Paste(text) => Some(Event::Paste(text)),
        _ => None,
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
    presentation: InlinePresentation,
    columns: u16,
    rows: u16,
}

impl CrosstermRenderer<io::Stderr> {
    fn stderr(size: (u16, u16)) -> Self {
        Self::new(io::stderr(), size)
    }
}

impl<W: Write> CrosstermRenderer<W> {
    fn new(writer: W, size: (u16, u16)) -> Self {
        Self {
            writer,
            presentation: InlinePresentation::default(),
            columns: size.0.max(1),
            rows: size.1.max(1),
        }
    }

    fn present(&mut self, plan: InlineRenderPlan) -> io::Result<()> {
        crossterm_executor::execute(&mut self.writer, &mut self.presentation, plan)
    }

    #[cfg(test)]
    fn draw(&mut self, view: &PromptView) -> io::Result<()> {
        <Self as Renderer>::draw(
            self,
            view,
            PromptStart::CurrentPosition { column: 0 },
            self.columns,
        )
    }
}

impl<W: Write> Renderer for CrosstermRenderer<W> {
    fn draw(
        &mut self,
        view: &PromptView,
        start: PromptStart,
        drawing_columns: u16,
    ) -> io::Result<()> {
        let resolved = resolve::resolve_prompt(drawing_columns, view);
        let framed = frame::frame(&resolved, self.rows);
        let plan = inline_plan::plan_draw(
            framed,
            &self.presentation,
            start,
            drawing_columns,
            self.rows,
        );
        self.present(plan)
    }

    fn columns(&self) -> u16 {
        self.columns
    }

    fn finish(&mut self, outcome: RenderFinish) -> io::Result<()> {
        let plan = inline_plan::plan_finish(outcome, &self.presentation);
        self.present(plan)
    }

    fn resize(&mut self, columns: u16, rows: u16) {
        self.columns = columns.max(1);
        self.rows = rows.max(1);
        // A resize may reflow existing content and push rows past the top of
        // the screen, which invalidates both the saved origin and the row
        // count. Neither can be recovered by inspection, so the region is
        // abandoned; the next frame re-establishes on the cursor's own row.
        self.presentation.lose_region();
    }
}

/// The prompt's component roles resolved against a theme and a terminal
/// profile.
///
/// Views are built from these values rather than from roles, so the style a
/// span carries is the style as it will be emitted. Resolving here — above
/// layout and planning — is what makes two rows that look the same compare
/// equal, whatever the profile collapsed to produce them.
pub(crate) struct PromptStyles {
    pub body: TextStyle,
    pub muted: TextStyle,
    pub accent: TextStyle,
    pub question: TextStyle,
    pub answer: TextStyle,
    pub placeholder: TextStyle,
    pub cursor: TextStyle,
    pub option: TextStyle,
    pub option_selected: TextStyle,
    pub button: TextStyle,
    pub button_focused: TextStyle,
    pub help: TextStyle,
    pub error: TextStyle,
}

impl PromptStyles {
    /// The style a field's question takes; an unfocused field recedes.
    pub(crate) fn question(&self, focused: bool) -> &TextStyle {
        if focused { &self.question } else { &self.muted }
    }

    pub(crate) fn resolve(theme: &Theme, profile: &TerminalProfile) -> Self {
        Self {
            body: profile.resolve_text_style(&theme.text_style(ComponentRole::Body)),
            muted: profile.resolve_text_style(&theme.text_style(ComponentRole::Muted)),
            accent: profile.resolve_text_style(&theme.text_style(ComponentRole::Accent)),
            question: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptQuestion)),
            answer: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptAnswer)),
            placeholder: profile
                .resolve_text_style(&theme.text_style(ComponentRole::PromptPlaceholder)),
            cursor: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptCursor)),
            option: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptOption)),
            option_selected: profile
                .resolve_text_style(&theme.text_style(ComponentRole::PromptOptionSelected)),
            button: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptButton)),
            button_focused: profile
                .resolve_text_style(&theme.text_style(ComponentRole::PromptButtonFocused)),
            help: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptHelp)),
            error: profile.resolve_text_style(&theme.text_style(ComponentRole::PromptError)),
        }
    }
}

/// Styles whose every role is a distinct value, so a test that asserts which
/// role a span was built from cannot pass by coincidence.
#[cfg(test)]
pub(crate) fn test_styles() -> PromptStyles {
    let style = |index: u8| TextStyle::new().foreground(urushi::Color::Rgb(index, 0, 0));
    PromptStyles {
        body: style(1),
        muted: style(2),
        accent: style(3),
        question: style(4),
        answer: style(5),
        placeholder: style(6),
        cursor: style(7),
        option: style(8),
        option_selected: style(9),
        button: style(10),
        button_focused: style(11),
        help: style(12),
        error: style(13),
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
    use crate::runtime::inline_plan::{InlineCommand, RenderState, step};
    use crate::{Confirm, ConfirmAnswer, ConfirmSource, Input, Select, SelectOption};

    /// The plan the renderer would execute for `view`, without writing it.
    fn draw_plan<W>(renderer: &CrosstermRenderer<W>, view: &PromptView) -> InlineRenderPlan {
        draw_plan_at(
            renderer,
            PromptStart::CurrentPosition { column: 0 },
            renderer.columns,
            view,
        )
    }

    fn draw_plan_at<W>(
        renderer: &CrosstermRenderer<W>,
        start: PromptStart,
        drawing_columns: u16,
        view: &PromptView,
    ) -> InlineRenderPlan {
        let framed = lay_out(drawing_columns, renderer.rows, view);
        inline_plan::plan_draw(
            framed,
            &renderer.presentation,
            start,
            drawing_columns,
            renderer.rows,
        )
    }

    /// The state a partially written plan leaves behind: the fold of the
    /// commands that succeeded. Testing recovery this way needs no failure
    /// injection.
    fn fold(previous: &InlinePresentation, commands: &[InlineCommand]) -> InlinePresentation {
        commands
            .iter()
            .fold(RenderState::resuming(previous.clone()), step)
            .presentation
    }

    /// The rows a `columns` x `rows` terminal box shows of `view`.
    fn lay_out(columns: u16, rows: u16, view: &PromptView) -> frame::FramedView {
        frame::frame(&resolve::resolve_prompt(columns, view), rows)
    }
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
    struct ScriptedEvents {
        events: VecDeque<io::Result<Event>>,
        /// How many leading events the source reports as already waiting when
        /// it is polled. Everything past them is reachable only by a blocking
        /// read, which is what separates a burst from what the user typed
        /// after it.
        waiting: usize,
    }

    impl ScriptedEvents {
        fn new(events: impl IntoIterator<Item = io::Result<Event>>) -> Self {
            Self {
                events: events.into_iter().collect(),
                waiting: 0,
            }
        }

        /// The same script, with its first `waiting` events arriving as one
        /// burst.
        fn arriving_together(mut self, waiting: usize) -> Self {
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

    struct RecordingRenderer {
        views: Vec<Option<String>>,
        regions: Vec<(PromptStart, u16)>,
        finishes: Vec<RenderFinish>,
        resizes: Vec<(u16, u16)>,
        columns: u16,
        fail_draw: Option<usize>,
        fail_finish: bool,
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
    fn form_builder_defaults_and_caps_the_prompt_region_width() {
        let group = || {
            Group::builder()
                .field(TestField::new("field", "value"))
                .build()
                .expect("test group has a field")
        };
        let default = Form::builder()
            .group(group())
            .build()
            .expect("default form is valid");
        assert_eq!(default.start, PromptStart::NewLine);
        assert_eq!(default.drawing_width(20), 20);

        let positioned = Form::builder()
            .start(PromptStart::CurrentPosition { column: 7 })
            .width(8)
            .group(group())
            .build()
            .expect("positioned form is valid");
        assert_eq!(positioned.drawing_width(20), 8);
        assert_eq!(positioned.drawing_width(12), 5);

        let oversized = Form::builder()
            .start(PromptStart::CurrentPosition { column: 7 })
            .width(u16::MAX)
            .group(group())
            .build()
            .expect("oversized width is capped at runtime");
        assert_eq!(oversized.drawing_width(20), 13);

        let zero = Form::builder()
            .width(0)
            .group(group())
            .build()
            .expect("zero width is normalized at runtime");
        assert_eq!(zero.drawing_width(20), 1);

        let out_of_bounds = Form::builder()
            .start(PromptStart::CurrentPosition { column: u16::MAX })
            .group(group())
            .build()
            .expect("out-of-bounds start is normalized at runtime");
        assert_eq!(
            out_of_bounds.start.within(20),
            PromptStart::CurrentPosition { column: 19 }
        );
        assert_eq!(out_of_bounds.drawing_width(20), 1);
    }

    #[test]
    fn form_run_passes_the_selected_region_to_the_renderer() {
        let group = Group::builder()
            .field(TestField::new("field", "value"))
            .build()
            .expect("test group has a field");
        let form = Form::builder()
            .start(PromptStart::CurrentPosition { column: 7 })
            .width(12)
            .group(group)
            .build()
            .expect("configured form is valid");
        let mut events = ScriptedEvents::new([Ok(cancel())]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        assert!(matches!(
            form.run_with(&mut events, &mut renderer, &mut terminal, &test_styles(),),
            Ok(FormOutcome::Cancelled)
        ));
        assert_eq!(
            renderer.regions,
            [(PromptStart::CurrentPosition { column: 7 }, 12)]
        );
    }

    #[test]
    fn form_run_clamps_the_start_after_a_narrowing_resize() {
        let group = Group::builder()
            .field(TestField::new("field", "value"))
            .build()
            .expect("test group has a field");
        let form = Form::builder()
            .start(PromptStart::CurrentPosition { column: 7 })
            .group(group)
            .build()
            .expect("configured form is valid");
        let mut events = ScriptedEvents::new([
            Ok(Event::Resize {
                columns: 3,
                rows: 4,
            }),
            Ok(cancel()),
        ]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        assert!(matches!(
            form.run_with(&mut events, &mut renderer, &mut terminal, &test_styles()),
            Ok(FormOutcome::Cancelled)
        ));
        assert_eq!(
            renderer.regions,
            [
                (PromptStart::CurrentPosition { column: 7 }, 73),
                (PromptStart::CurrentPosition { column: 2 }, 1),
            ]
        );
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
        .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
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
                &mut terminal,
                &test_styles(),
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
                &mut terminal,
                &test_styles(),
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
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
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
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
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

    fn test_theme() -> Theme {
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
            .with_text_style(ComponentRole::PromptQuestion, TextStyle::new().bold())
            .with_text_style(ComponentRole::PromptCursor, TextStyle::new().underline());
        Theme::new(tokens, components)
    }

    fn renderer_view(lines: Vec<PromptLine>, cursor: Option<ViewCursor>) -> PromptView {
        PromptView { lines, cursor }
    }

    fn view_line(text: &str, style: &TextStyle) -> PromptLine {
        PromptLine::spans(vec![ViewSpan::new(text, style)])
    }

    /// A row belonging to the focused field, as `Form::view` marks them.
    fn active_line(mut line: PromptLine) -> PromptLine {
        line.active = true;
        line
    }

    /// Fails the `nth` bare line feed, which lands the failure inside the
    /// window where a frame is re-anchoring its origin.
    struct FailOnLineFeedWriter {
        bytes: Vec<u8>,
        line_feeds: usize,
        fail_at: usize,
    }

    impl FailOnLineFeedWriter {
        fn new(nth: usize) -> Self {
            Self {
                bytes: Vec::new(),
                line_feeds: 0,
                fail_at: nth,
            }
        }
    }

    impl io::Write for FailOnLineFeedWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes == b"\n" {
                self.line_feeds += 1;
                if self.line_feeds == self.fail_at {
                    return Err(io::Error::other("planned line feed failure"));
                }
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
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
    fn inline_renderer_uses_resolved_theme_styles_and_anchors_on_the_current_row() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![PromptLine::spans(vec![
                ViewSpan::new("質問", &styles.question),
                ViewSpan::new("＊", &styles.cursor),
            ])],
            Some(ViewCursor { row: 0, column: 2 }),
        );

        // A current-position start anchors on the row the cursor already
        // occupies. Clearing begins at that origin and leaves its left side
        // untouched.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );

        renderer.draw(&view).expect("renderer writes to a buffer");

        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.contains("\x1b[1m質問\x1b[0m"));
        assert!(output.contains("\x1b[4m＊\x1b[0m"));
        assert!(output.contains("\x1b[K"));
        assert!(!output.contains("\x1b[2K"));
    }

    #[test]
    fn a_new_line_prompt_comes_out_to_a_fresh_row_first() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![view_line("question", &styles.question)],
            Some(ViewCursor { row: 0, column: 2 }),
        );

        // NewLine always reaches a fresh row before anchoring it, without
        // querying where the cursor happened to be.
        let plan = draw_plan_at(&renderer, PromptStart::NewLine, renderer.columns, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::CarriageReturnLineFeed,
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );

        // The row the carriage return and line feed reached is the region top,
        // so the fold counts one reserved row rather than the two it passed
        // through.
        let anchored = fold(&renderer.presentation, &plan.commands[..3]);
        assert!(anchored.anchored);
        assert_eq!(anchored.reserved_rows, 1);
        assert_eq!(plan.next.reserved_rows, 1);

        <CrosstermRenderer<_> as Renderer>::draw(&mut renderer, &view, PromptStart::NewLine, 20)
            .expect("renderer writes to a buffer");
        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(output.starts_with("\x1b[?25l\r\n"), "{output:?}");
    }

    #[test]
    fn every_prompt_start_has_an_explicit_first_frame_plan() {
        let styles = test_styles();
        let renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(vec![view_line("question", &styles.question)], None);

        let cases = [
            (
                PromptStart::NewLine,
                Some(InlineCommand::CarriageReturnLineFeed),
            ),
            (
                PromptStart::CurrentLine,
                Some(InlineCommand::MoveToColumn(0)),
            ),
            (PromptStart::CurrentPosition { column: 4 }, None),
        ];
        for (start, initial) in cases {
            let columns = 20_u16.saturating_sub(start.column()).max(1);
            let plan = draw_plan_at(&renderer, start, columns, &view);
            let mut expected = vec![InlineCommand::HideCursor];
            expected.extend(initial);
            expected.extend([
                InlineCommand::SavePosition,
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::Write(plan.next.rows[0].clone()),
                InlineCommand::RestorePosition,
            ]);
            assert_eq!(plan.commands, expected, "{start:?}");
        }
    }

    #[test]
    fn start_column_and_width_change_the_wrap_bound_before_resolve() {
        let styles = test_styles();
        let view = renderer_view(
            vec![view_line(
                "a question whose words wrap at the supplied boundary",
                &styles.question,
            )],
            None,
        );
        let group = || {
            Group::builder()
                .field(TestField::new("field", "value"))
                .build()
                .expect("test group has a field")
        };
        let full = Form::builder()
            .group(group())
            .build()
            .expect("full-width form");
        let positioned = Form::builder()
            .start(PromptStart::CurrentPosition { column: 12 })
            .group(group())
            .build()
            .expect("positioned form");
        let limited = Form::builder()
            .width(8)
            .group(group())
            .build()
            .expect("width-limited form");
        let oversized = Form::builder()
            .width(99)
            .group(group())
            .build()
            .expect("width-capped form");

        let row_count = |form: &Form| {
            let width = form.drawing_width(20);
            resolve::resolve_prompt(width, &view).lines[0]
                .view
                .rows()
                .len()
        };
        assert_eq!(positioned.drawing_width(20), 8);
        assert!(row_count(&positioned) > row_count(&full));
        assert_eq!(limited.drawing_width(20), 8);
        assert_eq!(row_count(&limited), row_count(&positioned));
        assert_eq!(oversized.drawing_width(20), 20);
        assert_eq!(row_count(&oversized), row_count(&full));
    }

    /// Every plan the inline renderer can produce, named for failure messages.
    ///
    /// The canonical-list invariants below hold for the vocabulary as a whole,
    /// not for one frame, so they are asserted over the whole set rather than
    /// re-derived per test.
    fn every_plan_shape() -> Vec<(&'static str, Vec<InlineCommand>)> {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let one_row = |cursor| renderer_view(vec![view_line("only", &styles.question)], cursor);
        let three_rows = |cursor| {
            renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.answer),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                cursor,
            )
        };

        let mut plans = Vec::new();

        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        plans.push((
            "first frame, cursor at the origin",
            draw_plan(&renderer, &one_row(Some(ViewCursor { row: 0, column: 0 }))).commands,
        ));
        plans.push((
            "first frame, three rows",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 2, column: 4 })),
            )
            .commands,
        ));

        plans.push((
            "first frame, new line",
            draw_plan_at(
                &renderer,
                PromptStart::NewLine,
                renderer.columns,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));
        plans.push((
            "first frame, current line",
            draw_plan_at(
                &renderer,
                PromptStart::CurrentLine,
                renderer.columns,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));
        plans.push((
            "first frame, current position",
            draw_plan_at(
                &renderer,
                PromptStart::CurrentPosition { column: 4 },
                renderer.columns - 4,
                &one_row(Some(ViewCursor { row: 0, column: 3 })),
            )
            .commands,
        ));

        renderer
            .draw(&one_row(Some(ViewCursor { row: 0, column: 1 })))
            .expect("first frame renders");
        plans.push((
            "growth from one row to three",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 1, column: 2 })),
            )
            .commands,
        ));

        renderer
            .draw(&three_rows(Some(ViewCursor { row: 1, column: 2 })))
            .expect("grown frame renders");
        plans.push((
            "redraw of a changed middle row",
            draw_plan(
                &renderer,
                &renderer_view(
                    vec![
                        view_line("first", &styles.question),
                        view_line("changed", &styles.answer),
                        view_line("third", &styles.help).with_kind(LineKind::Help),
                    ],
                    Some(ViewCursor { row: 2, column: 0 }),
                ),
            )
            .commands,
        ));
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
        ] {
            plans.push((
                match outcome {
                    RenderFinish::Submitted => "finish: submitted",
                    RenderFinish::Cancelled => "finish: cancelled",
                    _ => "finish: error",
                },
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
            ));
        }

        renderer.resize(20, 4);
        plans.push((
            "re-establishing after a lost region",
            draw_plan(
                &renderer,
                &three_rows(Some(ViewCursor { row: 0, column: 0 })),
            )
            .commands,
        ));

        plans
    }

    #[test]
    fn no_plan_emits_a_movement_of_zero_distance() {
        for (name, commands) in every_plan_shape() {
            for command in &commands {
                let zero = matches!(
                    command,
                    InlineCommand::MoveUp(0)
                        | InlineCommand::MoveDown(0)
                        | InlineCommand::MoveRight(0)
                );
                assert!(!zero, "{name}: {commands:?}");
            }
        }
    }

    #[test]
    fn every_write_is_positioned_and_cleared_the_same_way() {
        let mut writes = 0;
        for (name, commands) in every_plan_shape() {
            for (index, command) in commands.iter().enumerate() {
                if !matches!(command, InlineCommand::Write(_)) {
                    continue;
                }
                writes += 1;
                // Position, clear, write — the same three steps for every row,
                // with no column command and no first-row special case. The
                // clear immediately before the write is what the recovery
                // contract rests on.
                let positioning = matches!(
                    commands[..index],
                    [.., InlineCommand::RestorePosition, InlineCommand::ClearLine]
                        | [
                            ..,
                            InlineCommand::RestorePosition,
                            InlineCommand::MoveDown(_),
                            InlineCommand::ClearLine,
                        ]
                );
                assert!(positioning, "{name}: row form at {index} in {commands:?}");
            }
        }
        assert!(writes >= 8, "the shapes must actually write rows: {writes}");
    }

    #[test]
    fn a_write_is_always_followed_by_an_absolute_reposition() {
        for (name, commands) in every_plan_shape() {
            let mut written = false;
            for command in &commands {
                match command {
                    InlineCommand::Write(_) => written = true,
                    // The cursor's position after a write that reaches the
                    // right margin is terminal-dependent, so nothing relative
                    // may follow one until an absolute command has removed the
                    // ambiguity.
                    InlineCommand::MoveUp(_)
                    | InlineCommand::MoveDown(_)
                    | InlineCommand::MoveRight(_) => {
                        assert!(
                            !written,
                            "{name}: relative move after a write in {commands:?}"
                        )
                    }
                    InlineCommand::RestorePosition
                    | InlineCommand::MoveToColumn(_)
                    | InlineCommand::CarriageReturnLineFeed => written = false,
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn first_draw_reserves_rows_before_saving_the_render_origin() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.answer),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );

        // Two bare line feeds scroll the extra rows into existence, the cursor
        // returns to the top of them, and only then is the origin anchored. No
        // origin is saved before the line feeds: a first frame has nothing to
        // return to, and a position saved ahead of them would be the very one
        // a line feed at the bottom of the screen invalidates.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands[..4],
            [
                InlineCommand::HideCursor,
                InlineCommand::LineFeed,
                InlineCommand::LineFeed,
                InlineCommand::MoveUp(2),
            ]
        );
        assert_eq!(plan.commands[4], InlineCommand::SavePosition);
        assert_eq!(
            plan.commands
                .iter()
                .position(|command| *command == InlineCommand::SavePosition),
            Some(4),
            "the frame saves the origin once, after the rows are reserved"
        );
        // Mid-growth the region is unanchored: the line feeds may have
        // scrolled the display, and no saved position survives that.
        let growing = fold(&renderer.presentation, &plan.commands[..2]);
        assert!(!growing.anchored);
        // No command carries the anchor or the row count: folding the prefix
        // that ends at the closing SavePosition derives both. Nothing has been
        // written yet, so the region is reserved but not yet owned.
        let reserved = fold(&renderer.presentation, &plan.commands[..5]);
        assert!(reserved.anchored);
        assert_eq!(reserved.reserved_rows, 3);
        assert_eq!(reserved.owned_rows, 0);
        assert!(!reserved.drawn);

        renderer
            .draw(&view)
            .expect("renderer reserves and draws three rows");
        assert_eq!(renderer.presentation.reserved_rows, 3);
    }

    #[test]
    fn unchanged_frames_only_reposition_the_cursor() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let view = renderer_view(
            vec![view_line("stable", &styles.answer)],
            Some(ViewCursor { row: 0, column: 2 }),
        );
        renderer.draw(&view).expect("first frame renders");

        // Nothing is cleared and nothing is rewritten; the frame only puts the
        // cursor back.
        let plan = draw_plan(&renderer, &view);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::HideCursor,
                InlineCommand::RestorePosition,
                InlineCommand::MoveRight(2),
                InlineCommand::ShowCursor,
            ]
        );
        // A frame that only repositions leaves the region exactly as it was,
        // whichever command it failed on.
        for k in 0..=plan.commands.len() {
            assert_eq!(
                fold(&renderer.presentation, &plan.commands[..k]),
                renderer.presentation
            );
        }

        let first_frame_bytes = renderer.writer.len();
        renderer.draw(&view).expect("unchanged frame renders");
        let update = String::from_utf8(renderer.writer[first_frame_bytes..].to_vec())
            .expect("renderer writes UTF-8 commands");
        assert!(!update.contains("stable"), "{update:?}");
    }

    #[test]
    fn submitted_prompt_finishes_with_a_scrolling_line_feed() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("answer", &styles.answer),
                    view_line("help", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ))
            .expect("prompt renders");

        // Step down to the last owned row, then release the region with a real
        // line feed. Relative movement would stop at the terminal boundary
        // instead of scrolling, so the final row must not be re-entered with a
        // MoveDown-style command.
        let plan = inline_plan::plan_finish(RenderFinish::Submitted, &renderer.presentation);
        assert_eq!(
            plan.commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::MoveDown(1),
                InlineCommand::CarriageReturnLineFeed,
                InlineCommand::ShowCursor,
            ]
        );

        renderer
            .finish(RenderFinish::Submitted)
            .expect("submitted prompt finishes");
    }

    #[test]
    fn a_wrapped_question_is_omitted_whole_before_focus_error_and_help() {
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .field(Input::new(FieldKey::new("first"), "First", "one").expect("input"))
                    .field(
                        Input::new(
                            FieldKey::new("second"),
                            "A question long enough that it wraps across several terminal rows",
                            "value",
                        )
                        .expect("input")
                        .help("enter continue")
                        .validate(Box::new(|_| {
                            Err(crate::ValidationError::new("Not acceptable."))
                        })),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);
        // Enter on the focused field is refused and raises its error row.
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);

        let styles = test_styles();
        let laid_out = lay_out(20, 3, &form.view(&state, &styles, 20));
        let drawn = laid_out
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();
        assert!(drawn.iter().any(|line| line.contains("Not acceptable")));
        assert!(drawn.iter().any(|line| line.contains("› value")));
        assert!(!drawn.iter().any(|line| line.contains("A question")));
        assert!(drawn.iter().any(|line| line.contains("enter continue")));
    }

    #[test]
    fn a_validation_error_wider_than_the_terminal_is_wrapped_not_cut() {
        // The regression guard for resolving against the terminal's real
        // width. Resolving unbounded would make the error row as wide as its
        // message and silently lose everything past the last column.
        const MESSAGE: &str = "That value is not one this field will accept.";
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Input::new(FieldKey::new("name"), "Name", "value")
                            .expect("input")
                            .validate(Box::new(|_| Err(crate::ValidationError::new(MESSAGE)))),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        // Enter is refused, which is what raises the validation error row.
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);

        let styles = test_styles();
        let view = form.view(&state, &styles, 20);
        let framed = lay_out(20, 20, &view);
        let drawn = framed
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();

        assert!(
            drawn.iter().all(|row| row.chars().count() < MESSAGE.len()),
            "the error row was never wrapped: {drawn:?}"
        );
        let joined = drawn
            .iter()
            .map(|row| row.trim().to_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            joined.contains(MESSAGE),
            "wrapping lost part of the message: {joined:?}"
        );
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
        let styles = PromptStyles::resolve(&theme, &profile);
        let renderer = CrosstermRenderer::new(Vec::new(), (80, 10));
        let before = lay_out(
            renderer.columns,
            renderer.rows,
            &form.view(&state, &styles, renderer.columns),
        );
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
        let after = lay_out(
            renderer.columns,
            renderer.rows,
            &form.view(&state, &styles, renderer.columns),
        );
        let unchanged = before
            .rows
            .iter()
            .zip(&after.rows)
            .filter(|(before, after)| before == after)
            .count();
        assert!(
            unchanged >= 8,
            "only selection rows should change: {unchanged}"
        );
    }

    #[test]
    fn recovery_state_is_the_fold_of_the_commands_that_succeeded() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let plan = draw_plan(
            &renderer,
            &renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.option),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ),
        );

        // The state at any failure point is the fold of the prefix that
        // succeeded, so it can be asserted without injecting a failure. Rows
        // are reserved before anything is owned, and each row becomes owned by
        // the clear that precedes its write.
        let expected = [
            (0, false, 0, 0, false),
            (2, false, 2, 0, false),
            (5, true, 3, 0, false),
            (8, true, 3, 1, true),
            (12, true, 3, 2, true),
            (16, true, 3, 3, true),
            (plan.commands.len(), true, 3, 3, true),
        ];
        for (prefix, anchored, reserved_rows, owned_rows, drawn) in expected {
            let state = fold(&renderer.presentation, &plan.commands[..prefix]);
            assert_eq!(
                (
                    state.anchored,
                    state.reserved_rows,
                    state.owned_rows,
                    state.drawn
                ),
                (anchored, reserved_rows, owned_rows, drawn),
                "fold of the first {prefix} commands"
            );
        }

        // A completed frame collapses the owned extent to its own height, which
        // the commands do not carry; only the planner knows it.
        assert_eq!(plan.next.owned_rows, 3);
        assert!(plan.next.drawn);
    }

    #[test]
    fn a_failure_while_the_origin_is_being_anchored_abandons_the_region() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(FailOnLineFeedWriter::new(2), (20, 4));

        assert!(
            renderer
                .draw(&renderer_view(
                    vec![
                        view_line("first", &styles.question),
                        view_line("second", &styles.option),
                        view_line("third", &styles.help).with_kind(LineKind::Help),
                    ],
                    None,
                ))
                .is_err()
        );

        // The failure landed between the first line feed and the SaveOrigin
        // that would have ended the window, so no origin is saved and the rows
        // the first line feed scrolled in cannot be located again. The region
        // is abandoned rather than erased at a guessed position.
        assert!(!renderer.presentation.anchored);
        assert_eq!(renderer.presentation.reserved_rows, 0);
        assert_eq!(renderer.presentation.owned_rows, 0);
        assert!(renderer.presentation.rows.is_empty());
        // Nothing was written, so there is no residue either: cleanup emits
        // nothing at all, not even the closing line feed that a lost region
        // with content on screen would need.
        assert!(!renderer.presentation.drawn);
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
            RenderFinish::Panicking,
        ] {
            assert_eq!(
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
                [],
                "{outcome:?} cleanup after a region that was never anchored"
            );
        }

        renderer
            .finish(RenderFinish::Error)
            .expect("cleanup of an abandoned region writes nothing");
    }

    #[test]
    fn a_resize_abandons_the_region_and_cleanup_pushes_below_the_residue() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![
                    view_line("first", &styles.question),
                    view_line("second", &styles.option),
                    view_line("third", &styles.help).with_kind(LineKind::Help),
                ],
                None,
            ))
            .expect("first draw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 3);

        renderer.resize(30, 6);
        // A resize may reflow content and move everything the origin pointed
        // at, so the region goes with it. `drawn` is the one field that
        // survives, because the residue on screen is not undone by a resize.
        assert!(!renderer.presentation.anchored);
        assert_eq!(renderer.presentation.reserved_rows, 0);
        assert_eq!(renderer.presentation.owned_rows, 0);
        assert!(renderer.presentation.drawn);

        // Cancelling immediately afterwards has no rows to erase, yet the
        // residue is still on screen. Gating the closing line feed on
        // owned_rows would skip it and let the next output land on top of that
        // residue; the gate is drawn, which the loss did not reset.
        for outcome in [
            RenderFinish::Submitted,
            RenderFinish::Cancelled,
            RenderFinish::Error,
            RenderFinish::Panicking,
        ] {
            assert_eq!(
                inline_plan::plan_finish(outcome, &renderer.presentation).commands,
                [
                    InlineCommand::ShowCursor,
                    InlineCommand::CarriageReturnLineFeed,
                ],
                "{outcome:?} cleanup after a lost region"
            );
        }

        renderer
            .finish(RenderFinish::Cancelled)
            .expect("cleanup after a resize succeeds");
    }

    #[test]
    fn a_redraw_after_a_resize_re_establishes_on_the_cursor_row() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let tall = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );
        renderer.draw(&tall).expect("first draw succeeds");
        renderer.resize(20, 4);

        // A prompt that is still running re-establishes on the cursor's own
        // row, overwriting it. It does not push below the residue first: that
        // is what a finishing prompt does, and doing it here would add a blank
        // row on every resize.
        let plan = draw_plan(&renderer, &tall);
        assert_eq!(
            plan.commands[..5],
            [
                InlineCommand::HideCursor,
                InlineCommand::MoveToColumn(0),
                InlineCommand::LineFeed,
                InlineCommand::LineFeed,
                InlineCommand::MoveUp(2),
            ]
        );
        assert_eq!(plan.commands[5], InlineCommand::SavePosition);
        assert!(
            !plan
                .commands
                .contains(&InlineCommand::CarriageReturnLineFeed),
            "a redraw never releases the terminal below the region"
        );
        // The old origin is gone, so nothing restores to it before the frame
        // has saved a new one.
        assert!(
            !plan.commands[..5].contains(&InlineCommand::RestorePosition),
            "a lost region has no origin to return to"
        );

        // A single-row prompt overwrites the whole of what it had, so it leaves
        // no residue at all: the re-established region is exactly the cursor's
        // row.
        renderer.resize(20, 4);
        let short = draw_plan(
            &renderer,
            &renderer_view(vec![view_line("only", &styles.question)], None),
        );
        assert_eq!(
            short.commands[..3],
            [
                InlineCommand::HideCursor,
                InlineCommand::MoveToColumn(0),
                InlineCommand::SavePosition,
            ]
        );

        renderer
            .draw(&tall)
            .expect("redraw after a resize succeeds");
        assert!(renderer.presentation.anchored);
        assert_eq!(renderer.presentation.owned_rows, 3);
    }

    #[test]
    fn a_burst_of_resize_events_re_establishes_the_region_once() {
        let mut events = ScriptedEvents::new([
            Ok(Event::Resize {
                columns: 10,
                rows: 5,
            }),
            Ok(Event::Resize {
                columns: 20,
                rows: 6,
            }),
            Ok(Event::Resize {
                columns: 30,
                rows: 7,
            }),
            Ok(enter()),
        ])
        .arriving_together(3);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        let outcome = form([TestField::new("field", "value")])
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
            .expect("the form submits");

        // Dragging a window edge emits a resize per intermediate size, and each
        // one loses the region. Acting on every one would re-establish the
        // region — and leave residue — as many times as the drag reported.
        assert!(matches!(outcome, FormOutcome::Submitted(_)));
        assert_eq!(renderer.resizes, [(30, 7)]);
    }

    #[test]
    fn coalescing_a_resize_burst_keeps_the_event_that_ended_it() {
        let mut events = ScriptedEvents::new([
            Ok(Event::Resize {
                columns: 10,
                rows: 5,
            }),
            Ok(enter()),
        ])
        .arriving_together(2);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = RecordingTerminal::interactive();

        // Draining the burst reads one event past its end. That event is what
        // the user typed, and it is held over rather than dropped.
        let outcome = form([TestField::new("field", "value")])
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
            .expect("the form submits");
        assert!(matches!(outcome, FormOutcome::Submitted(_)));
        assert_eq!(renderer.resizes, [(10, 5)]);
    }

    #[test]
    fn every_row_is_cleared_before_it_is_written_in_the_same_frame() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 4));
        let tall = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.help).with_kind(LineKind::Help),
            ],
            Some(ViewCursor { row: 1, column: 3 }),
        );
        let short = renderer_view(vec![view_line("only", &styles.question)], None);

        // A write that fails partway never reaches the fold, so it cannot raise
        // owned_rows itself. It is covered because the clear on its own row
        // already did — the invariant the recovery contract rests on. Dropping
        // a clear for a row believed to be empty or merely appended to would
        // fail here rather than silently leaving residue behind a failed write.
        for view in [&tall, &short, &tall] {
            let plan = draw_plan(&renderer, view);
            let mut state = RenderState::resuming(renderer.presentation.clone());
            for command in &plan.commands {
                if matches!(command, InlineCommand::Write(_)) {
                    assert!(
                        state.presentation.owned_rows > state.cursor_row,
                        "row {} is written while cleanup owns only {} rows",
                        state.cursor_row,
                        state.presentation.owned_rows
                    );
                }
                state = step(state, command);
            }
            renderer.draw(view).expect("renderer writes to a buffer");
        }
    }

    #[test]
    fn error_cleanup_clears_the_rows_a_partial_first_draw_touched() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let writer = PrefixThenFailWriter::new("first");
        let mut renderer = CrosstermRenderer::new(writer, (20, 4));
        let view = renderer_view(
            vec![
                view_line("first", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.error).with_kind(LineKind::Error),
            ],
            None,
        );

        assert!(renderer.draw(&view).is_err());
        // Three rows were scrolled into existence, but the write that failed
        // had reached only the first of them. The fold owns exactly that row:
        // its ClearLine succeeded, so the bytes the partial write left
        // there are covered, while the two rows below are still the blank ones
        // the line feeds scrolled in and there is nothing on them to erase.
        assert_eq!(renderer.presentation.reserved_rows, 3);
        assert_eq!(renderer.presentation.owned_rows, 1);
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Error, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Error)
            .expect("error cleanup succeeds after one draw failure");

        let output =
            String::from_utf8(renderer.writer.bytes).expect("renderer writes UTF-8 commands");
        assert!(output.contains('f'));
    }

    #[test]
    fn error_cleanup_after_partial_growth_owns_only_the_rows_the_frame_reached() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(PrefixThenFailWriter::new("growth"), (20, 4));
        renderer
            .draw(&renderer_view(
                vec![view_line("short", &styles.question)],
                None,
            ))
            .expect("initial draw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 1);

        let growth = renderer_view(
            vec![
                view_line("growth", &styles.question),
                view_line("second", &styles.option),
                view_line("third", &styles.error).with_kind(LineKind::Error),
            ],
            None,
        );
        assert!(renderer.draw(&growth).is_err());
        // The region grew from one row to three before the failing write, and
        // reserved_rows records that. What cleanup must erase is narrower: the
        // rows below the first were scrolled in blank and never written.
        assert_eq!(renderer.presentation.reserved_rows, 3);
        assert_eq!(renderer.presentation.owned_rows, 1);
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Error, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Error)
            .expect("growth cleanup succeeds after one draw failure");
    }

    #[test]
    fn inline_renderer_clears_stale_rows_and_clamps_cjk_cursor_in_narrow_viewports() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (4, 2));
        renderer
            .draw(&renderer_view(
                vec![
                    PromptLine::spans(vec![
                        ViewSpan::new("名前 ", &styles.question),
                        ViewSpan::new("あいうえ", &styles.answer),
                    ]),
                    view_line("validation message", &styles.error).with_kind(LineKind::Error),
                ],
                Some(ViewCursor { row: 0, column: 11 }),
            ))
            .expect("narrow draw succeeds");
        let narrow = renderer_view(
            vec![view_line("名前 あいうえ", &styles.answer)],
            Some(ViewCursor { row: 0, column: 11 }),
        );
        let layout = lay_out(renderer.columns, renderer.rows, &narrow);
        assert!(layout.rows.len() <= 2);
        // Bounding the cursor to the terminal box is the plan stage's job: it
        // is the stage the geometry is an input to, and the frame stage
        // chooses rows and has no horizontal concern at all.
        let plan = draw_plan(&renderer, &narrow);
        assert!(!plan.commands.iter().any(|command| matches!(
            command,
            InlineCommand::MoveRight(column) | InlineCommand::MoveToColumn(column)
                if *column >= renderer.columns
        )));

        renderer.resize(3, 1);
        renderer
            .draw(&renderer_view(
                vec![view_line("短い", &styles.question)],
                None,
            ))
            .expect("redraw succeeds");
        assert_eq!(renderer.presentation.owned_rows, 1);
        // A shrunk viewport leaves the region one row tall, so cancelling
        // erases exactly that row — never the screen.
        assert_eq!(
            inline_plan::plan_finish(RenderFinish::Cancelled, &renderer.presentation).commands,
            [
                InlineCommand::RestorePosition,
                InlineCommand::ClearLine,
                InlineCommand::RestorePosition,
                InlineCommand::RestorePosition,
            ]
        );

        renderer
            .finish(RenderFinish::Cancelled)
            .expect("cancel cleanup succeeds");
    }

    #[test]
    fn narrow_viewports_omit_wide_scalars_without_losing_cursor_bounds() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let cjk_line = view_line("あ", &styles.cursor);
        for columns in [0, 1] {
            let mut renderer = CrosstermRenderer::new(Vec::new(), (columns, 1));
            let view = renderer_view(
                vec![cjk_line.clone()],
                Some(ViewCursor { row: 0, column: 0 }),
            );
            let layout = lay_out(renderer.columns, renderer.rows, &view);
            assert_eq!(layout.rows.len(), 1);
            // A wide grapheme that cannot be shown whole leaves blank cells:
            // half of one is not something a terminal can draw.
            assert!(!layout.rows[0].text().contains('あ'));
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
        let styles = PromptStyles::resolve(&theme, &profile);
        for rows in 1..=4 {
            let renderer = CrosstermRenderer::new(Vec::new(), (40, rows));
            let view = renderer_view(
                vec![
                    view_line("previous question", &styles.muted),
                    view_line("previous answer", &styles.answer),
                    view_line("", &styles.body),
                    active_line(view_line("┃ current question", &styles.accent)),
                    active_line(PromptLine::spans(vec![
                        ViewSpan::new("┃ ", &styles.accent),
                        ViewSpan::new("› current answer", &styles.cursor),
                    ])),
                    view_line("", &styles.body),
                    view_line("enter continue", &styles.help).with_kind(LineKind::Help),
                ],
                Some(ViewCursor { row: 4, column: 18 }),
            );

            let layout = lay_out(renderer.columns, renderer.rows, &view);

            assert_eq!(layout.rows.len(), usize::from(rows));
            assert!(layout.cursor.is_some_and(|cursor| cursor.row < rows));
            // Monochrome collapses every role onto one style, so the row a
            // frame decision selected is identifiable only by its content.
            assert!(
                layout
                    .rows
                    .iter()
                    .any(|line| line.text().contains("current answer")),
                "active input missing at {rows} rows"
            );
            assert_eq!(
                layout
                    .rows
                    .iter()
                    .any(|line| line.text().contains("enter continue")),
                rows >= 3,
                "unexpected help visibility at {rows} rows"
            );
        }
    }

    #[test]
    fn one_row_viewports_show_the_actionable_choice() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let renderer = CrosstermRenderer::new(Vec::new(), (40, 1));
        let view = renderer_view(
            vec![
                active_line(view_line("┃ choose a language", &styles.accent)),
                active_line(
                    view_line("┃   Japanese", &styles.option)
                        .with_kind(LineKind::Choice { focused: false }),
                ),
                active_line(
                    PromptLine::spans(vec![
                        ViewSpan::new("┃ ", &styles.accent),
                        ViewSpan::new("› English", &styles.option_selected),
                    ])
                    .with_kind(LineKind::Choice { focused: true }),
                ),
                view_line("", &styles.body),
                view_line("↑/↓ select", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );

        let layout = lay_out(renderer.columns, renderer.rows, &view);

        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.rows[0].text(), "┃ › English");

        let confirm_view = renderer_view(
            vec![
                active_line(view_line("┃ continue?", &styles.accent)),
                active_line(view_line("┃", &styles.accent)),
                active_line(
                    PromptLine::spans(vec![
                        ViewSpan::new("┃ ", &styles.accent),
                        ViewSpan::new("  Yes  ", &styles.button_focused),
                        ViewSpan::new("   No  ", &styles.button),
                    ])
                    .with_kind(LineKind::Choice { focused: true }),
                ),
                view_line("y/n answer", &styles.help).with_kind(LineKind::Help),
            ],
            None,
        );

        let confirm_layout = lay_out(renderer.columns, renderer.rows, &confirm_view);

        assert_eq!(confirm_layout.rows.len(), 1);
        assert_eq!(confirm_layout.rows[0].text(), "┃   Yes     No  ");
    }

    #[test]
    fn inline_renderer_applies_terminal_profile_before_writing_styles() {
        let theme = test_theme();
        let profile = TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled);
        let styles = PromptStyles::resolve(&theme, &profile);
        let mut renderer = CrosstermRenderer::new(Vec::new(), (20, 2));
        renderer
            .draw(&renderer_view(
                vec![view_line("plain", &styles.question)],
                None,
            ))
            .expect("draw succeeds");
        let output = String::from_utf8(renderer.writer).expect("renderer writes UTF-8 commands");
        assert!(!output.contains("\x1b[1m"));
        assert!(output.contains("plain"));
    }

    #[test]
    fn every_pair_of_roles_compares_equal_exactly_when_it_paints_the_same() {
        // The property the whole resolved-style view rests on: a row is the
        // unit the plan stage compares to decide whether to redraw, so two
        // spans must carry equal values whenever, and only whenever, they
        // reach the terminal as the same bytes.
        let theme = test_theme();
        for color in [
            ColorProfile::TrueColor,
            ColorProfile::Ansi256,
            ColorProfile::Ansi16,
            ColorProfile::Monochrome,
        ] {
            for ansi in [AnsiPolicy::Enabled, AnsiPolicy::Disabled] {
                let profile = TerminalProfile::new(color, ansi);
                let styles = PromptStyles::resolve(&theme, &profile);
                let roles = [
                    ("body", &styles.body),
                    ("muted", &styles.muted),
                    ("accent", &styles.accent),
                    ("question", &styles.question),
                    ("answer", &styles.answer),
                    ("placeholder", &styles.placeholder),
                    ("cursor", &styles.cursor),
                    ("option", &styles.option),
                    ("option_selected", &styles.option_selected),
                    ("button", &styles.button),
                    ("button_focused", &styles.button_focused),
                    ("help", &styles.help),
                    ("error", &styles.error),
                ];
                for (left_name, left) in roles {
                    for (right_name, right) in roles {
                        assert_eq!(
                            left == right,
                            left.paint("x") == right.paint("x"),
                            "{left_name} vs {right_name} under {color:?}/{ansi:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_profile_that_erases_a_difference_makes_the_runs_equal_and_merges_them() {
        let theme = test_theme();
        let colored = PromptStyles::resolve(
            &theme,
            &TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled),
        );
        let monochrome = PromptStyles::resolve(
            &theme,
            &TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Disabled),
        );

        // Two roles the theme gives different appearances.
        assert_ne!(colored.question, colored.answer);
        // The profile erases that distinction, so the values must be equal:
        // otherwise rows that render identically compare unequal and redraw
        // every frame.
        assert_eq!(monochrome.question, monochrome.answer);

        let line = PromptLine::spans(vec![
            ViewSpan::new("ab", &monochrome.question),
            ViewSpan::new("cd", &monochrome.answer),
        ]);
        let framed = lay_out(10, 1, &renderer_view(vec![line], None));
        assert_eq!(framed.rows[0].runs.len(), 1);
        assert_eq!(framed.rows[0].text(), "abcd");
    }

    #[test]
    fn compact_input_keeps_question_with_control_until_only_control_fits() {
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(Input::new(FieldKey::new("name"), "Name", "value").expect("input"))
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let state = FormState::Running { group: 0, field: 0 };
        let view = form.view(&state, &test_styles(), 40);

        let two = lay_out(40, 2, &view);
        let two_text = two
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();
        assert!(two_text.iter().any(|row| row.contains("Name")));
        assert!(two_text.iter().any(|row| row.contains("› value")));
        assert!(!two_text.iter().any(|row| row.contains("continue")));

        let one = lay_out(40, 1, &view);
        assert_eq!(one.rows.len(), 1);
        assert!(one.rows[0].text().contains("› value"));
        assert_eq!(one.cursor, Some(ViewCursor { row: 0, column: 9 }));
    }

    #[test]
    fn compact_input_prioritizes_error_then_restores_question() {
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Input::new(FieldKey::new("name"), "Name", "value")
                            .expect("input")
                            .validate(Box::new(|_| {
                                Err(crate::ValidationError::new("Not acceptable"))
                            })),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        assert_eq!(form.reduce(&mut state, enter()), ReducerResult::Running);
        let view = form.view(&state, &test_styles(), 40);

        let two = lay_out(40, 2, &view);
        let two_text = two
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();
        assert!(two_text.iter().any(|row| row.contains("Not acceptable")));
        assert!(two_text.iter().any(|row| row.contains("› value")));
        assert!(!two_text.iter().any(|row| row.contains("Name")));

        let three = lay_out(40, 3, &view);
        let three_text = three
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();
        assert!(three_text.iter().any(|row| row.contains("Name")));
        assert!(three_text.iter().any(|row| row.contains("Not acceptable")));
        assert!(three_text.iter().any(|row| row.contains("› value")));
    }

    #[test]
    fn compact_group_drops_complete_inactive_fields_before_active_essentials() {
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(Input::new(FieldKey::new("first"), "First", "one").expect("input"))
                    .field(Input::new(FieldKey::new("second"), "Second", "two").expect("input"))
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let state = FormState::Running { group: 0, field: 1 };
        let compact = lay_out(40, 2, &form.view(&state, &test_styles(), 40));
        let text = compact
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();

        assert!(text.iter().any(|row| row.contains("Second")));
        assert!(text.iter().any(|row| row.contains("› two")));
        assert!(!text.iter().any(|row| row.contains("First")));
        assert!(!text.iter().any(|row| row.contains("› one")));
    }

    #[test]
    fn compact_select_retains_the_focused_choice_and_nearby_control_rows() {
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Select::new(
                            FieldKey::new("choice"),
                            "Choose",
                            vec![
                                SelectOption::new("One", 1),
                                SelectOption::new("Two", 2),
                                SelectOption::new("Three", 3),
                            ],
                        )
                        .expect("select")
                        .visible_rows(3),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let state = FormState::Running { group: 0, field: 0 };
        let compact = lay_out(40, 2, &form.view(&state, &test_styles(), 40));
        let text = compact
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();

        assert!(text.iter().any(|row| row.contains("› One")));
        assert!(text.iter().any(|row| row.contains("Choose")));
        assert_eq!(text.len(), 2);
    }

    #[test]
    fn compact_wrapped_question_uses_spare_rows_for_description_and_help() {
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Input::new(
                            FieldKey::new("name"),
                            "A question long enough that it wraps across several terminal rows",
                            "value",
                        )
                        .expect("input")
                        .description("Supporting detail")
                        .help("enter continue"),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let state = FormState::Running { group: 0, field: 0 };
        let compact = lay_out(20, 3, &form.view(&state, &test_styles(), 20));
        let text = compact
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();

        assert!(text.iter().any(|row| row.contains("Supporting detail")));
        assert!(text.iter().any(|row| row.contains("› value")));
        assert!(text.iter().any(|row| row.contains("enter continue")));
        assert!(!text.iter().any(|row| row.contains("A question")));
    }

    #[test]
    fn compact_confirm_keeps_description_ahead_of_its_spacer() {
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Confirm::new(FieldKey::new("confirm"), "Continue?", Some(true))
                            .expect("confirm")
                            .description("This cannot be undone."),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let state = FormState::Running { group: 0, field: 0 };
        let compact = lay_out(40, 3, &form.view(&state, &test_styles(), 40));
        let text = compact
            .rows
            .iter()
            .map(frame::FramedRow::text)
            .collect::<Vec<_>>();

        assert!(text.iter().any(|row| row.contains("Continue?")));
        assert!(
            text.iter()
                .any(|row| row.contains("This cannot be undone."))
        );
        assert!(text.iter().any(|row| row.contains("Yes")));
        assert!(text.iter().all(|row| !row.trim().is_empty()));
    }

    #[test]
    fn compact_confirm_keeps_the_selected_button_row_when_buttons_wrap() {
        let mut form = Form::builder()
            .group(
                Group::builder()
                    .field(
                        Confirm::new(FieldKey::new("confirm"), "Q", Some(true)).expect("confirm"),
                    )
                    .build()
                    .expect("group"),
            )
            .build()
            .expect("form");
        let mut state = FormState::Running { group: 0, field: 0 };
        assert_eq!(
            form.reduce(
                &mut state,
                Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    modifiers: KeyModifiers::default(),
                }),
            ),
            ReducerResult::Running
        );
        let styles = test_styles();
        let compact = lay_out(12, 1, &form.view(&state, &styles, 12));

        assert_eq!(compact.rows.len(), 1);
        assert!(
            compact.rows[0]
                .runs()
                .iter()
                .any(|run| run.style == styles.button_focused && run.text.contains("No"))
        );
    }

    fn assert_io_operation(result: Result<FormOutcome, RunError>, expected: IoOperation) {
        match result {
            Err(RunError::Io { operation, .. }) => assert_eq!(operation, expected),
            _ => panic!("expected I/O error"),
        }
    }
}
