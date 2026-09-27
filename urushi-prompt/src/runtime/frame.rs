//! The Frame stage: which of a resolved prompt's rows a bounded viewport
//! shows, and the canonical runs those rows are drawn from.
//!
//! Frame is where prompt policy lives. Resolve has already sized every field
//! body against the terminal width, so nothing here reflows, clips, or
//! measures text. Whole inactive fields and optional description/help content
//! yield before the active question, validation error, and focus row.
//! Anchor-derived region ranges provide those semantics without exposing them
//! to the generic view tree.
//!
//! It is a pure function of its inputs. No terminal, no writer, no styles to
//! resolve.

use std::collections::BTreeSet;

use urushi::{StyledGrapheme, TextStyle};

use super::{
    FieldRegionKind, LineKind, ViewCursor,
    resolve::{ResolvedLine, ResolvedPrompt},
};

/// A run of equally styled text, and the cells it occupies.
///
/// The width travels with the run because the plan stage must know how far a
/// write advances the cursor, and because rows are compared for equality on
/// the redraw path. It is measured once, during Resolve, and never recomputed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledRun {
    pub text: String,
    pub display_width: usize,
    pub style: TextStyle,
}

/// A drawable row: the runs it is written from, nothing else.
///
/// The classification Frame selected the row by stays in this module. A row is
/// the unit the plan stage compares to decide whether to redraw, so carrying
/// policy metadata into it would make rows that look identical compare unequal
/// and redraw every frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct FramedRow {
    pub runs: Vec<StyledRun>,
}

impl FramedRow {
    /// Aggregates a resolved row into canonical runs.
    ///
    /// The canonical form is what makes row equality mean "looks the same":
    /// adjacent graphemes of equal style merge greedily from the left, and no
    /// empty run is emitted. Styles are already resolved against the theme and
    /// selected render settings, so equal values are equal appearance.
    pub(crate) fn aggregate(row: &[StyledGrapheme]) -> Self {
        let mut runs: Vec<StyledRun> = Vec::new();
        for grapheme in row {
            match runs.last_mut() {
                Some(run) if &run.style == grapheme.style() => {
                    run.text.push_str(grapheme.symbol());
                    run.display_width += grapheme.width();
                }
                _ => runs.push(StyledRun {
                    text: grapheme.symbol().to_owned(),
                    display_width: grapheme.width(),
                    style: grapheme.style().clone(),
                }),
            }
        }
        runs.retain(|run| !run.text.is_empty());
        Self { runs }
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }

    #[cfg(test)]
    pub(crate) fn runs(&self) -> &[StyledRun] {
        &self.runs
    }
}

/// The rows a terminal box shows, and where the cursor sits among them.
pub(crate) struct FramedView {
    pub rows: Vec<FramedRow>,
    pub cursor: Option<ViewCursor>,
}

/// A framed row together with the source line's classification.
#[derive(Debug, Clone, Default)]
struct Row {
    row: FramedRow,
    kind: LineKind,
    active: bool,
    field: bool,
    logical_line: usize,
    regions: Vec<FieldRegionKind>,
    /// A row produced by wrapping a longer line, other than its first.
    continued: bool,
}

impl Row {
    fn has_region(&self, kind: FieldRegionKind) -> bool {
        self.regions.contains(&kind)
    }

    fn is_choice(&self) -> bool {
        matches!(self.kind, LineKind::Choice { .. })
    }

    /// Whether this row is the one the viewport should keep visible.
    ///
    /// The head row of a selectable line nominates it. Row granularity is the
    /// stated rule: asking instead whether a row holds both a field marker and
    /// a choice-styled span silently answered no when a choice line wrapped
    /// and its labels landed on a later row.
    fn is_focus(&self, focused_choice_only: bool) -> bool {
        self.active
            && !self.continued
            && if focused_choice_only {
                self.kind == LineKind::Choice { focused: true }
            } else {
                self.is_choice()
            }
    }

    /// Whether the error or help row may be reinstated over this row.
    ///
    /// A selectable row and the head row of the focused field carry the state
    /// the user is acting on, so they are never displaced. A wrapped
    /// continuation is displaceable: the row it continues stays on screen, and
    /// dropping the validation error or the help line entirely is the worse
    /// outcome in a viewport this short.
    fn is_spare(&self) -> bool {
        !(self.active && !self.continued) && !self.is_choice()
    }
}

