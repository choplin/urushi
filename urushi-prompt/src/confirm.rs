use std::any::Any;

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, KeyCode, LineKind, PromptLine, PromptStyles,
        PromptView, RuntimeField, ViewSpan, clipped_line_view, fixed_view, line_view,
    },
};
use urushi::{Align, PrintableText, VerticalAlign, View};

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
    description: Option<String>,
    default: Option<bool>,
    selected: Option<bool>,
    source: Option<ConfirmSource>,
    yes_label: String,
    no_label: String,
    button_alignment: Align,
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
            description: None,
            default,
            selected: default,
            source: None,
            yes_label: "Yes".to_owned(),
            no_label: "No".to_owned(),
            button_alignment: Align::Left,
            help: "←/→ choose • y yes • n no • enter submit • shift+tab back • esc cancel"
                .to_owned(),
            unanswered_message: "Choose yes or no.".to_owned(),
            show_unanswered: false,
        })
    }

    /// Sets the labels shown for yes and no.
    /// `yes` and `no` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    #[must_use]
    pub fn labels(mut self, yes: impl Into<String>, no: impl Into<String>) -> Self {
        self.yes_label = yes.into();
        self.no_label = no.into();
        self
    }

    /// Sets the horizontal alignment of the yes and no buttons.
    ///
    /// The default is [`Align::Left`]. Center and right alignment position the
    /// buttons within the natural width of the question and description.
    #[must_use]
    pub fn button_alignment(mut self, alignment: Align) -> Self {
        self.button_alignment = alignment;
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

    /// Sets the navigation hint shown beneath the choices.
    /// `help` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
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

        match (key.code, key.modifiers.control, key.modifiers.alt) {
            (KeyCode::Escape, _, _) => FieldAction::Cancel,
            (KeyCode::Up | KeyCode::Left, _, _) | (KeyCode::Char('h'), false, false) => {
                self.select_explicit(true);
                FieldAction::Stay
            }
            (KeyCode::Down | KeyCode::Right, _, _) | (KeyCode::Char('l'), false, false) => {
                self.select_explicit(false);
                FieldAction::Stay
            }
            (KeyCode::Char('y' | 'Y'), false, false) => {
                self.select_explicit(true);
                FieldAction::Accept
            }
            (KeyCode::Char('n' | 'N'), false, false) => {
                self.select_explicit(false);
                FieldAction::Accept
            }
            (KeyCode::Enter | KeyCode::Tab, _, _) => self.submit(),
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

    fn view(&self, styles: &PromptStyles, focused: bool, _width: usize) -> PromptView {
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
        let left_padding = match self.button_alignment {
            Align::Left => 0,
            alignment @ (Align::Center | Align::Right) => {
                let button_width = PrintableText::new(self.yes_label.as_str())
                    .width()
                    .saturating_add(4)
                    .saturating_add(1)
                    .saturating_add(PrintableText::new(self.no_label.as_str()).width())
                    .saturating_add(4);
                let question_width = PrintableText::new(self.question.as_str()).width();
                let header_width = self
                    .description
                    .as_deref()
                    .map_or(question_width, |description| {
                        question_width.max(PrintableText::new(description).width())
                    });
                let remaining_width = header_width.saturating_sub(button_width);
                match alignment {
                    Align::Center => remaining_width / 2,
                    Align::Right => remaining_width,
                    Align::Left => unreachable!("left alignment is handled above"),
                }
            }
        };
        lines.push(PromptLine::blank());
        let mut buttons = Vec::new();
        for (index, (value, label)) in [(true, &self.yes_label), (false, &self.no_label)]
            .into_iter()
            .enumerate()
        {
            let button_focused = self.selected == Some(value);
            let style = if button_focused {
                &styles.button_focused
            } else {
                &styles.button
            };
            if index > 0 {
                buttons.push(ViewSpan::new(" ", &styles.body));
            }
            buttons.push(ViewSpan::new(format!("  {label}  "), style));
        }
        // The buttons reflow rather than being cut: a viewport too narrow for
        // them should cost a row, not a label. Which rows survive after that
        // is the frame stage's decision, and a button row is one it never
        // displaces.
        //
        // Alignment is a decision about where the group starts, so the leading
        // blank is a column of its own rather than text the group could be
        // reflowed away from.
        let buttons = if left_padding > 0 {
            View::row(
                VerticalAlign::Top,
                [
                    fixed_view(
                        left_padding,
                        vec![ViewSpan::new(" ".repeat(left_padding), &styles.body)],
                    ),
                    line_view(buttons),
                ],
            )
        } else {
            line_view(buttons)
        };
        lines.push(PromptLine::new(buttons).with_kind(LineKind::Choice {
            focused: self.selected.is_some(),
        }));
        if self.show_unanswered {
            lines.push(
                PromptLine::new(View::row(
                    VerticalAlign::Top,
                    [
                        fixed_view(2, vec![ViewSpan::new("! ", &styles.error)]),
                        View::text(self.unanswered_message.clone(), styles.error.clone()),
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
            cursor: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::test_styles;

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
        let styles = test_styles();
        let initial = confirm.view(&styles, true, 80);
        assert_eq!(initial.cursor, None);
        assert_eq!(initial.lines[0].runs()[0].style, styles.question);
        assert!(
            initial.lines[2]
                .runs()
                .iter()
                .any(|span| span.style == styles.button_focused && span.text == "  No  ")
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
        assert_eq!(confirm.event(key(KeyCode::Char('y'))), FieldAction::Accept);
        assert_eq!(confirm.source, Some(ConfirmSource::Explicit));
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
        let styles = test_styles();
        let initial = confirm.view(&styles, true, 80);
        assert_eq!(initial.lines[2].runs()[0].style, styles.button);
        assert_eq!(initial.lines[2].runs()[2].style, styles.button);
        assert_eq!(initial.lines[2].runs()[0].text, "  Proceed  ");
        assert_eq!(initial.lines[2].runs()[2].text, "  Stop  ");
        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Stay);
        let answered = confirm.view(&styles, true, 80);
        assert_eq!(answered.lines[3].kind, LineKind::Error);
        assert_eq!(answered.lines[3].runs()[0].style, styles.error);
        assert_eq!(answered.lines[4].kind, LineKind::Help);
        assert_eq!(answered.lines[4].runs()[0].style, styles.help);
        assert_eq!(answered.lines[3].runs()[0].text, "! Choose an answer.");
        assert_eq!(
            answered.lines[4].runs()[0].text,
            "Choose, then press Enter."
        );
        assert_eq!(confirm.event(key(KeyCode::Right)), FieldAction::Stay);
        assert_eq!(confirm.view(&styles, true, 80).lines.len(), 4);
        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Accept);
    }

    #[test]
    fn buttons_are_left_aligned_by_default() {
        let confirm = Confirm::new(
            FieldKey::new("confirm"),
            "Generate the personalized greeting?",
            Some(true),
        )
        .expect("confirm is valid");

        let styles = test_styles();
        let view = confirm.view(&styles, true, 80);
        assert!(view.lines[1].runs().is_empty());
        assert_eq!(view.lines[2].runs()[0].style, styles.button_focused);
        assert_eq!(view.lines[2].runs()[0].text, "  Yes  ");
    }

    #[test]
    fn buttons_can_be_centered_within_the_natural_header_width() {
        let confirm = Confirm::new(
            FieldKey::new("confirm"),
            "Generate the personalized greeting?",
            Some(true),
        )
        .expect("confirm is valid")
        .button_alignment(Align::Center);

        let styles = test_styles();
        let view = confirm.view(&styles, true, 80);
        assert_eq!(view.lines[2].runs()[0].style, styles.body);
        assert_eq!(view.lines[2].runs()[0].text, "          ");
        assert_eq!(view.lines[2].runs()[1].style, styles.button_focused);
        assert_eq!(view.lines[2].runs()[1].text, "  Yes  ");
    }

    #[test]
    fn buttons_can_be_right_aligned_within_the_natural_header_width() {
        let confirm = Confirm::new(
            FieldKey::new("confirm"),
            "Generate the personalized greeting?",
            Some(true),
        )
        .expect("confirm is valid")
        .button_alignment(Align::Right);

        let styles = test_styles();
        let view = confirm.view(&styles, true, 80);
        assert_eq!(view.lines[2].runs()[0].style, styles.body);
        assert_eq!(view.lines[2].runs()[0].text, "                     ");
        assert_eq!(view.lines[2].runs()[1].style, styles.button_focused);
        assert_eq!(view.lines[2].runs()[1].text, "  Yes  ");
    }
}
