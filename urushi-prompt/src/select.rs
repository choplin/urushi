use std::any::Any;

use urushi::ComponentRole;

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, KeyCode, PromptView, RuntimeField, ViewLine, ViewSpan,
    },
};

/// A labelled value offered by [`Select`].
pub struct SelectOption<T> {
    /// The text shown to the user.
    pub label: String,
    /// The typed value submitted when this option is selected.
    pub value: T,
}

impl<T> SelectOption<T> {
    /// Creates a selectable typed value with a display label.
    pub fn new(label: impl Into<String>, value: T) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

/// A field that selects one value from a non-empty list of options.
pub struct Select<T> {
    key: FieldKey<T>,
    question: String,
    options: Vec<SelectOption<T>>,
    selected: usize,
    help: String,
}

impl<T> Select<T> {
    /// Creates a select field with the first option initially selected.
    pub fn new(
        key: FieldKey<T>,
        question: impl Into<String>,
        options: Vec<SelectOption<T>>,
    ) -> Result<Self, FieldConfigError> {
        if key.name().is_empty() {
            return Err(FieldConfigError::EmptyName);
        }
        if options.is_empty() {
            return Err(FieldConfigError::EmptyOptions);
        }

        Ok(Self {
            key,
            question: question.into(),
            options,
            selected: 0,
            help: "↑/↓ select • enter continue • shift+tab back • esc cancel".to_owned(),
        })
    }

    /// Sets the navigation hint shown beneath the options.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    fn previous(&mut self) {
        if self.options.len() > 1 {
            self.selected = (self.selected + self.options.len() - 1) % self.options.len();
        }
    }

    fn next(&mut self) {
        if self.options.len() > 1 {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }
}

impl<T: 'static> runtime::private::Sealed for Select<T> {
    fn into_entry(self: Box<Self>) -> FieldEntry {
        FieldEntry::new(self.key.name().to_owned(), self)
    }
}