/// Choose the rows a viewport `rows` tall shows of `resolved`.
pub(crate) fn frame(resolved: &ResolvedPrompt, rows: u16) -> FramedView {
    let mut framed: Vec<Row> = Vec::new();
    let mut cursor = None;

    for (logical_row, line) in resolved.lines.iter().enumerate() {
        let row_offset = framed.len();
        if let Some(value) = resolved
            .cursor
            .filter(|value| usize::from(value.row) == logical_row)
        {
            cursor = Some(ViewCursor {
                row: framed.len().min(usize::from(u16::MAX)) as u16,
                column: value.column,
            });
        }
        if let Some(value) = line.cursor {
            cursor = Some(ViewCursor {
                row: row_offset
                    .saturating_add(usize::from(value.row))
                    .min(usize::from(u16::MAX)) as u16,
                column: value.column,
            });
        }
        framed.extend(line.view.rows().iter().enumerate().map(|(index, row)| {
            Row {
                row: FramedRow::aggregate(row),
                kind: line.kind,
                active: line.active,
                field: line.field,
                logical_line: logical_row,
                regions: line
                    .regions
                    .iter()
                    .filter(|region| (region.start..region.end).contains(&index))
                    .map(|region| region.kind)
                    .collect(),
                continued: index > 0,
            }
        }));
    }

    if framed.is_empty() {
        framed.push(Row::default());
    }

    let max_rows = usize::from(rows.max(1));
    if framed.len() > max_rows {
        if framed.iter().any(|row| row.field) {
            return frame_presentations(framed, cursor, max_rows);
        }
        let active_start = framed.iter().position(|row| row.active).unwrap_or(0);
        let focus_row = cursor.map_or_else(
            || {
                framed
                    .iter()
                    .position(|row| row.is_focus(true))
                    .or_else(|| framed.iter().position(|row| row.is_focus(false)))
                    .unwrap_or(active_start)
            },
            |value| usize::from(value.row),
        );
        let mut start = active_start.saturating_sub(1).min(framed.len() - max_rows);
        if focus_row < start {
            start = focus_row;
        } else if focus_row >= start + max_rows {
            start = focus_row + 1 - max_rows;
        }
        framed = framed.drain(start..start + max_rows).collect();
        cursor = cursor.and_then(|value| {
            let row = usize::from(value.row);
            (start..start + max_rows)
                .contains(&row)
                .then_some(ViewCursor {
                    row: (row - start) as u16,
                    column: value.column,
                })
        });

        if let Some(error) = first_row_of(&resolved.lines, LineKind::Error)
            && !framed.iter().any(|row| row.kind == LineKind::Error)
            && let Some(slot) = framed.iter().rposition(Row::is_spare)
        {
            framed[slot] = Row {
                row: error,
                kind: LineKind::Error,
                active: false,
                field: false,
                logical_line: usize::MAX,
                regions: Vec::new(),
                continued: false,
            };
        }
        if let Some(help) = first_row_of(&resolved.lines, LineKind::Help)
            && !framed.iter().any(|row| row.kind == LineKind::Help)
            && let Some(slot) = framed
                .iter()
                .rposition(|row| row.is_spare() && row.kind != LineKind::Error)
        {
            framed[slot] = Row {
                row: help,
                kind: LineKind::Help,
                active: false,
                field: false,
                logical_line: usize::MAX,
                regions: Vec::new(),
                continued: false,
            };
        }
    }

    FramedView {
        rows: framed.into_iter().map(|row| row.row).collect(),
        cursor,
    }
}

fn frame_presentations(rows: Vec<Row>, cursor: Option<ViewCursor>, max_rows: usize) -> FramedView {
    let active_line = rows
        .iter()
        .find(|row| row.field && row.active)
        .map(|row| row.logical_line);
    let Some(active_line) = active_line else {
        return FramedView {
            rows: rows.into_iter().take(max_rows).map(|row| row.row).collect(),
            cursor: None,
        };
    };
    let active: Vec<usize> = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.logical_line == active_line).then_some(index))
        .collect();
    let focus = cursor
        .map(|value| usize::from(value.row))
        .filter(|index| active.contains(index))
        .or_else(|| {
            active
                .iter()
                .copied()
                .find(|index| rows[*index].has_region(FieldRegionKind::Focus))
        })
        .or_else(|| {
            active
                .iter()
                .copied()
                .find(|index| rows[*index].has_region(FieldRegionKind::Control))
        })
        .unwrap_or(active[0]);

    let region = |kind| {
        active
            .iter()
            .copied()
            .filter(|index| rows[*index].has_region(kind))
            .collect::<Vec<_>>()
    };
    let questions = region(FieldRegionKind::Question);
    let errors = region(FieldRegionKind::Error);
    let controls = region(FieldRegionKind::Control);
    let descriptions = region(FieldRegionKind::Description);
    let mut selected = BTreeSet::from([focus]);

    if errors.is_empty() {
        insert_complete(&mut selected, &questions, max_rows)
    } else {
        if !insert_complete(&mut selected, &errors, max_rows)
            && selected.len() < max_rows
            && let Some(error) = errors.first()
        {
            selected.insert(*error);
        }
        insert_complete(&mut selected, &questions, max_rows)
    };

    let mut nearby_controls = controls;
    nearby_controls.sort_by_key(|index| (index.abs_diff(focus), *index));
    insert_until_full(&mut selected, nearby_controls, max_rows);

    let required: BTreeSet<usize> = questions
        .iter()
        .chain(&errors)
        .chain(&descriptions)
        .copied()
        .collect();
    insert_until_full(&mut selected, descriptions, max_rows);

    let help = rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| (row.kind == LineKind::Help).then_some(index));
    insert_until_full(&mut selected, help, max_rows);

    let mut other_fields: Vec<(usize, Vec<usize>)> = rows
        .iter()
        .filter(|row| row.field && row.logical_line != active_line)
        .map(|row| row.logical_line)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|line| {
            let indices = rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| (row.logical_line == line).then_some(index))
                .collect();
            (line.abs_diff(active_line), indices)
        })
        .collect();
    other_fields.sort_by_key(|(distance, indices)| (*distance, indices[0]));
    for (_, field) in other_fields {
        insert_complete(&mut selected, &field, max_rows);
    }

    insert_until_full(
        &mut selected,
        active
            .iter()
            .copied()
            .filter(|index| !required.contains(index)),
        max_rows,
    );

    insert_until_full(
        &mut selected,
        rows.iter()
            .enumerate()
            .filter_map(|(index, row)| (!row.field).then_some(index)),
        max_rows,
    );

    selected_rows(rows, cursor, selected)
}

