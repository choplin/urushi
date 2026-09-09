use std::any::Any;

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, FieldPresentation, FieldRegionKind, KeyCode,
        PromptStyles, RuntimeField, TextSpan, clipped_line_view, field_line_view, fixed_view,
        line_view, region,
    },
};
use urushi::{Align, BlockStyle, GridStyle, Length, VerticalAlign, View};

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

    fn view(&self, styles: &PromptStyles, focused: bool, _width: usize) -> FieldPresentation {
        let question = region(
            FieldRegionKind::Question,
            field_line_view(
                styles,
                focused,
                line_view(vec![TextSpan::new(
                    self.question.clone(),
                    styles.question(focused).clone(),
                )]),
            ),
        );
        let mut header = vec![question];
        let mut regions = vec![
            FieldRegionKind::Question,
            FieldRegionKind::Control,
            FieldRegionKind::Focus,
        ];
        if let Some(description) = &self.description {
            header.push(region(
                FieldRegionKind::Description,
                field_line_view(
                    styles,
                    focused,
                    line_view(vec![TextSpan::new(
                        description.clone(),
                        styles.muted.clone(),
                    )]),
                ),
            ));
            regions.push(FieldRegionKind::Description);
        }
        header.push(field_line_view(
            styles,
            focused,
            View::text("", styles.body.clone()),
        ));
        let mut buttons = Vec::new();
        let focused_value = self.selected.unwrap_or(true);
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
                buttons.push(View::text(" ", styles.body.clone()));
            }
            let button = View::text(format!("  {label}  "), style.clone());
            buttons.push(if focused_value == value {
                region(FieldRegionKind::Focus, button)
            } else {
                button
            });
        }
        let aligned_buttons = View::block(
            BlockStyle::new()
                .width(Length::fill(1))
                .align(self.button_alignment),
            View::row(VerticalAlign::Top, buttons),
        );
        let buttons = region(
            FieldRegionKind::Control,
            field_line_view(styles, focused, aligned_buttons),
        );
        let alignment_group = View::grid(
            GridStyle::new(),
            [
                [View::block(
                    BlockStyle::new().align(Align::Left),
                    View::column(Align::Left, header),
                )],
                [buttons],
            ],
        );
        let mut body = vec![alignment_group];
        if self.show_unanswered {
            body.push(region(
                FieldRegionKind::Error,
                field_line_view(
                    styles,
                    focused,
                    View::row(
                        VerticalAlign::Top,
                        [
                            fixed_view(2, vec![TextSpan::new("! ", styles.error.clone())]),
                            View::text(self.unanswered_message.clone(), styles.error.clone()),
                        ],
                    ),
                ),
            ));
            regions.push(FieldRegionKind::Error);
        }
        FieldPresentation::new(View::column(Align::Left, body))
            .with_help(clipped_line_view(vec![TextSpan::new(
                self.help.clone(),
                styles.help.clone(),
            )]))
            .with_regions(regions)
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
        assert!(
            initial.rows()[0]
                .runs()
                .iter()
                .any(|run| run.style == styles.question)
        );
        assert!(
            initial.rows()[2]
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
        assert!(
            initial.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.button && run.text == "  Proceed  ")
        );
        assert!(
            initial.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.button && run.text == "  Stop  ")
        );
        assert_eq!(confirm.event(key(KeyCode::Enter)), FieldAction::Stay);
        let answered = confirm.view(&styles, true, 80);
        assert!(
            answered
                .regions
                .iter()
                .any(|region| region.kind == FieldRegionKind::Error)
        );
        assert!(answered.rows()[3].text().contains("! Choose an answer."));
        assert!(answered.help.is_some());
        assert_eq!(confirm.event(key(KeyCode::Right)), FieldAction::Stay);
        assert_eq!(confirm.view(&styles, true, 80).rows().len(), 3);
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
        assert_eq!(view.rows()[1].text().trim(), "┃");
        assert!(
            view.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.button_focused && run.text == "  Yes  ")
        );
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
        assert!(view.rows()[2].text().starts_with("┃ "));
        assert_eq!(view.rows()[2].text().find("Yes"), Some(16));
        assert!(
            view.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.button_focused && run.text == "  Yes  ")
        );
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
        assert!(view.rows()[2].text().starts_with("┃ "));
        assert_eq!(view.rows()[2].text().find("Yes"), Some(27));
        assert!(
            view.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.button_focused && run.text == "  Yes  ")
        );
    }

    #[test]
    fn grid_alignment_uses_the_widest_header_or_control_cell() {
        let styles = test_styles();
        let start = |confirm: Confirm, label: &str| {
            confirm
                .view(&styles, true, 80)
                .rows()
                .into_iter()
                .find_map(|row| row.text().find(label))
                .expect("button label is rendered")
        };

        let buttons_wider = |alignment| {
            Confirm::new(FieldKey::new("confirm"), "Q", Some(true))
                .expect("confirm")
                .labels("A deliberately long answer", "No")
                .button_alignment(alignment)
        };
        assert_eq!(
            start(buttons_wider(Align::Left), "A deliberately"),
            start(buttons_wider(Align::Center), "A deliberately")
        );
        assert_eq!(
            start(buttons_wider(Align::Left), "A deliberately"),
            start(buttons_wider(Align::Right), "A deliberately")
        );

        let description_wider = |alignment| {
            Confirm::new(FieldKey::new("confirm"), "Q", Some(true))
                .expect("confirm")
                .description("A description wider than either button row")
                .button_alignment(alignment)
        };
        let left = start(description_wider(Align::Left), "Yes");
        let center = start(description_wider(Align::Center), "Yes");
        let right = start(description_wider(Align::Right), "Yes");
        assert!(left < center && center < right);
    }
}
