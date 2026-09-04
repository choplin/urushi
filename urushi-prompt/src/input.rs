use std::any::Any;

use unicode_segmentation::UnicodeSegmentation;
use urushi::{PrintableText, VerticalAlign, View};

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, KeyCode, LineKind, PromptLine, PromptStyles,
        PromptView, RuntimeField, ViewCursor, ViewSpan, clipped_line_view, fixed_view, line_view,
        window_spans,
    },
};

/// An error reported by a synchronous input validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// The message shown beneath the input.
    pub message: String,
}

impl ValidationError {
    /// Creates a validation error with a displayable message.
    /// `message` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

/// A synchronous validation rule for a prompt value.
pub type Validator<T> = Box<dyn Fn(&T) -> Result<(), ValidationError> + Send + Sync + 'static>;

/// A one-line text input field.
pub struct Input {
    key: FieldKey<String>,
    question: String,
    description: Option<String>,
    value: String,
    placeholder: Option<String>,
    help: String,
    required: bool,
    required_message: String,
    validators: Vec<Validator<String>>,
    cursor: usize,
    validation_error: Option<String>,
}

impl Input {
    /// Creates an input with a typed key, prompt question, and initial value.
    pub fn new(
        key: FieldKey<String>,
        question: impl Into<String>,
        initial_value: impl Into<String>,
    ) -> Result<Self, FieldConfigError> {
        if key.name().is_empty() {
            return Err(FieldConfigError::EmptyName);
        }

        let value = initial_value.into();
        let cursor = value.graphemes(true).count();
        Ok(Self {
            key,
            question: question.into(),
            description: None,
            value,
            placeholder: None,
            help: "enter continue • shift+tab back • esc cancel".to_owned(),
            required: false,
            required_message: "This field is required.".to_owned(),
            validators: Vec::new(),
            cursor,
            validation_error: None,
        })
    }

    /// Sets text shown when the current value is empty.
    /// `placeholder` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Sets supporting text shown below the question.
    /// `description` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets the navigation hint shown beneath the field.
    /// `help` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Requires a non-empty value before the form can advance.
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    /// Sets the message shown when a required input is empty.
    #[must_use]
    pub fn required_message(mut self, message: impl Into<String>) -> Self {
        self.required_message = message.into();
        self
    }

    /// Appends a synchronous validator, evaluated after the required check.
    #[must_use]
    pub fn validate(mut self, validator: Validator<String>) -> Self {
        self.validators.push(validator);
        self
    }

    /// Returns the prompt question.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// Returns the current input value.
    pub fn value(&self) -> &str {
        &self.value
    }

    fn edit(&mut self) {
        self.validation_error = None;
    }

    fn byte_index(&self, grapheme_index: usize) -> usize {
        self.value
            .grapheme_indices(true)
            .nth(grapheme_index)
            .map_or(self.value.len(), |(byte_index, _)| byte_index)
    }

    fn delete_grapheme_at(&mut self, grapheme_index: usize) {
        let start = self.byte_index(grapheme_index);
        let end = self.byte_index(grapheme_index + 1);
        if start != end {
            self.value.replace_range(start..end, "");
        }
    }

    fn insert_text(&mut self, text: &str) {
        let text = text
            .replace("\r\n", "\n")
            .chars()
            .filter_map(|character| match character {
                '\r' | '\n' | '\t' => Some(' '),
                character if character.is_control() => None,
                character => Some(character),
            })
            .collect::<String>();
        let byte_index = self.byte_index(self.cursor);
        self.value.insert_str(byte_index, &text);
        self.cursor += text.graphemes(true).count();
        self.edit();
    }

    fn submit(&mut self) -> FieldAction {
        if self.required && self.value.is_empty() {
            self.validation_error = Some(self.required_message.clone());
            return FieldAction::Stay;
        }

        for validator in &self.validators {
            if let Err(error) = validator(&self.value) {
                self.validation_error = Some(error.message);
                return FieldAction::Stay;
            }
        }

        self.validation_error = None;
        FieldAction::Accept
    }