fn selected_rows(
    rows: Vec<Row>,
    cursor: Option<ViewCursor>,
    selected: BTreeSet<usize>,
) -> FramedView {
    let selected: Vec<usize> = selected.into_iter().collect();
    let framed_cursor = cursor.and_then(|value| {
        let source = usize::from(value.row);
        selected
            .iter()
            .position(|index| *index == source)
            .map(|row| ViewCursor {
                row: row.min(usize::from(u16::MAX)) as u16,
                column: value.column,
            })
    });
    FramedView {
        rows: selected
            .into_iter()
            .map(|index| rows[index].row.clone())
            .collect(),
        cursor: framed_cursor,
    }
}

fn insert_complete(selected: &mut BTreeSet<usize>, rows: &[usize], max_rows: usize) -> bool {
    let missing = rows
        .iter()
        .filter(|index| !selected.contains(index))
        .count();
    if selected.len().saturating_add(missing) > max_rows {
        return false;
    }
    selected.extend(rows.iter().copied());
    true
}

fn insert_until_full(
    selected: &mut BTreeSet<usize>,
    rows: impl IntoIterator<Item = usize>,
    max_rows: usize,
) {
    for row in rows {
        if selected.len() == max_rows {
            break;
        }
        selected.insert(row);
    }
}

/// The first row of the first line classified `kind`, as it was resolved.
fn first_row_of(lines: &[ResolvedLine], kind: LineKind) -> Option<FramedRow> {
    lines
        .iter()
        .find(|line| line.kind == kind)?
        .view
        .rows()
        .first()
        .map(|row| FramedRow::aggregate(row))
}

#[cfg(test)]
mod tests {
    use urushi::{TextStyle, VerticalAlign, View};

    use super::*;
    use crate::runtime::{
        PromptLine, PromptView, TextSpan, fixed_view, resolve::resolve_prompt, test_styles,
    };

    /// Framing takes a resolved view and a row count. There is no terminal, no
    /// writer, and no style left to resolve anywhere in this file.
    fn framed(columns: u16, rows: u16, view: &PromptView) -> FramedView {
        frame(&resolve_prompt(columns, view), rows)
    }

    fn view(lines: Vec<PromptLine>) -> PromptView {
        PromptView {
            lines,
            cursor: None,
        }
    }

    #[test]
    fn a_row_has_one_canonical_run_sequence_however_it_was_composed() {
        // Rows are compared to decide whether to redraw, so the same visible
        // row must always aggregate to the same runs. Otherwise a prompt
        // redraws every frame for no visible reason.
        let style = TextStyle::new().bold();
        let whole = framed(
            10,
            1,
            &view(vec![PromptLine::spans(vec![TextSpan::new(
                "abcd",
                style.clone(),
            )])]),
        );
        let split = framed(
            10,
            1,
            &view(vec![PromptLine::spans(vec![
                TextSpan::new("", style.clone()),
                TextSpan::new("ab", style.clone()),
                TextSpan::new("cd", style.clone()),
            ])]),
        );

        assert_eq!(whole.rows, split.rows);
        assert_eq!(split.rows[0].runs.len(), 1);
        assert!(split.rows[0].runs.iter().all(|run| !run.text.is_empty()));
        assert_eq!(split.rows[0].runs[0].display_width, 4);
    }

