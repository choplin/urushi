use std::any::Any;

use urushi::ComponentRole;

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, KeyCode, PromptView, RuntimeField, ViewLine, ViewSpan,
    },
};

/// The provenance of a submitted confirmation value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmSource {
    /// The user submitted the configured default without selecting an answer.
    Default,
    /// The user selected an answer with a key before submitting.
    Explicit,
}

/// A submitted confirmation value and how it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmAnswer {
    /// The selected yes (`true`) or no (`false`) answer.
    pub value: bool,
    /// Whether the value came from the default or an explicit keypress.
    pub source: ConfirmSource,
}

/// A yes-or-no field with an optional default answer.
pub struct Confirm {
    key: FieldKey<ConfirmAnswer>,
    question: String,
    default: Option<bool>,
    selected: Option<bool>,
    source: Option<ConfirmSource>,
    yes_label: String,
    no_label: String,
    help: String,
    unanswered_message: String,
    show_unanswered: bool,
}

impl Confirm {
    /// Creates a confirmation field with an optional default answer.
    pub fn new(
        key: FieldKey<ConfirmAnswer>,
        question: impl Into<String>,
        default: Option<bool>,
    ) -> Result<Self, FieldConfigError> {
        if key.name().is_empty() {
            return Err(FieldConfigError::EmptyName);
        }

        Ok(Self {
            key,
            question: question.into(),
            default,
            selected: default,
            source: None,
            yes_label: "Yes".to_owned(),
            no_label: "No".to_owned(),
            help: "←/→ or y/n choose · Enter submit · Shift+Tab back · Esc cancel".to_owned(),
            unanswered_message: "Choose yes or no.".to_owned(),
            show_unanswered: false,
        })
    }

    /// Sets the labels shown for yes and no.
    #[must_use]
    pub fn labels(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.yes_label = yes.into();
        self.no_label = no.into();
        self
    }

    /// Sets the navigation hint shown beneath the choices.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Sets the message shown when a confirmation without a default is unanswered.
    #[must_use]
    pub fn unanswered_message(mut self, message: impl Into<String>) -> Self {
        self.unanswered_message = message.into();
        self
    }

    fn select_explicit(&mut self, value: bool) {
        self.selected = Some(value);
        self.source = Some(ConfirmSource::Explicit);
        self.show_unanswered = false;
    }

    fn submit(&mut self) -> FieldAction {
        match (self.source, self.default) {
            (Some(_), _) => FieldAction::Accept,
            (None, Some(default)) => {
                self.selected = Some(default);
                self.source = Some(ConfirmSource::Default);
                FieldAction::Accept
            }
            (None, None) => {
                self.show_unanswered = true;
                FieldAction::Stay
            }
        }
    }
}

impl runtime::private::Sealed for Confirm {
    fn into_entry(self: Box<Self>) -> FieldEntry {
        FieldEntry::new(self.key.name().to_owned(), self)
    }
}

impl RuntimeField for Confirm {
    fn event(&mut self, event: Event) -> FieldAction {
        let Event::Key(key) = event else {
            return FieldAction::Stay;
        };

        match key.code {
            KeyCode::Up | KeyCode::Left | KeyCode::Char('y') => {
                self.select_explicit(true);
                FieldAction::Stay
            }
            KeyCode::Down | KeyCode::Right | KeyCode::Char('n') => {
                self.select_explicit(false);
                FieldAction::Stay
            }
            KeyCode::Enter | KeyCode::Tab => self.submit(),
            _ => FieldAction::Stay,
        }
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        let source = match self.source {
            Some(source) => source,
            None => unreachable!("a confirm value is collected only after acceptance"),
        };
        Box::new(ConfirmAnswer {
            value: match self.selected {
                Some(value) => value,
                None => unreachable!("a confirm value is collected only after acceptance"),
            },
            source,
        })
    }

