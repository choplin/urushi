//! Terminal-independent layout of a [`PromptView`] into drawable rows.
//!
//! Layout is a pure function of the view and the terminal box it must fit.
//! Keeping it free of renderer state lets the inline draw path be reasoned
//! about — and tested — without a terminal.

use urushi::{TextStyle, visible_width};

use super::{LineKind, PromptView, ViewCursor, ViewLine};

/// A view resolved against a concrete terminal box.
pub(crate) struct LaidOutView {
    pub lines: Vec<RenderedLine>,
    pub cursor: Option<ViewCursor>,
}

/// A drawable row: text and the styles it is emitted with, nothing else.
///
/// The classification layout selects rows by lives on [`Row`] and stops there.
/// A row is the unit the plan stage compares to decide whether to redraw, so
/// carrying policy metadata into it would make rows that look identical
/// compare unequal and redraw every frame.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RenderedLine {
    pub spans: Vec<RenderedSpan>,
}

impl RenderedLine {
    fn push(&mut self, style: &TextStyle, character: char) {
        if let Some(span) = self.spans.last_mut().filter(|span| &span.style == style) {
            span.text.push(character);
        } else {
            self.spans.push(RenderedSpan {
                text: character.to_string(),
                style: style.clone(),
            });
        }
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedSpan {
    pub text: String,
    pub style: TextStyle,
}

/// A laid-out row together with the source row's frame classification.
#[derive(Debug, Clone, Default)]
struct Row {
    line: RenderedLine,
    kind: LineKind,
    active: bool,
    /// A row produced by wrapping a longer line, other than its first.
    continued: bool,
}

impl Row {
    fn new(line: RenderedLine, source: &ViewLine) -> Self {
        Self {
            line,
            kind: source.kind,
            active: source.active,
            continued: false,
        }
    }

    fn is_choice(&self) -> bool {
        matches!(self.kind, LineKind::Choice { .. })
    }

    /// Whether this row is the one the viewport should keep visible.
    ///
    /// The head row of a selectable line nominates it. Before the view
    /// carried resolved styles this asked whether one rendered row held both
    /// the field marker and a choice-styled span, which silently answered no
    /// when a choice line wrapped and its labels landed on a later row. Row
    /// granularity replaces that accident with a stated rule.
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

/// Resolve `view` into the rows a `columns` x `rows` terminal box can show.
pub(crate) fn lay_out(columns: u16, rows: u16, view: &PromptView) -> LaidOutView {
    let width = usize::from(columns.max(1));
    let mut laid_out: Vec<Row> = Vec::new();
    let mut cursor = None;

    for (logical_row, line) in view.lines.iter().enumerate() {
        if view
            .cursor
            .is_some_and(|value| usize::from(value.row) == logical_row)
        {
            let view_cursor = view.cursor.expect("the cursor row was matched");
            let desired_offset = usize::from(view_cursor.column).saturating_sub(width - 1);
            let (clipped, offset) = clip_line(line, desired_offset, width);
            cursor = Some(ViewCursor {
                row: laid_out.len().min(usize::from(u16::MAX)) as u16,
                column: usize::from(view_cursor.column)
                    .saturating_sub(offset)
                    .min(width - 1) as u16,
            });
            laid_out.push(Row::new(clipped, line));
        } else if line.kind == LineKind::Help {
            laid_out.push(Row::new(clip_line(line, 0, width).0, line));
        } else {
            laid_out.extend(
                wrap_line(line, width)
                    .into_iter()
                    .enumerate()
                    .map(|(index, row)| Row {
                        continued: index > 0,
                        ..Row::new(row, line)
                    }),
            );
        }
    }

    if laid_out.is_empty() {
        laid_out.push(Row::default());
    }

    let max_rows = usize::from(rows.max(1));
    if laid_out.len() > max_rows {
        let active_start = laid_out.iter().position(|row| row.active).unwrap_or(0);
        let focus_row = cursor.map_or_else(
            || {
                laid_out
                    .iter()
                    .position(|row| row.is_focus(true))
                    .or_else(|| laid_out.iter().position(|row| row.is_focus(false)))
                    .unwrap_or(active_start)
            },
            |value| usize::from(value.row),
        );
        let mut start = active_start
            .saturating_sub(1)
            .min(laid_out.len() - max_rows);
        if focus_row < start {
            start = focus_row;
        } else if focus_row >= start + max_rows {
            start = focus_row + 1 - max_rows;
        }
        laid_out = laid_out.drain(start..start + max_rows).collect();
        cursor = cursor.and_then(|value| {
            let row = usize::from(value.row);
            (start..start + max_rows)
                .contains(&row)
                .then_some(ViewCursor {
                    row: (row - start) as u16,
                    column: value.column,
                })
        });

        if let Some(error) = first_error_line(view, width)
            && !laid_out.iter().any(|row| row.kind == LineKind::Error)
            && let Some(slot) = laid_out.iter().rposition(Row::is_spare)
        {
            laid_out[slot] = Row {
                line: error,
                kind: LineKind::Error,
                active: false,
                continued: false,
            };
        }
        if let Some(help) = first_help_line(view, width)
            && !laid_out.iter().any(|row| row.kind == LineKind::Help)
            && let Some(slot) = laid_out
                .iter()
                .rposition(|row| row.is_spare() && row.kind != LineKind::Error)
        {
            laid_out[slot] = Row {
                line: help,
                kind: LineKind::Help,
                active: false,
                continued: false,
            };
        }
    }

    LaidOutView {
        lines: laid_out.into_iter().map(|row| row.line).collect(),
        cursor,
    }
}

pub(crate) fn wrap_line(line: &ViewLine, width: usize) -> Vec<RenderedLine> {
    let mut lines = vec![RenderedLine::default()];
    let mut used = 0;
    for span in &line.spans {
        for character in span.text.chars() {
            if character == '\n' {
                lines.push(RenderedLine::default());
                used = 0;
                continue;
            }
            let character_width = visible_width(&character.to_string());
            if character_width > width {
                continue;
            }
            if used > 0 && used + character_width > width {
                lines.push(RenderedLine::default());
                used = 0;
            }
            lines
                .last_mut()
                .expect("a wrapped line always has a current row")
                .push(&span.style, character);
            used += character_width;
        }
    }
    lines
}

pub(crate) fn clip_line(line: &ViewLine, offset: usize, width: usize) -> (RenderedLine, usize) {
    let mut clipped = RenderedLine::default();
    let mut seen = 0;
    let mut actual_offset = None;
    let mut used = 0;
    for span in &line.spans {
        for character in span.text.chars() {
            let character_width = visible_width(&character.to_string());
            if seen + character_width <= offset {
                seen += character_width;
                continue;
            }
            if character_width > width {
                seen += character_width;
                continue;
            }
            let start = *actual_offset.get_or_insert(seen);
            if used > 0 && used + character_width > width {
                return (clipped, start);
            }
            clipped.push(&span.style, character);
            used += character_width;
            seen += character_width;
        }
    }
    (clipped, actual_offset.unwrap_or(seen))
}

/// The first error row, laid out the way an error row is normally drawn.
fn first_error_line(view: &PromptView, width: usize) -> Option<RenderedLine> {
    let line = view
        .lines
        .iter()
        .find(|line| line.kind == LineKind::Error)?;
    wrap_line(line, width).into_iter().next()
}

/// The first help row, laid out the way a help row is normally drawn.
fn first_help_line(view: &PromptView, width: usize) -> Option<RenderedLine> {
    let line = view.lines.iter().find(|line| line.kind == LineKind::Help)?;
    Some(clip_line(line, 0, width).0)
}
