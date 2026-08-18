//! The Frame stage: which of a resolved prompt's rows a bounded viewport
//! shows, and the canonical runs those rows are drawn from.
//!
//! Frame is where prompt policy lives. Resolve has already sized every line
//! against the terminal width, so nothing here reflows, clips, or measures
//! text: the stage chooses rows and nothing else. Its two policies are the
//! ones a prompt stops being usable without — keep the row the user is acting
//! on visible, and do not let a short viewport swallow the validation error or
//! the help line.
//!
//! It is a pure function of its inputs. No terminal, no writer, no styles to
//! resolve.

use urushi::{StyledGrapheme, TextStyle};

use super::{
    LineKind, ViewCursor,
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
    /// the terminal profile, so equal values are equal appearance.
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
    /// A row produced by wrapping a longer line, other than its first.
    continued: bool,
}

impl Row {
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
        if let Some(value) = resolved
            .cursor
            .filter(|value| usize::from(value.row) == logical_row)
        {
            cursor = Some(ViewCursor {
                row: framed.len().min(usize::from(u16::MAX)) as u16,
                column: value.column,
            });
        }
        framed.extend(line.view.rows().iter().enumerate().map(|(index, row)| Row {
            row: FramedRow::aggregate(row),
            kind: line.kind,
            active: line.active,
            continued: index > 0,
        }));
    }

    if framed.is_empty() {
        framed.push(Row::default());
    }

    let max_rows = usize::from(rows.max(1));
    if framed.len() > max_rows {
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
                continued: false,
            };
        }
    }

    FramedView {
        rows: framed.into_iter().map(|row| row.row).collect(),
        cursor,
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
        PromptLine, PromptView, ViewSpan, fixed_view, resolve::resolve_prompt, test_styles,
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
            &view(vec![PromptLine::spans(vec![ViewSpan::new("abcd", &style)])]),
        );
        let split = framed(
            10,
            1,
            &view(vec![PromptLine::spans(vec![
                ViewSpan::new("", &style),
                ViewSpan::new("ab", &style),
                ViewSpan::new("cd", &style),
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
            &view(vec![PromptLine::spans(vec![ViewSpan::new("名前", &style)])]),
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
                ViewSpan::new("ab", &style),
                ViewSpan::new("cd", &style),
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
                fixed_view(2, vec![ViewSpan::new("› ", &styles.option_selected)]),
                View::text("English", styles.option_selected.clone()),
            ],
        ))
        .with_kind(LineKind::Choice { focused: true });
        choice.active = true;
        let mut question = PromptLine::spans(vec![ViewSpan::new("Language", &styles.accent)]);
        question.active = true;

        let framed = framed(
            40,
            4,
            &view(vec![
                PromptLine::spans(vec![ViewSpan::new("Earlier answer", &styles.muted)]),
                question,
                PromptLine::spans(vec![ViewSpan::new("  Japanese", &styles.option)])
                    .with_kind(LineKind::Choice { focused: false }),
                choice,
                PromptLine::spans(vec![ViewSpan::new("↑/↓ select", &styles.help)])
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
