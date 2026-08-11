use std::any::Any;

use urushi::{ComponentRole, visible_width};

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, KeyCode, PromptView, RuntimeField, ViewCursor,
        ViewLine, ViewSpan,
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
    value: String,
    placeholder: Option<String>,
    required: bool,
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
        let cursor = value.chars().count();
        Ok(Self {
            key,
            question: question.into(),
            value,
            placeholder: None,
            required: false,
            validators: Vec::new(),
            cursor,
            validation_error: None,
        })
    }

    /// Sets text shown when the current value is empty.
    #[must_use]
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Requires a non-empty value before the form can advance.
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
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

    fn byte_index(&self, scalar_index: usize) -> usize {
        self.value
            .char_indices()
            .nth(scalar_index)
            .map_or(self.value.len(), |(byte_index, _)| byte_index)
    }

    fn delete_scalar_at(&mut self, scalar_index: usize) {
        let start = self.byte_index(scalar_index);
        let end = self.byte_index(scalar_index + 1);
        if start != end {
            self.value.replace_range(start..end, "");
        }
    }

    fn submit(&mut self) -> FieldAction {
        if self.required && self.value.is_empty() {
            self.validation_error = Some("This field is required.".to_owned());
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

    fn answer_spans(&self) -> Vec<ViewSpan> {
        if self.value.is_empty() {
            let mut spans = vec![ViewSpan {
                text: " ".to_owned(),
                role: ComponentRole::PromptCursor,
            }];
            if let Some(placeholder) = &self.placeholder {
                spans.push(ViewSpan {
                    text: placeholder.clone(),
                    role: ComponentRole::PromptPlaceholder,
                });
            }
            return spans;
        }

        let cursor_byte = self.byte_index(self.cursor);
        let mut spans = Vec::new();
        if cursor_byte > 0 {
            spans.push(ViewSpan {
                text: self.value[..cursor_byte].to_owned(),
                role: ComponentRole::PromptAnswer,
            });
        }
        if let Some(character) = self.value[cursor_byte..].chars().next() {
            spans.push(ViewSpan {
                text: character.to_string(),
                role: ComponentRole::PromptCursor,
            });
            let after_cursor = cursor_byte + character.len_utf8();
            if after_cursor < self.value.len() {
                spans.push(ViewSpan {
                    text: self.value[after_cursor..].to_owned(),
                    role: ComponentRole::PromptAnswer,
                });
            }
        } else {
            spans.push(ViewSpan {
                text: " ".to_owned(),
                role: ComponentRole::PromptCursor,
            });
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
        let Event::Key(key) = event else {
            return FieldAction::Stay;
        };

        match key.code {
            KeyCode::Char(character) => {
                let byte_index = self.byte_index(self.cursor);
                self.value.insert(byte_index, character);
                self.cursor += 1;
                self.edit();
                FieldAction::Stay
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                FieldAction::Stay
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.value.chars().count());
                FieldAction::Stay
            }
            KeyCode::Home => {
                self.cursor = 0;
                FieldAction::Stay
            }
            KeyCode::End => {
                self.cursor = self.value.chars().count();
                FieldAction::Stay
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.delete_scalar_at(self.cursor - 1);
                self.cursor -= 1;
                self.edit();
                FieldAction::Stay
            }
            KeyCode::Delete if self.cursor < self.value.chars().count() => {
                self.delete_scalar_at(self.cursor);
                self.edit();
                FieldAction::Stay
            }
            KeyCode::Enter | KeyCode::Tab => self.submit(),
            _ => FieldAction::Stay,
        }
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        Box::new(std::mem::take(&mut self.value))
    }

    fn view(&self) -> PromptView {
        let answer_start = visible_width(&self.question).saturating_add(1);
        let cursor_prefix = &self.value[..self.byte_index(self.cursor)];
        let cursor_column = answer_start.saturating_add(visible_width(cursor_prefix));
        let mut lines = vec![ViewLine {
            spans: vec![
                ViewSpan {
                    text: self.question.clone(),
                    role: ComponentRole::PromptQuestion,
                },
                ViewSpan {
                    text: " ".to_owned(),
                    role: ComponentRole::PromptQuestion,
                },
            ],
        }];
        lines[0].spans.extend(self.answer_spans());
        if let Some(message) = &self.validation_error {
            lines.push(ViewLine {
                spans: vec![ViewSpan {
                    text: message.clone(),
                    role: ComponentRole::PromptError,
                }],
            });
        }

        PromptView {
            lines,
            cursor: Some(ViewCursor {
                row: 0,
                column: cursor_column.min(usize::from(u16::MAX)) as u16,
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
        runtime::{EventSource, RenderFinish, Renderer, TerminalControl},
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
    fn edits_follow_unicode_scalar_boundaries() {
        let mut input = Input::new(FieldKey::new("name"), "Name", "aあb").expect("input is valid");

        assert_eq!(input.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(input.event(key(KeyCode::Char('い'))), FieldAction::Stay);
        assert_eq!(input.value(), "aあいb");
        assert_eq!(input.event(key(KeyCode::Backspace)), FieldAction::Stay);
        assert_eq!(input.value(), "aあb");
        assert_eq!(input.event(key(KeyCode::Delete)), FieldAction::Stay);
        assert_eq!(input.value(), "aあ");
    }

    #[test]
    fn view_uses_semantic_roles_and_cjk_display_columns() {
        let mut input = Input::new(FieldKey::new("name"), "名前", "あ")
            .expect("input is valid")
            .placeholder("入力してください");
        assert_eq!(input.event(key(KeyCode::Home)), FieldAction::Stay);

        let view = input.view();
        assert_eq!(
            view.lines[0].spans,
            vec![
                ViewSpan {
                    text: "名前".to_owned(),
                    role: ComponentRole::PromptQuestion,
                },
                ViewSpan {
                    text: " ".to_owned(),
                    role: ComponentRole::PromptQuestion,
                },
                ViewSpan {
                    text: "あ".to_owned(),
                    role: ComponentRole::PromptCursor,
                },
            ]
        );
        assert_eq!(view.cursor, Some(ViewCursor { row: 0, column: 5 }));

        let empty = Input::new(FieldKey::new("empty"), "Name", "")
            .expect("input is valid")
            .placeholder("Example");
        assert_eq!(
            empty.view().lines[0].spans[2].role,
            ComponentRole::PromptCursor
        );
        assert_eq!(
            empty.view().lines[0].spans[3].role,
            ComponentRole::PromptPlaceholder
        );

        assert_eq!(input.event(key(KeyCode::End)), FieldAction::Stay);
        assert_eq!(
            input.view().lines[0].spans.last(),
            Some(&ViewSpan {
                text: " ".to_owned(),
                role: ComponentRole::PromptCursor,
            })
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
            .validate(Box::new(|_| Err(ValidationError::new("validator failure"))));
        assert_eq!(required.event(key(KeyCode::Tab)), FieldAction::Stay);
        assert_eq!(required.validation_error(), Some("This field is required."));
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
        fn draw(&mut self, view: &PromptView) -> io::Result<()> {
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
            .run_with(&mut events, &mut renderer, &mut terminal)
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
        assert_eq!(renderer.views[1].lines.len(), 2);
        assert_eq!(
            renderer.views[1].lines[1].spans[0].role,
            ComponentRole::PromptError
        );
        assert_eq!(renderer.views[2].lines.len(), 1);
        assert_eq!(renderer.views[3].lines[0].spans[0].text, "Second");
    }
}
