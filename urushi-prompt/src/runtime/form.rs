//! Form construction, navigation, and blocking execution orchestration.

use std::{any::Any, collections::HashMap, fmt, marker::PhantomData};

use urushi::{ColorLevel, RenderSettings, Theme};

use super::{
    TextSpan,
    crossterm::{CrosstermEventSource, CrosstermRenderer, CrosstermTerminalControl},
    error::{FormBuildError, GroupBuildError, IoOperation, RunError},
    field::{self, Field, FieldAction, FieldEntry},
    terminal::{
        Event, EventSource, KeyCode, KeyEvent, KeyModifiers, RenderFinish, Renderer,
        TerminalControl, TerminalSession,
    },
    view::{GUTTER, LineKind, PromptLine, PromptStyles, PromptView, gutter_view},
};

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

/// What an inline prompt does when its terminal viewport is resized.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum InlineResizePolicy {
    /// Stop the form and return [`RunError::Resized`] without erasing an
    /// unlocatable prompt region.
    #[default]
    ReturnError,
    /// Clear the visible primary-buffer viewport and redraw the current form.
    ///
    /// This does not enter the alternate screen or clear scrollback, but it
    /// does erase every other visible cell in the viewport.
    ClearViewportAndRedraw,
}

impl PromptStart {
    pub(crate) const fn column(self) -> u16 {
        match self {
            Self::NewLine | Self::CurrentLine => 0,
            Self::CurrentPosition { column } => column,
        }
    }

    pub(super) fn within(self, terminal_columns: u16) -> Self {
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
    inline_resize_policy: InlineResizePolicy,
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

    /// Chooses how this inline prompt responds to a terminal resize.
    ///
    /// The default is [`InlineResizePolicy::ReturnError`]. Clearing and
    /// redrawing must be selected explicitly because it erases the complete
    /// visible primary-buffer viewport, including content outside the prompt.
    #[must_use]
    pub fn inline_resize_policy(mut self, policy: InlineResizePolicy) -> Self {
        self.inline_resize_policy = policy;
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
            inline_resize_policy: self.inline_resize_policy,
        })
    }
}

/// A blocking, ordered collection of prompt groups.
pub struct Form {
    groups: Vec<Group>,
    pub(super) start: PromptStart,
    width: Option<u16>,
    inline_resize_policy: InlineResizePolicy,
}

impl Form {
    /// Starts building a form.
    pub fn builder() -> FormBuilder {
        FormBuilder {
            groups: Vec::new(),
            start: PromptStart::default(),
            width: None,
            inline_resize_policy: InlineResizePolicy::default(),
        }
    }