impl<T: 'static> RuntimeField for Select<T> {
    fn event(&mut self, event: Event) -> FieldAction {
        let Event::Key(key) = event else {
            return FieldAction::Stay;
        };

        match key.code {
            KeyCode::Up | KeyCode::Left => {
                self.previous();
                FieldAction::Stay
            }
            KeyCode::Down | KeyCode::Right => {
                self.next();
                FieldAction::Stay
            }
            KeyCode::Enter | KeyCode::Tab => FieldAction::Accept,
            _ => FieldAction::Stay,
        }
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        Box::new(self.options.remove(self.selected).value)
    }

    fn view(&self) -> PromptView {
        let mut lines = vec![ViewLine {
            spans: vec![ViewSpan {
                text: self.question.clone(),
                role: ComponentRole::PromptQuestion,
            }],
        }];
        lines.extend(
            self.options
                .iter()
                .enumerate()
                .map(|(index, option)| ViewLine {
                    spans: {
                        let role = if index == self.selected {
                            ComponentRole::PromptOptionSelected
                        } else {
                            ComponentRole::PromptOption
                        };
                        vec![
                            ViewSpan {
                                text: if index == self.selected { "› " } else { "  " }.to_owned(),
                                role,
                            },
                            ViewSpan {
                                text: option.label.clone(),
                                role,
                            },
                        ]
                    },
                }),
        );
        lines.push(ViewLine {
            spans: vec![ViewSpan {
                text: self.help.clone(),
                role: ComponentRole::PromptHelp,
            }],
        });

        PromptView {
            lines,
            cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io};

    use super::*;
    use crate::{
        Confirm, ConfirmSource, Form, FormOutcome, Group, Input,
        runtime::{EventSource, RenderFinish, Renderer, TerminalControl},
    };

    fn key(code: KeyCode) -> Event {
        Event::Key(runtime::KeyEvent {
            code,
            modifiers: runtime::KeyModifiers::default(),
        })
    }

    fn options() -> Vec<SelectOption<&'static str>> {
        vec![
            SelectOption::new("One", "one"),
            SelectOption::new("Two", "two"),
        ]
    }

    #[test]
    fn construction_rejects_empty_names_and_options() {
        assert!(matches!(
            Select::<String>::new(
                FieldKey::new(""),
                "Question",
                vec![SelectOption::new("One", String::new())]
            ),
            Err(FieldConfigError::EmptyName)
        ));
        assert!(matches!(
            Select::<String>::new(FieldKey::new("answer"), "Question", Vec::new()),
            Err(FieldConfigError::EmptyOptions)
        ));
    }

    #[test]
    fn single_option_is_stable_and_multiple_options_cycle() {
        let mut single = Select::new(
            FieldKey::new("single"),
            "Single",
            vec![SelectOption::new("Only", 1)],
        )
        .expect("single option is valid");
        assert_eq!(single.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(single.view().lines[1].spans[1].text, "Only");
        assert_eq!(single.event(key(KeyCode::Right)), FieldAction::Stay);
        assert_eq!(single.view().lines[1].spans[1].text, "Only");

        let mut multiple = Select::new(FieldKey::new("multiple"), "Multiple", options())
            .expect("multiple options are valid");
        assert_eq!(multiple.event(key(KeyCode::Up)), FieldAction::Stay);
        assert_eq!(
            multiple.view().lines[2].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(multiple.event(key(KeyCode::Down)), FieldAction::Stay);
        assert_eq!(
            multiple.view().lines[1].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(multiple.event(key(KeyCode::Right)), FieldAction::Stay);
        assert_eq!(
            multiple.view().lines[2].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(multiple.event(key(KeyCode::Left)), FieldAction::Stay);
        assert_eq!(
            multiple.view().lines[1].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
    }

    #[test]
    fn view_has_question_and_option_semantic_roles() {
        let select = Select::new(FieldKey::new("choice"), "Choose", options())
            .expect("select is valid")
            .help("Use arrows, then Enter.");
        let view = select.view();
        assert_eq!(view.cursor, None);
        assert_eq!(view.lines[0].spans[0].text, "Choose");
        assert_eq!(view.lines[0].spans[0].role, ComponentRole::PromptQuestion);
        assert_eq!(view.lines[1].spans[0].text, "› ");
        assert_eq!(
            view.lines[1].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(view.lines[2].spans[0].role, ComponentRole::PromptOption);
        assert_eq!(view.lines[3].spans[0].role, ComponentRole::PromptHelp);
        assert_eq!(view.lines[3].spans[0].text, "Use arrows, then Enter.");
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

    #[derive(Debug, PartialEq, Eq)]
    struct NonCloneValue(&'static str);

    #[test]
    fn submit_moves_non_clone_values_and_back_preserves_choices() {
        let selection_key = FieldKey::new("selection");
        let confirmation_key = FieldKey::new("confirmation");
        let tail_key = FieldKey::new("tail");
        let selection = Select::new(
            selection_key.clone(),
            "Select",
            vec![
                SelectOption::new("First", NonCloneValue("first")),
                SelectOption::new("Second", NonCloneValue("second")),
            ],
        )
        .expect("select is valid");
        let confirmation = Confirm::new(confirmation_key.clone(), "Confirm", Some(true))
            .expect("confirm is valid");
        let tail = Input::new(tail_key.clone(), "Tail", "done").expect("input is valid");
        let form = Form::builder()
            .group(
                Group::builder()
                    .field(selection)
                    .field(confirmation)
                    .field(tail)
                    .build()
                    .expect("group is valid"),
            )
            .build()
            .expect("form is valid");
        let mut events = ScriptedEvents::new([
            Ok(key(KeyCode::Down)),
            Ok(key(KeyCode::Enter)),
            Ok(key(KeyCode::Right)),
            Ok(key(KeyCode::Tab)),
            Ok(key(KeyCode::BackTab)),
            Ok(key(KeyCode::Enter)),
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
        assert_eq!(values.get(&selection_key), Some(&NonCloneValue("second")));
        assert_eq!(
            values.get(&confirmation_key).map(|answer| answer.source),
            Some(ConfirmSource::Explicit)
        );
        assert_eq!(
            values.get(&confirmation_key).map(|answer| answer.value),
            Some(false)
        );
        assert_eq!(values.get(&tail_key), Some(&"done".to_owned()));
        assert_eq!(
            renderer.views[1].lines[2].spans[1].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(
            renderer.views[5].lines[2].spans[1].role,
            ComponentRole::PromptOptionSelected
        );
    }
}
