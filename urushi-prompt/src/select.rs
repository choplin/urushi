use std::any::Any;

use unicode_segmentation::UnicodeSegmentation;
use urushi::{Align, PrintableText, VerticalAlign, View};

use crate::{
    FieldConfigError, FieldKey,
    runtime::{
        self, Event, FieldAction, FieldEntry, FieldPresentation, FieldRegionKind, KeyCode,
        KeyModifiers, PromptStyles, RuntimeField, TextSpan, clipped_line_view, field_line_view,
        fixed_view, line_view, line_view_with_cursor, region, window_spans,
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
    /// `label` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
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
    description: Option<String>,
    options: Vec<SelectOption<T>>,
    selected: usize,
    filtered: Vec<usize>,
    filter: String,
    filtering: bool,
    visible_rows: usize,
    help: String,
    filtering_help: String,
    filtered_help: String,
    no_matches_message: String,
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

        let filtered = (0..options.len()).collect();
        Ok(Self {
            key,
            question: question.into(),
            description: None,
            options,
            selected: 0,
            filtered,
            filter: String::new(),
            filtering: false,
            visible_rows: 7,
            help: "↑/↓ select • enter continue • shift+tab back • esc cancel".to_owned(),
            filtering_help: "type to filter • ↑/↓ select • enter apply • esc close".to_owned(),
            filtered_help: "↑/↓ select • enter continue • / edit filter • esc clear".to_owned(),
            no_matches_message: "No matches".to_owned(),
        })
    }

    /// Sets the navigation hint shown beneath the options.
    /// `help` is plain text. Escape sequences in it are counted as ordinary
    /// characters when the prompt measures its cells, so a pre-styled string
    /// mis-aligns the field; style it through the prompt's theme instead.
    #[must_use]
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
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

    /// Sets the hints shown while editing and after applying a filter.
    #[must_use]
    pub fn filter_help(mut self, editing: impl Into<String>, applied: impl Into<String>) -> Self {
        self.filtering_help = editing.into();
        self.filtered_help = applied.into();
        self
    }

    /// Sets the message shown when filtering removes every option.
    #[must_use]
    pub fn no_matches_message(mut self, message: impl Into<String>) -> Self {
        self.no_matches_message = message.into();
        self
    }

    /// Limits the number of option rows shown before the list scrolls.
    #[must_use]
    pub fn visible_rows(mut self, rows: usize) -> Self {
        self.visible_rows = rows.max(1);
        self
    }

    fn previous(&mut self) {
        if let Some(position) = self.selected_position()
            && self.filtered.len() > 1
        {
            let position = (position + self.filtered.len() - 1) % self.filtered.len();
            self.selected = self.filtered[position];
        }
    }

    fn next(&mut self) {
        if let Some(position) = self.selected_position()
            && self.filtered.len() > 1
        {
            self.selected = self.filtered[(position + 1) % self.filtered.len()];
        }
    }

    fn selected_position(&self) -> Option<usize> {
        self.filtered
            .iter()
            .position(|index| *index == self.selected)
    }

    fn update_filter(&mut self) {
        let needle = self.filter.to_lowercase();
        self.filtered = self
            .options
            .iter()
            .enumerate()
            .filter_map(|(index, option)| {
                option
                    .label
                    .to_lowercase()
                    .contains(&needle)
                    .then_some(index)
            })
            .collect();
        if !self.filtered.contains(&self.selected)
            && let Some(first) = self.filtered.first()
        {
            self.selected = *first;
        }
    }

    fn append_filter(&mut self, text: &str) {
        self.filter
            .extend(text.chars().filter(|character| !character.is_control()));
        self.update_filter();
    }

    fn backspace_filter(&mut self) {
        if let Some((start, _)) = self.filter.grapheme_indices(true).next_back() {
            self.filter.truncate(start);
            self.update_filter();
        }
    }

    fn move_to(&mut self, position: usize) {
        if let Some(index) = self.filtered.get(position) {
            self.selected = *index;
        }
    }

    fn page(&mut self, direction: isize) {
        let Some(position) = self.selected_position() else {
            return;
        };
        let distance = (self.visible_rows / 2).max(1);
        let position = if direction.is_negative() {
            position.saturating_sub(distance)
        } else {
            (position + distance).min(self.filtered.len().saturating_sub(1))
        };
        self.move_to(position);
    }

    fn current_help(&self) -> &str {
        if self.filtering {
            &self.filtering_help
        } else if !self.filter.is_empty() {
            &self.filtered_help
        } else {
            &self.help
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
        if let Event::Paste(text) = event {
            if self.filtering {
                self.append_filter(&text);
            }
            return FieldAction::Stay;
        }
        let Event::Key(key) = event else {
            return FieldAction::Stay;
        };

        match (
            key.code,
            key.modifiers.contains(KeyModifiers::CONTROL),
            key.modifiers.contains(KeyModifiers::ALT),
        ) {
            (KeyCode::Escape, _, _) if self.filtering => {
                self.filtering = false;
                FieldAction::Handled
            }
            (KeyCode::Escape, _, _) if !self.filter.is_empty() => {
                self.filter.clear();
                self.update_filter();
                FieldAction::Handled
            }
            (KeyCode::Escape, _, _) => FieldAction::Cancel,
            (KeyCode::Char('/'), false, false) => {
                self.filtering = true;
                FieldAction::Stay
            }
            (KeyCode::Backspace, _, _) if self.filtering => {
                self.backspace_filter();
                FieldAction::Stay
            }
            (KeyCode::Char(character), false, false) if self.filtering => {
                self.append_filter(&character.to_string());
                FieldAction::Stay
            }
            (KeyCode::Up | KeyCode::Left, _, _)
            | (KeyCode::Char('k' | 'p'), true, _)
            | (KeyCode::Char('k'), false, false) => {
                self.previous();
                FieldAction::Stay
            }
            (KeyCode::Down | KeyCode::Right, _, _)
            | (KeyCode::Char('j' | 'n'), true, _)
            | (KeyCode::Char('j'), false, false) => {
                self.next();
                FieldAction::Stay
            }
            (KeyCode::Home, _, _) | (KeyCode::Char('g'), false, false) => {
                self.move_to(0);
                FieldAction::Stay
            }
            (KeyCode::End, _, _) | (KeyCode::Char('G'), false, false) => {
                self.move_to(self.filtered.len().saturating_sub(1));
                FieldAction::Stay
            }
            (KeyCode::Char('u'), true, _) => {
                self.page(-1);
                FieldAction::Stay
            }
            (KeyCode::Char('d'), true, _) => {
                self.page(1);
                FieldAction::Stay
            }
            (KeyCode::Enter | KeyCode::Tab, _, _) if self.filtering => {
                self.filtering = false;
                FieldAction::Stay
            }
            (KeyCode::Enter | KeyCode::Tab, _, _) if !self.filtered.is_empty() => {
                FieldAction::Accept
            }
            _ => FieldAction::Stay,
        }
    }

    fn take_value(&mut self) -> Box<dyn Any> {
        Box::new(self.options.remove(self.selected).value)
    }

    fn view(&self, styles: &PromptStyles, focused: bool, width: usize) -> FieldPresentation {
        let mut title_spans = vec![TextSpan::new(
            self.question.clone(),
            styles.question(focused).clone(),
        )];
        let mut cursor = None;
        if self.filtering || !self.filter.is_empty() {
            title_spans.push(TextSpan::new("  / ", styles.answer.clone()));
            if !self.filter.is_empty() {
                title_spans.push(TextSpan::new(self.filter.clone(), styles.answer.clone()));
            }
            if self.filtering {
                // The blank marks where typing continues, so it belongs to the
                // cursor and disappears with it when the field loses focus.
                if focused {
                    title_spans.push(TextSpan::new(" ", styles.cursor.clone()));
                    cursor = Some(
                        PrintableText::new(
                            format!("{}  / {}", self.question, self.filter).as_str(),
                        )
                        .width(),
                    );
                }
            }
        }
        // A filter typed past the terminal's width scrolls the title, the same
        // way an input scrolls its value, and for the same reason: the window
        // is chosen before the view is composed.
        let cursor = cursor.map(|column| {
            let (windowed, column) = window_spans(title_spans.clone(), column, width);
            title_spans = windowed;
            column
        });
        let title = match cursor {
            Some(column) => region(
                FieldRegionKind::Focus,
                field_line_view(styles, focused, line_view_with_cursor(title_spans, column)),
            ),
            None => field_line_view(styles, focused, line_view(title_spans)),
        };
        let mut body = vec![region(FieldRegionKind::Question, title)];
        let mut regions = vec![
            FieldRegionKind::Question,
            FieldRegionKind::Control,
            FieldRegionKind::Focus,
        ];
        if let Some(description) = &self.description {
            body.push(region(
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
        let list_height = self.visible_rows.min(self.options.len());
        let (start, end) = if self.filtered.is_empty() {
            (0, 0)
        } else {
            let selected_position = self.selected_position().unwrap_or(0);
            let start = selected_position
                .saturating_sub(self.visible_rows / 2)
                .min(self.filtered.len().saturating_sub(self.visible_rows));
            let end = (start + self.visible_rows).min(self.filtered.len());
            (start, end)
        };
        let mut controls = Vec::new();
        if self.filtered.is_empty() {
            let no_matches = region(
                FieldRegionKind::Error,
                field_line_view(
                    styles,
                    focused,
                    line_view(vec![TextSpan::new(
                        self.no_matches_message.clone(),
                        styles.error.clone(),
                    )]),
                ),
            );
            controls.push(if cursor.is_some() {
                no_matches
            } else {
                region(FieldRegionKind::Focus, no_matches)
            });
            regions.push(FieldRegionKind::Error);
        } else {
            controls.extend(self.filtered[start..end].iter().map(|index| {
                let option = &self.options[*index];
                let selected = *index == self.selected;
                let style = if selected {
                    &styles.option_selected
                } else {
                    &styles.option
                };
                let option = field_line_view(
                    styles,
                    focused,
                    View::row(
                        VerticalAlign::Top,
                        [
                            fixed_view(
                                2,
                                vec![TextSpan::new(
                                    if selected { "› " } else { "  " },
                                    style.clone(),
                                )],
                            ),
                            line_view(vec![TextSpan::new(option.label.clone(), style.clone())]),
                        ],
                    ),
                );
                if selected && cursor.is_none() {
                    region(FieldRegionKind::Focus, option)
                } else {
                    option
                }
            }));
        }
        let rendered_rows = if self.filtered.is_empty() {
            1
        } else {
            end - start
        };
        controls.extend(
            (rendered_rows..list_height)
                .map(|_| field_line_view(styles, focused, View::text("", styles.body.clone()))),
        );
        if self.options.len() > self.visible_rows {
            let above = start;
            let below = self.filtered.len().saturating_sub(end);
            let status = match (above, below) {
                (0, 0) => format!("  = {}", self.filtered.len()),
                (0, below) => format!("  ↓ {below}"),
                (above, 0) => format!("  ↑ {above}"),
                (above, below) => format!("  ↑ {above} • ↓ {below}"),
            };
            controls.push(field_line_view(
                styles,
                focused,
                line_view(vec![TextSpan::new(status, styles.muted.clone())]),
            ));
        }
        body.push(region(
            FieldRegionKind::Control,
            View::column(Align::Left, controls),
        ));
        let presentation = FieldPresentation::new(View::column(Align::Left, body))
            .with_help(clipped_line_view(vec![TextSpan::new(
                self.current_help().to_owned(),
                styles.help.clone(),
            )]))
            .with_regions(regions);
        if cursor.is_some() {
            presentation.with_cursor()
        } else {
            presentation
        }
    }

    fn blur(&mut self) {
        self.filtering = false;
    }

    fn captures_tab(&self) -> bool {
        self.filtering
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, io};

    use super::*;
    use crate::runtime::PromptView;
    use crate::{
        Confirm, ConfirmSource, Form, FormOutcome, Group, Input,
        runtime::{EventSource, RenderFinish, Renderer, test_styles},
    };

    fn key(code: KeyCode) -> Event {
        Event::Key(runtime::KeyEvent::new(code))
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
        let styles = test_styles();
        assert_eq!(single.event(key(KeyCode::Left)), FieldAction::Stay);
        assert!(
            single.view(&styles, true, 80).rows()[1]
                .text()
                .contains("› Only")
        );
        assert_eq!(single.event(key(KeyCode::Right)), FieldAction::Stay);
        assert!(
            single.view(&styles, true, 80).rows()[1]
                .text()
                .contains("› Only")
        );

        let mut multiple = Select::new(FieldKey::new("multiple"), "Multiple", options())
            .expect("multiple options are valid");
        assert_eq!(multiple.event(key(KeyCode::Up)), FieldAction::Stay);
        assert!(
            multiple.view(&styles, true, 80).rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.option_selected)
        );
        assert_eq!(multiple.event(key(KeyCode::Down)), FieldAction::Stay);
        assert!(
            multiple.view(&styles, true, 80).rows()[1]
                .runs()
                .iter()
                .any(|run| run.style == styles.option_selected)
        );
        assert_eq!(multiple.event(key(KeyCode::Right)), FieldAction::Stay);
        assert!(
            multiple.view(&styles, true, 80).rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.option_selected)
        );
        assert_eq!(multiple.event(key(KeyCode::Left)), FieldAction::Stay);
        assert!(
            multiple.view(&styles, true, 80).rows()[1]
                .runs()
                .iter()
                .any(|run| run.style == styles.option_selected)
        );
    }

    #[test]
    fn view_carries_resolved_styles_and_frame_classification() {
        let select = Select::new(FieldKey::new("choice"), "Choose", options())
            .expect("select is valid")
            .help("Use arrows, then Enter.");
        let styles = test_styles();
        let view = select.view(&styles, true, 80);
        assert_eq!(view.cursor, None);
        assert!(view.rows()[0].text().contains("Choose"));
        assert!(
            view.rows()[0]
                .runs()
                .iter()
                .any(|run| run.style == styles.question)
        );
        assert!(view.rows()[1].text().contains("› One"));
        assert!(
            view.rows()[1]
                .runs()
                .iter()
                .any(|run| run.style == styles.option_selected)
        );
        assert!(
            view.rows()[2]
                .runs()
                .iter()
                .any(|run| run.style == styles.option)
        );
        assert!(view.help.is_some());
        assert!(
            select.view(&styles, false, 80).rows()[0]
                .runs()
                .iter()
                .any(|run| run.style == styles.muted)
        );
    }

    #[test]
    fn long_lists_scroll_and_filter_without_leaking_escape_to_the_form() {
        let options = (0..12)
            .map(|index| SelectOption::new(format!("Item {index}"), index))
            .collect();
        let mut select = Select::new(FieldKey::new("choice"), "Choose", options)
            .expect("select is valid")
            .visible_rows(3);

        assert!(
            select
                .view(&test_styles(), true, 80)
                .rows()
                .iter()
                .any(|line| { line.runs().iter().any(|run| run.text == "  ↓ 9") })
        );
        assert_eq!(select.event(key(KeyCode::End)), FieldAction::Stay);
        assert_eq!(select.selected, 11);
        assert!(
            select
                .view(&test_styles(), true, 80)
                .rows()
                .iter()
                .any(|line| { line.runs().iter().any(|run| run.text == "  ↑ 9") })
        );

        assert_eq!(select.event(key(KeyCode::Char('/'))), FieldAction::Stay);
        assert_eq!(select.event(key(KeyCode::Char('1'))), FieldAction::Stay);
        assert_eq!(select.filtered, vec![1, 10, 11]);
        assert_eq!(select.event(key(KeyCode::Escape)), FieldAction::Handled);
        assert_eq!(select.filter, "1");
        assert_eq!(select.event(key(KeyCode::Escape)), FieldAction::Handled);
        assert!(select.filter.is_empty());
        assert_eq!(select.filtered.len(), 12);
    }

    #[test]
    fn an_empty_filter_result_cannot_be_submitted() {
        let mut select =
            Select::new(FieldKey::new("choice"), "Choose", options()).expect("select is valid");
        assert_eq!(select.event(key(KeyCode::Char('/'))), FieldAction::Stay);
        assert_eq!(
            select.event(Event::Paste("missing".to_owned())),
            FieldAction::Stay
        );
        assert!(select.filtered.is_empty());
        assert_eq!(select.event(key(KeyCode::Enter)), FieldAction::Stay);
        let presentation = select.view(&test_styles(), true, 80);
        assert!(
            presentation
                .regions
                .iter()
                .any(|region| region.kind == FieldRegionKind::Error)
        );
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

        fn poll_event(&mut self) -> io::Result<Option<Event>> {
            Ok(None)
        }

        fn poll_event_timeout(
            &mut self,
            _timeout: std::time::Duration,
        ) -> io::Result<Option<Event>> {
            self.poll_event()
        }
    }

    #[derive(Default)]
    struct RecordingRenderer {
        views: Vec<PromptView>,
    }

    impl Renderer for RecordingRenderer {
        fn draw(
            &mut self,
            _output: &mut dyn urushi_terminal::CommandWriter,
            view: &PromptView,
            _start: crate::PromptStart,
            _drawing_columns: u16,
        ) -> io::Result<()> {
            self.views.push(view.clone());
            Ok(())
        }

        fn finish(
            &mut self,
            _output: &mut dyn urushi_terminal::CommandWriter,
            _outcome: RenderFinish,
        ) -> io::Result<()> {
            Ok(())
        }

        fn clear_viewport(
            &mut self,
            _output: &mut dyn urushi_terminal::CommandWriter,
        ) -> io::Result<()> {
            Ok(())
        }
    }

    fn rendered(view: &PromptView) -> crate::runtime::frame::FramedView {
        crate::runtime::frame::frame(&crate::runtime::resolve::resolve_prompt(80, view), u16::MAX)
    }

    struct InteractiveTerminal;

    impl urushi_terminal::RawModeControl for InteractiveTerminal {
        fn is_interactive(&self) -> bool {
            true
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl urushi_terminal::CommandWriter for InteractiveTerminal {
        fn write_command(&mut self, _command: urushi_terminal::Command<'_>) -> io::Result<()> {
            Ok(())
        }
    }

    impl urushi_terminal::TerminalQuery for InteractiveTerminal {
        fn terminal_size(&mut self) -> io::Result<urushi_terminal::TerminalSize> {
            Ok(urushi_terminal::TerminalSize::new(80, 24))
        }

        fn cursor_position(&mut self) -> io::Result<urushi_terminal::Position> {
            Ok(urushi_terminal::Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<urushi_terminal::WindowSize> {
            Ok(urushi_terminal::WindowSize::new(
                urushi_terminal::TerminalSize::new(80, 24),
                None,
            ))
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    impl urushi_terminal::KeyboardEnhancementQuery for InteractiveTerminal {
        fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    impl urushi_terminal::TerminalOutput for InteractiveTerminal {
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
            .run_with(&mut events, &mut renderer, &mut terminal, &test_styles())
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
        for view in [&renderer.views[1], &renderer.views[5]] {
            assert!(rendered(view).rows.iter().any(|row| {
                row.text().contains("Second")
                    && row
                        .runs()
                        .iter()
                        .any(|run| run.style == test_styles().option_selected)
            }));
        }
    }
}