    /// Runs this form using the process terminal input and standard error.
    ///
    /// The supplied theme and detected terminal capabilities are resolved once into the
    /// prompt's styles, and fields build their view from those resolved
    /// values; no component role reaches the renderer.
    pub fn run(self, theme: &Theme) -> Result<FormOutcome, RunError> {
        let stderr = std::io::stderr();
        let (size, mut settings) =
            match urushi_terminal::detect(&stderr).map_err(|source| RunError::Io {
                operation: IoOperation::EnterTerminal,
                source,
                cleanup: None,
            })? {
                urushi_terminal::TerminalDetection::Terminal(info) => (
                    (
                        info.size().columns().try_into().unwrap_or(u16::MAX),
                        info.size().rows().try_into().unwrap_or(u16::MAX),
                    ),
                    RenderSettings::from(info.capabilities()),
                ),
                urushi_terminal::TerminalDetection::NonTerminal => {
                    ((80, 24), RenderSettings::default())
                }
            };
        settings = apply_no_color(
            settings,
            std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
        );
        let mut events = CrosstermEventSource;
        let mut renderer = CrosstermRenderer::stderr(size);
        let mut terminal = CrosstermTerminalControl;
        let styles = PromptStyles::resolve(theme, &settings);
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
                // Act once on the latest size already waiting: ReturnError has
                // one terminal exit, while ClearViewportAndRedraw has one
                // destructive clear and one replacement frame.
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
                match self.inline_resize_policy {
                    InlineResizePolicy::ReturnError => {
                        let cleanup = session.cleanup(RenderFinish::Error);
                        return Err(RunError::Resized { cleanup });
                    }
                    InlineResizePolicy::ClearViewportAndRedraw => {
                        if let Err(source) = session.renderer.clear_viewport() {
                            return Err(session.fail(IoOperation::Render, source));
                        }
                        continue;
                    }
                }
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

    pub(super) fn drawing_width(&self, terminal_columns: u16) -> u16 {
        let start = self.start.within(terminal_columns);
        let available = terminal_columns.saturating_sub(start.column()).max(1);
        self.width.unwrap_or(available).max(1).min(available)
    }

    pub(super) fn reduce(&mut self, state: &mut FormState, event: Event) -> ReducerResult {
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

    pub(super) fn view(
        &self,
        state: &FormState,
        styles: &PromptStyles,
        columns: u16,
    ) -> PromptView {
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
            lines.push(PromptLine::spans(vec![TextSpan::new(
                title.clone(),
                styles.question.clone(),
            )]));
        }
        if let Some(description) = &self.groups[group].description {
            lines.push(PromptLine::spans(vec![TextSpan::new(
                description.clone(),
                styles.muted.clone(),
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

fn apply_no_color(settings: RenderSettings, no_color: bool) -> RenderSettings {
    if no_color {
        settings.color_level(ColorLevel::None)
    } else {
        settings
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
            .push(field::private::Sealed::into_entry(Box::new(field)));
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormState {
    Running { group: usize, field: usize },
    Submitted,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReducerResult {
    Running,
    Submitted,
    Cancelled,
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use crate::runtime::{
        IoOperation, KeyCode, KeyEvent, KeyModifiers, terminal::tests::*, test_styles,
    };
    use crate::{
        Confirm, ConfirmAnswer, ConfirmSource, FieldConfigError, Input, Select, SelectOption,
    };

    #[test]
    fn no_color_only_narrows_the_prompt_color_level() {
        let settings = RenderSettings::default()
            .color_level(ColorLevel::TrueColor)
            .hyperlinks(true);

        let narrowed = apply_no_color(settings, true);

        assert_eq!(narrowed.get_color_level(), ColorLevel::None);
        assert!(narrowed.get_hyperlinks());
        assert_eq!(apply_no_color(settings, false), settings);
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
        assert_eq!(
            default.inline_resize_policy,
            InlineResizePolicy::ReturnError
        );
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
            .inline_resize_policy(InlineResizePolicy::ClearViewportAndRedraw)
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
        assert_eq!(renderer.viewport_clears, 1);
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

        let resized = RunError::Resized { cleanup: None };
        assert_eq!(resized.to_string(), "terminal resized during inline prompt");
        assert!(std::error::Error::source(&resized).is_none());
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
    fn the_default_resize_policy_returns_an_error_after_one_coalesced_resize() {
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

        let outcome = form([TestField::new("field", "value")]).run_with(
            &mut events,
            &mut renderer,
            &mut terminal,
            &test_styles(),
        );

        assert!(matches!(outcome, Err(RunError::Resized { cleanup: None })));
        assert_eq!(renderer.resizes, [(30, 7)]);
        assert_eq!(renderer.viewport_clears, 0);
        assert_eq!(renderer.views.len(), 1);
        assert_eq!(renderer.finishes, [RenderFinish::Error]);
    }

    #[test]
    fn clear_viewport_policy_redraws_then_keeps_the_event_that_ended_the_burst() {
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
        let outcome = form_with_resize_policy(
            [TestField::new("field", "value")],
            InlineResizePolicy::ClearViewportAndRedraw,
        )
        .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
        .expect("the form submits");
        assert!(matches!(outcome, FormOutcome::Submitted(_)));
        assert_eq!(renderer.resizes, [(10, 5)]);
        assert_eq!(renderer.viewport_clears, 1);
        assert_eq!(renderer.views.len(), 2);
    }

    #[test]
    fn resize_error_retains_a_cleanup_failure() {
        let mut events = ScriptedEvents::new([Ok(Event::Resize {
            columns: 10,
            rows: 5,
        })]);
        let mut renderer = RecordingRenderer {
            fail_finish: true,
            ..RecordingRenderer::default()
        };
        let mut terminal = RecordingTerminal::interactive();

        let result = form([TestField::new("field", "value")]).run_with(
            &mut events,
            &mut renderer,
            &mut terminal,
            &test_styles(),
        );

        assert!(matches!(
            result,
            Err(RunError::Resized {
                cleanup: Some(ref error)
            }) if error.to_string() == "finish failed"
        ));
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
    fn a_failed_viewport_clear_is_a_render_error_and_runs_cleanup() {
        let mut events = ScriptedEvents::new([Ok(Event::Resize {
            columns: 10,
            rows: 5,
        })]);
        let mut renderer = RecordingRenderer {
            fail_clear_viewport: true,
            ..RecordingRenderer::default()
        };
        let mut terminal = RecordingTerminal::interactive();

        let result = form_with_resize_policy(
            [TestField::new("field", "value")],
            InlineResizePolicy::ClearViewportAndRedraw,
        )
        .run_with(&mut events, &mut renderer, &mut terminal, &test_styles());

        assert_io_operation(result, IoOperation::Render);
        assert_eq!(renderer.viewport_clears, 1);
        assert_eq!(renderer.finishes, [RenderFinish::Error]);
    }
}