    #[test]
    fn a_run_carries_the_cells_it_occupies_not_the_characters_it_holds() {
        let style = TextStyle::new().bold();
        let framed = framed(
            10,
            1,
            &view(vec![PromptLine::spans(vec![TextSpan::new(
                "名前",
                style.clone(),
            )])]),
        );

        assert_eq!(framed.rows[0].runs[0].text, "名前");
        assert_eq!(framed.rows[0].runs[0].display_width, 4);
    }

    #[test]
    fn a_style_the_profile_erased_merges_the_runs_it_separated() {
        let style = TextStyle::new().bold();
        let framed = framed(
            10,
            1,
            &view(vec![PromptLine::spans(vec![
                TextSpan::new("ab", style.clone()),
                TextSpan::new("cd", style.clone()),
            ])]),
        );

        assert_eq!(framed.rows[0].runs.len(), 1);
        assert_eq!(framed.rows[0].text(), "abcd");
    }

    #[test]
    fn a_short_viewport_follows_the_focused_choice_and_reinstates_the_help_row() {
        let styles = test_styles();
        let mut choice = PromptLine::new(View::row(
            VerticalAlign::Top,
            [
                fixed_view(2, vec![TextSpan::new("› ", styles.option_selected.clone())]),
                View::text("English", styles.option_selected.clone()),
            ],
        ))
        .with_kind(LineKind::Choice { focused: true });
        choice.active = true;
        let mut question =
            PromptLine::spans(vec![TextSpan::new("Language", styles.accent.clone())]);
        question.active = true;

        let framed = framed(
            40,
            4,
            &view(vec![
                PromptLine::spans(vec![TextSpan::new("Earlier answer", styles.muted.clone())]),
                question,
                PromptLine::spans(vec![TextSpan::new("  Japanese", styles.option.clone())])
                    .with_kind(LineKind::Choice { focused: false }),
                choice,
                PromptLine::spans(vec![TextSpan::new("↑/↓ select", styles.help.clone())])
                    .with_kind(LineKind::Help),
            ]),
        );

        let drawn: Vec<String> = framed.rows.iter().map(FramedRow::text).collect();
        assert_eq!(framed.rows.len(), 4);
        assert!(
            drawn.iter().any(|row| row.contains("English")),
            "the focused choice was scrolled away: {drawn:?}"
        );
        // The help row was scrolled out, so it is reinstated over the one row
        // that carries neither a choice nor the focused field's own state.
        assert!(
            drawn.iter().any(|row| row.contains("select")),
            "the help row was not reinstated: {drawn:?}"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::runtime::{
        FormState, LineKind, PromptLine, PromptStyles, ReducerResult, TextSpan, ViewCursor, frame,
        terminal::tests::enter, terminal_backend::TerminalRenderer, test_styles, view::tests::*,
    };
    use crate::{FieldKey, Form, Group, Input};
    use urushi::RenderSettings;
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
    fn narrow_viewports_omit_wide_scalars_without_losing_cursor_bounds() {
        let theme = test_theme();
        let settings = ansi_settings();
        let styles = PromptStyles::resolve(&theme, &settings);
        let cjk_line = view_line("あ", &styles.cursor);
        for columns in [0, 1] {
            let mut renderer = TerminalRenderer::new(Vec::new(), (columns, 1));
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
            let output = String::from_utf8(renderer.writer.into_inner())
                .expect("renderer writes UTF-8 commands");
            assert!(!output.contains('あ'));
        }
    }

    #[test]
    fn short_viewports_never_replace_the_active_field_with_help() {
        let theme = test_theme();
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        for rows in 1..=4 {
            let renderer = TerminalRenderer::new(Vec::new(), (40, rows));
            let view = renderer_view(
                vec![
                    view_line("previous question", &styles.muted),
                    view_line("previous answer", &styles.answer),
                    view_line("", &styles.body),
                    active_line(view_line("┃ current question", &styles.accent)),
                    active_line(PromptLine::spans(vec![
                        TextSpan::new("┃ ", styles.accent.clone()),
                        TextSpan::new("› current answer", styles.cursor.clone()),
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
        let settings = RenderSettings::default();
        let styles = PromptStyles::resolve(&theme, &settings);
        let renderer = TerminalRenderer::new(Vec::new(), (40, 1));
        let view = renderer_view(
            vec![
                active_line(view_line("┃ choose a language", &styles.accent)),
                active_line(
                    view_line("┃   Japanese", &styles.option)
                        .with_kind(LineKind::Choice { focused: false }),
                ),
                active_line(
                    PromptLine::spans(vec![
                        TextSpan::new("┃ ", styles.accent.clone()),
                        TextSpan::new("› English", styles.option_selected.clone()),
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
                        TextSpan::new("┃ ", styles.accent.clone()),
                        TextSpan::new("  Yes  ", styles.button_focused.clone()),
                        TextSpan::new("   No  ", styles.button.clone()),
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
}