    fn answer_spans(&self, styles: &PromptStyles, focused: bool) -> Vec<ViewSpan> {
        if self.value.is_empty() {
            // The blank the cursor sits on exists only to be highlighted; an
            // unfocused field has no cursor and would draw a stray space.
            let mut spans = if focused {
                vec![ViewSpan::new(" ", &styles.cursor)]
            } else {
                Vec::new()
            };
            if let Some(placeholder) = &self.placeholder {
                spans.push(ViewSpan::new(placeholder.clone(), &styles.placeholder));
            }
            return spans;
        }

        let cursor_byte = self.byte_index(self.cursor);
        let mut spans = Vec::new();
        if cursor_byte > 0 {
            spans.push(ViewSpan::new(
                self.value[..cursor_byte].to_owned(),
                &styles.answer,
            ));
        }
        if let Some(grapheme) = self.value[cursor_byte..].graphemes(true).next() {
            // An unfocused field has no cursor, so the character under it is
            // drawn as ordinary answer text.
            let style = if focused {
                &styles.cursor
            } else {
                &styles.answer
            };
            spans.push(ViewSpan::new(grapheme, style));
            let after_cursor = cursor_byte + grapheme.len();
            if after_cursor < self.value.len() {
                spans.push(ViewSpan::new(
                    self.value[after_cursor..].to_owned(),
                    &styles.answer,
                ));
            }
        } else if focused {
            spans.push(ViewSpan::new(" ", &styles.cursor));
        }
        spans
    }
}

impl runtime::private::Sealed for Input {
    fn into_entry(self: Box<Self>) -> FieldEntry {
        FieldEntry::new(self.key.name().to_owned(), self)
    }
}

impl RuntimeField for Input {
    fn event(&mut self, event: Event) -> FieldAction {
        if let Event::Paste(text) = event {
            self.insert_text(&text);
            return FieldAction::Stay;
        }
        let Event::Key(key) = event else {
            return FieldAction::Stay;
        };

        match (key.code, key.modifiers.control, key.modifiers.alt) {
            (KeyCode::Escape, _, _) => FieldAction::Cancel,
            (KeyCode::Char('a'), true, _) | (KeyCode::Home, _, _) => {
                self.cursor = 0;
                FieldAction::Stay
            }
            (KeyCode::Char('e'), true, _) | (KeyCode::End, _, _) => {
                self.cursor = self.value.graphemes(true).count();
                FieldAction::Stay
            }
            (KeyCode::Char('b'), true, _) | (KeyCode::Left, _, _) => {
                self.cursor = self.cursor.saturating_sub(1);
                FieldAction::Stay
            }
            (KeyCode::Char('f'), true, _) | (KeyCode::Right, _, _) => {
                self.cursor = (self.cursor + 1).min(self.value.graphemes(true).count());
                FieldAction::Stay
            }
            (KeyCode::Char('u'), true, _) => {
                let end = self.byte_index(self.cursor);
                self.value.replace_range(..end, "");
                self.cursor = 0;
                self.edit();
                FieldAction::Stay
            }
            (KeyCode::Char('k'), true, _) => {
                let start = self.byte_index(self.cursor);
                self.value.truncate(start);
                self.edit();
                FieldAction::Stay
            }
            (KeyCode::Backspace, _, _) if self.cursor > 0 => {
                self.delete_grapheme_at(self.cursor - 1);
                self.cursor -= 1;
                self.edit();
                FieldAction::Stay
            }
            (KeyCode::Delete, _, _) if self.cursor < self.value.graphemes(true).count() => {
                self.delete_grapheme_at(self.cursor);
                self.edit();
                FieldAction::Stay
            }
            (KeyCode::Enter | KeyCode::Tab, _, _) => self.submit(),
            (KeyCode::Char(character), false, false) => {
                self.insert_text(&character.to_string());
                FieldAction::Stay
            }
            _ => FieldAction::Stay,
        }
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        Box::new(std::mem::take(&mut self.value))
    }

    fn view(&self, styles: &PromptStyles, focused: bool, width: usize) -> PromptView {
        // The prompt marker keeps its cells whatever the value does, so the
        // window the value scrolls in is the field's width less the marker.
        let answer_start = 2_usize;
        let cursor_prefix = &self.value[..self.byte_index(self.cursor)];
        let cursor_column = PrintableText::new(cursor_prefix).width();
        let answer_spans = self.answer_spans(styles, focused);
        // A value wider than the terminal is windowed here, before a `View`
        // exists: choosing which cells of an already-composed run are visible
        // is a text-layer operation, and doing it here is what lets the view
        // be resolved against the width the terminal really has.
        let (answer_spans, cursor_column) = if focused {
            window_spans(
                answer_spans,
                cursor_column,
                width.saturating_sub(answer_start),
            )
        } else {
            (answer_spans, cursor_column)
        };
        let answer = PromptLine::new(View::row(
            VerticalAlign::Top,
            [
                fixed_view(answer_start, vec![ViewSpan::new("› ", &styles.answer)]),
                line_view(answer_spans),
            ],
        ));
        let mut lines = vec![PromptLine::spans(vec![ViewSpan::new(
            self.question.clone(),
            styles.question(focused),
        )])];
        if let Some(description) = &self.description {
            lines.push(PromptLine::spans(vec![ViewSpan::new(
                description.clone(),
                &styles.muted,
            )]));
        }
        let answer_row = lines.len();
        lines.push(answer);
        if let Some(message) = &self.validation_error {
            lines.push(
                PromptLine::new(View::row(
                    VerticalAlign::Top,
                    [
                        fixed_view(2, vec![ViewSpan::new("! ", &styles.error)]),
                        View::text(message.clone(), styles.error.clone()),
                    ],
                ))
                .with_kind(LineKind::Error),
            );
        }
        lines.push(
            PromptLine::new(clipped_line_view(vec![ViewSpan::new(
                self.help.clone(),
                &styles.help,
            )]))
            .with_kind(LineKind::Help),
        );

        PromptView {
            lines,
            cursor: focused.then(|| ViewCursor {
                row: answer_row.min(usize::from(u16::MAX)) as u16,
                column: answer_start
                    .saturating_add(cursor_column)
                    .min(usize::from(u16::MAX)) as u16,
            }),
        }
    }