    fn view(&self) -> PromptView {
        let mut lines = vec![ViewLine {
            spans: vec![
                ViewSpan {
                    text: "? ".to_owned(),
                    role: ComponentRole::PromptQuestion,
                },
                ViewSpan {
                    text: self.question.clone(),
                    role: ComponentRole::PromptQuestion,
                },
            ],
        }];
        for (value, label) in [(true, &self.yes_label), (false, &self.no_label)] {
            let role = if self.selected == Some(value) {
                ComponentRole::PromptOptionSelected
            } else {
                ComponentRole::PromptOption
            };
            lines.push(ViewLine {
                spans: vec![
                    ViewSpan {
                        text: if self.selected == Some(value) {
                            "› ".to_owned()
                        } else {
                            "  ".to_owned()
                        },
                        role,
                    },
                    ViewSpan {
                        text: label.clone(),
                        role,
                    },
                ],
            });
        }
        if self.show_unanswered {
            lines.push(ViewLine {
                spans: vec![
                    ViewSpan {
                        text: "! ".to_owned(),
                        role: ComponentRole::PromptError,
                    },
                    ViewSpan {
                        text: self.unanswered_message.clone(),
                        role: ComponentRole::PromptError,
                    },
                ],
            });
        }
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
    use super::*;

    fn key(code: KeyCode) -> Event {
        Event::Key(runtime::KeyEvent {
            code,
            modifiers: runtime::KeyModifiers::default(),
        })
    }

    #[test]
    fn constructor_rejects_an_empty_name() {
        assert!(matches!(
            Confirm::new(FieldKey::new(""), "Continue?", None),
            Err(FieldConfigError::EmptyName)
        ));
    }

    #[test]
    fn default_is_selected_visually_but_source_is_undecided_until_submit() {
        let mut confirm = Confirm::new(FieldKey::new("confirm"), "Continue?", Some(false))
            .expect("confirm is valid");
        let initial = confirm.view();
        assert_eq!(initial.cursor, None);
        assert_eq!(
            initial.lines[0].spans[0].role,
            ComponentRole::PromptQuestion
        );
        assert_eq!(
            initial.lines[2].spans[0].role,
            ComponentRole::PromptOptionSelected
        );
        assert_eq!(confirm.source, None);

        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Accept);
        assert_eq!(confirm.source, Some(ConfirmSource::Default));
        let answer = confirm.take_value();
        assert_eq!(
            answer.downcast_ref::<ConfirmAnswer>(),
            Some(&ConfirmAnswer {
                value: false,
                source: ConfirmSource::Default,
            })
        );
    }

    #[test]
    fn explicit_choice_is_explicit_even_when_it_matches_the_default() {
        let mut confirm = Confirm::new(FieldKey::new("confirm"), "Continue?", Some(true))
            .expect("confirm is valid");
        assert_eq!(confirm.event(key(KeyCode::Char('y'))), FieldAction::Stay);
        assert_eq!(confirm.source, Some(ConfirmSource::Explicit));
        assert_eq!(confirm.event(key(KeyCode::Tab)), FieldAction::Accept);
        let answer = confirm.take_value();
        assert_eq!(
            answer.downcast_ref::<ConfirmAnswer>(),
            Some(&ConfirmAnswer {
                value: true,
                source: ConfirmSource::Explicit,
            })
        );
    }

    #[test]
    fn no_default_requires_an_explicit_choice_and_shows_help() {
        let mut confirm = Confirm::new(FieldKey::new("confirm"), "Continue?", None)
            .expect("confirm is valid")
            .labels("Proceed", "Stop")
            .help("Choose, then press Enter.")
            .unanswered_message("Choose an answer.");
        let initial = confirm.view();
        assert_eq!(initial.lines[1].spans[0].role, ComponentRole::PromptOption);
        assert_eq!(initial.lines[2].spans[0].role, ComponentRole::PromptOption);
        assert_eq!(initial.lines[1].spans[1].text, "Proceed");
        assert_eq!(initial.lines[2].spans[1].text, "Stop");
        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Stay);
        assert_eq!(
            confirm.view().lines[3].spans[0].role,
            ComponentRole::PromptError
        );
        assert_eq!(
            confirm.view().lines[4].spans[0].role,
            ComponentRole::PromptHelp
        );
        assert_eq!(confirm.view().lines[3].spans[1].text, "Choose an answer.");
        assert_eq!(
            confirm.view().lines[4].spans[0].text,
            "Choose, then press Enter."
        );
        assert_eq!(confirm.event(key(KeyCode::Right)), FieldAction::Stay);
        assert_eq!(confirm.view().lines.len(), 4);
        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Accept);
    }
}
