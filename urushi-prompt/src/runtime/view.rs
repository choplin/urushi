//! Prompt views and the presentation values fields compose.

use urushi::{
    BlockStyle, ComponentRole, Key, Overflow, PrintableText, StyledText, TerminalProfile, TextSpan,
    TextStyle, Theme, VerticalAlign, View,
};

#[cfg(test)]
use super::frame;

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
    pub(super) fn active_name(&self) -> Option<String> {
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
    pub(crate) fn spans(spans: Vec<TextSpan>) -> Self {
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

/// Composes styled segments into one text flow.
pub(crate) fn line_view(spans: Vec<TextSpan>) -> View {
    View::styled_text(styled_text(spans))
}

/// Composes a line and places the prompt cursor anchor at one display column.
pub(crate) fn line_view_with_cursor(spans: Vec<TextSpan>, column: usize) -> View {
    let mut before: Vec<TextSpan> = Vec::new();
    let mut after: Vec<TextSpan> = Vec::new();
    let mut width = 0;
    for span in spans {
        for grapheme in PrintableText::new(span.text()).graphemes() {
            let target = if width < column {
                &mut before
            } else {
                &mut after
            };
            target.push(TextSpan::new(grapheme.as_str(), span.style().clone()));
            width = width.saturating_add(grapheme.width());
        }
    }
    View::row(
        VerticalAlign::Top,
        [
            line_view(before),
            View::anchor(cursor_key()),
            line_view(after),
        ],
    )
}

/// Composes styled runs into a line that is cut, not reflowed, when it is
/// wider than the terminal.
pub(crate) fn clipped_line_view(spans: Vec<TextSpan>) -> View {
    View::block(
        BlockStyle::new().overflow(Overflow::clip()),
        line_view(spans),
    )
}

/// A run of fixed width, placed beside content that may reflow.
///
/// Sizing it in cells is what pins it: a row shrinks its `Fill` children
/// first, then its auto children, and only then the ones that stated a size.
/// An unpinned marker would be reflowed away with the text it marks.
pub(crate) fn fixed_view(width: usize, spans: Vec<TextSpan>) -> View {
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
            fixed_view(GUTTER, vec![TextSpan::new(marker, style.clone())]),
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
    spans: Vec<TextSpan>,
    cursor: usize,
    width: usize,
) -> (Vec<TextSpan>, usize) {
    if width == 0 {
        return (Vec::new(), 0);
    }
    let offset = cursor.saturating_sub(width - 1);
    let mut windowed: Vec<TextSpan> = Vec::new();
    let mut seen = 0;
    let mut start = None;
    let mut used = 0;
    for span in &spans {
        for grapheme in PrintableText::new(span.text()).graphemes() {
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
            windowed.push(TextSpan::new(grapheme.as_str(), span.style().clone()));
            used += grapheme_width;
            seen += grapheme_width;
        }
    }
    (windowed, cursor.saturating_sub(start.unwrap_or(seen)))
}

fn styled_text(spans: Vec<TextSpan>) -> StyledText {
    StyledText::try_from_spans(spans)
        .expect("prompt text spans are always split at grapheme boundaries")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ViewCursor {
    pub row: u16,
    pub column: u16,
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::runtime::{
        Event, Form, FormState, KeyCode, KeyEvent, KeyModifiers, PromptLine, PromptView,
        ReducerResult, TextSpan, ViewCursor, frame, resolve, terminal::tests::*,
    };
    use crate::{Confirm, FieldKey, Group, Input, Select, SelectOption};
    use urushi::{
        AnsiPolicy, Color, ColorProfile, ComponentRole, ComponentStyles, SemanticTokens,
        TerminalProfile, TextStyle, Theme,
    };
    pub(crate) fn lay_out(columns: u16, rows: u16, view: &PromptView) -> frame::FramedView {
        frame::frame(&resolve::resolve_prompt(columns, view), rows)
    }

    pub(crate) fn test_theme() -> Theme {
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

    pub(crate) fn renderer_view(lines: Vec<PromptLine>, cursor: Option<ViewCursor>) -> PromptView {
        PromptView { lines, cursor }
    }

    pub(crate) fn view_line(text: &str, style: &TextStyle) -> PromptLine {
        PromptLine::spans(vec![TextSpan::new(text, style.clone())])
    }

    /// A row belonging to the focused field, as `Form::view` marks them.
    pub(crate) fn active_line(mut line: PromptLine) -> PromptLine {
        line.active = true;
        line
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
            TextSpan::new("ab", monochrome.question.clone()),
            TextSpan::new("cd", monochrome.answer.clone()),
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
}