    fn validation_error(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use super::*;
    use crate::{
        Form, FormOutcome, Group,
        runtime::{EventSource, RenderFinish, Renderer, TerminalControl, test_styles},
    };

    fn key(code: KeyCode) -> Event {
        Event::Key(runtime::KeyEvent {
            code,
            modifiers: runtime::KeyModifiers::default(),
        })
    }

    #[test]
    fn input_rejects_an_empty_field_name() {
        assert!(matches!(
            Input::new(FieldKey::new(""), "Name", ""),
            Err(FieldConfigError::EmptyName)
        ));
    }

    #[test]
    fn edits_follow_unicode_grapheme_boundaries() {
        let mut input = Input::new(FieldKey::new("name"), "Name", "aあb").expect("input is valid");

        assert_eq!(input.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(input.event(key(KeyCode::Char('い'))), FieldAction::Stay);
        assert_eq!(input.value(), "aあいb");
        assert_eq!(input.event(key(KeyCode::Backspace)), FieldAction::Stay);
        assert_eq!(input.value(), "aあb");
        assert_eq!(input.event(key(KeyCode::Delete)), FieldAction::Stay);
        assert_eq!(input.value(), "aあ");

        let mut combined =
            Input::new(FieldKey::new("combined"), "Name", "e\u{301}x").expect("input is valid");
        assert_eq!(combined.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(combined.event(key(KeyCode::Backspace)), FieldAction::Stay);
        assert_eq!(combined.value(), "x");
    }

    #[test]
    fn paste_inserts_at_the_cursor_and_flattens_line_breaks() {
        let mut input = Input::new(FieldKey::new("name"), "Name", "ab").expect("input is valid");
        assert_eq!(input.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(
            input.event(Event::Paste("あ\r\nい\tう".to_owned())),
            FieldAction::Stay
        );
        assert_eq!(input.value(), "aあ い うb");
    }

    #[test]
    fn view_uses_resolved_styles_and_cjk_display_columns() {
        let mut input = Input::new(FieldKey::new("name"), "名前", "あ")
            .expect("input is valid")
            .placeholder("入力してください");
        assert_eq!(input.event(key(KeyCode::Home)), FieldAction::Stay);

        let styles = test_styles();
        let view = input.view(&styles, true, 80);
        assert_eq!(view.lines[0].runs()[0].text, "名前");
        assert_eq!(view.lines[0].runs()[0].style, styles.question);
        assert_eq!(view.lines[1].runs()[0].text, "› ");
        assert_eq!(view.lines[1].runs()[1].style, styles.cursor);
        assert_eq!(view.cursor, Some(ViewCursor { row: 1, column: 2 }));
        assert_eq!(view.lines[2].kind, LineKind::Help);
        assert_eq!(view.lines[2].runs()[0].style, styles.help);

        let empty = Input::new(FieldKey::new("empty"), "Name", "")
            .expect("input is valid")
            .placeholder("Example");
        let empty_view = empty.view(&styles, true, 80);
        assert_eq!(empty_view.lines[1].runs()[1].style, styles.cursor);
        assert_eq!(empty_view.lines[1].runs()[2].style, styles.placeholder);

        assert_eq!(input.event(key(KeyCode::End)), FieldAction::Stay);
        let trailing = input.view(&styles, true, 80).lines[1].runs();
        let trailing = trailing.last().expect("the answer row has runs");
        assert_eq!(trailing.text, " ");
        assert_eq!(trailing.style, styles.cursor);
    }

    #[test]
    fn an_unfocused_input_recedes_and_drops_the_cursor() {
        let styles = test_styles();
        let input = Input::new(FieldKey::new("name"), "Name", "value").expect("input is valid");

        let unfocused = input.view(&styles, false, 80);
        assert_eq!(unfocused.lines[0].runs()[0].style, styles.muted);
        assert!(
            unfocused.lines[1]
                .runs()
                .iter()
                .all(|span| span.style != styles.cursor)
        );
        assert_eq!(
            unfocused.lines[1]
                .runs()
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>(),
            "› value"
        );

        let empty = Input::new(FieldKey::new("empty"), "Name", "").expect("input is valid");
        assert!(empty.view(&styles, false, 80).lines[1].runs()[1..].is_empty());
    }

    #[test]
    fn an_unfocused_input_keeps_a_space_the_cursor_was_resting_on() {
        // Losing focus drops the cursor, not a character of the value. The
        // blank that is dropped is the synthetic one drawn past the end of the
        // value, which is why it is decided by position rather than by text.
        let styles = test_styles();
        let mut input = Input::new(FieldKey::new("name"), "Name", "John Doe")
            .expect("input is valid")
            .placeholder("");
        assert_eq!(input.event(key(KeyCode::Home)), FieldAction::Stay);
        for _ in 0..4 {
            assert_eq!(input.event(key(KeyCode::Right)), FieldAction::Stay);
        }

        let unfocused = input.view(&styles, false, 80);
        assert_eq!(
            unfocused.lines[1]
                .runs()
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>(),
            "› John Doe"
        );
    }

    #[test]
    fn validators_run_in_registration_order_after_required() {
        let second_runs = Arc::new(AtomicUsize::new(0));
        let second_runs_for_validator = Arc::clone(&second_runs);
        let mut input = Input::new(FieldKey::new("name"), "Name", "value")
            .expect("input is valid")
            .validate(Box::new(|_| Err(ValidationError::new("first failure"))))
            .validate(Box::new(move |_| {
                second_runs_for_validator.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }));

        assert_eq!(input.event(key(KeyCode::Enter)), FieldAction::Stay);
        assert_eq!(input.validation_error(), Some("first failure"));
        assert_eq!(second_runs.load(Ordering::SeqCst), 0);

        let mut required = Input::new(FieldKey::new("required"), "Required", "")
            .expect("input is valid")
            .required()
            .required_message("A value is needed.")
            .validate(Box::new(|_| Err(ValidationError::new("validator failure"))));
        assert_eq!(required.event(key(KeyCode::Tab)), FieldAction::Stay);
        assert_eq!(required.validation_error(), Some("A value is needed."));
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
        views: Vec<PromptView>,
    }

    impl Renderer for RecordingRenderer {
        fn draw(
            &mut self,
            view: &PromptView,
            _start: crate::PromptStart,
            _drawing_columns: u16,
        ) -> io::Result<()> {
            self.views.push(view.clone());
            Ok(())
        }

        fn finish(&mut self, _outcome: RenderFinish) -> io::Result<()> {
            Ok(())
        }
    }

    struct InteractiveTerminal;

    impl TerminalControl for InteractiveTerminal {
        fn is_interactive(&self) -> bool {
            true
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn show_cursor(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn invalid_edits_clear_the_error_and_then_advance_with_typed_values() {
        let first = Input::new(FieldKey::new("first"), "First", "")
            .expect("input is valid")
            .required();
        let second = Input::new(FieldKey::new("second"), "Second", "done").expect("input is valid");
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(first)
                    .field(second)
                    .build()
                    .expect("group is valid"),
            )
            .build()
            .expect("form is valid");
        let mut events = ScriptedEvents::new([
            Ok(key(KeyCode::Enter)),
            Ok(key(KeyCode::Char('値'))),
            Ok(key(KeyCode::Tab)),
            Ok(key(KeyCode::Enter)),
        ]);
        let mut renderer = RecordingRenderer::default();
        let mut terminal = InteractiveTerminal;

        let outcome = form
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
            .expect("form submits");
        let FormOutcome::Submitted(values) = outcome else {
            panic!("expected submitted values");
        };
        assert_eq!(values.get(&FieldKey::new("first")), Some(&"値".to_owned()));
        assert_eq!(
            values.get(&FieldKey::new("second")),
            Some(&"done".to_owned())
        );
        assert_eq!(renderer.views.len(), 4);
        assert_eq!(renderer.views[1].lines.len(), 8);
        assert!(renderer.views[1].lines.iter().any(|line| {
            line.runs()
                .iter()
                .any(|run| run.style == test_styles().error)
        }));
        assert_eq!(renderer.views[2].lines.len(), 7);
        assert_eq!(renderer.views[3].lines[3].runs()[0].text, "┃ ");
        assert_eq!(renderer.views[3].lines[3].runs()[1].text, "Second");
        assert_eq!(
            renderer.views[3].lines[1]
                .runs()
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>(),
            "  › 値"
        );
        assert_eq!(
            renderer.views[3]
                .lines
                .iter()
                .flat_map(PromptLine::runs)
                .filter(|run| run.style == test_styles().help)
                .count(),
            1
        );
    }
}
