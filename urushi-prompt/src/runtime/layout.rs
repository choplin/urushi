//! Terminal-independent layout of a [`PromptView`] into drawable rows.
//!
//! Layout is a pure function of the view and the terminal box it must fit.
//! Keeping it free of renderer state lets the inline draw path be reasoned
//! about — and tested — without a terminal.

use urushi::{ComponentRole, visible_width};

use super::{PromptView, ViewCursor, ViewLine};

/// A view resolved against a concrete terminal box.
pub(crate) struct LaidOutView {
    pub lines: Vec<RenderedLine>,
    pub cursor: Option<ViewCursor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RenderedLine {
    pub spans: Vec<RenderedSpan>,
}

impl RenderedLine {
    fn push(&mut self, role: ComponentRole, character: char) {
        if let Some(span) = self.spans.last_mut().filter(|span| span.role == role) {
            span.text.push(character);
        } else {
            self.spans.push(RenderedSpan {
                text: character.to_string(),
                role,
            });
        }
    }

    pub(crate) fn has_role(&self, role: ComponentRole) -> bool {
        self.spans.iter().any(|span| span.role == role)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedSpan {
    pub text: String,
    pub role: ComponentRole,
}

/// Resolve `view` into the rows a `columns` x `rows` terminal box can show.
pub(crate) fn lay_out(columns: u16, rows: u16, view: &PromptView) -> LaidOutView {
    let width = usize::from(columns.max(1));
    let mut lines = Vec::new();
    let mut cursor = None;

    for (logical_row, line) in view.lines.iter().enumerate() {
        if view
            .cursor
            .is_some_and(|value| usize::from(value.row) == logical_row)
        {
            let view_cursor = view.cursor.expect("the cursor row was matched");
            let desired_offset = usize::from(view_cursor.column).saturating_sub(width - 1);
            let (line, offset) = clip_line(line, desired_offset, width);
            cursor = Some(ViewCursor {
                row: lines.len().min(usize::from(u16::MAX)) as u16,
                column: usize::from(view_cursor.column)
                    .saturating_sub(offset)
                    .min(width - 1) as u16,
            });
            lines.push(line);
        } else if line
            .spans
            .iter()
            .any(|span| span.role == ComponentRole::PromptHelp)
        {
            lines.push(clip_line(line, 0, width).0);
        } else {
            lines.extend(wrap_line(line, width));
        }
    }

    if lines.is_empty() {
        lines.push(RenderedLine::default());
    }

    let max_rows = usize::from(rows.max(1));
    if lines.len() > max_rows {
        let active_start = lines
            .iter()
            .position(|line| line.has_role(ComponentRole::Accent))
            .unwrap_or(0);
        let focus_row = cursor.map_or_else(
            || {
                lines
                    .iter()
                    .position(|line| {
                        line.has_role(ComponentRole::Accent)
                            && (line.has_role(ComponentRole::PromptOptionSelected)
                                || line.has_role(ComponentRole::PromptButtonFocused))
                    })
                    .or_else(|| {
                        lines.iter().position(|line| {
                            line.has_role(ComponentRole::Accent)
                                && (line.has_role(ComponentRole::PromptOption)
                                    || line.has_role(ComponentRole::PromptButton))
                        })
                    })
                    .unwrap_or(active_start)
            },
            |value| usize::from(value.row),
        );
        let mut start = active_start.saturating_sub(1).min(lines.len() - max_rows);
        if focus_row < start {
            start = focus_row;
        } else if focus_row >= start + max_rows {
            start = focus_row + 1 - max_rows;
        }
        lines = lines.drain(start..start + max_rows).collect();
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
            && !lines
                .iter()
                .any(|line| line.has_role(ComponentRole::PromptError))
            && let Some(slot) = lines.iter().rposition(|line| {
                !line.has_role(ComponentRole::Accent)
                    && !line.has_role(ComponentRole::PromptCursor)
                    && !line.has_role(ComponentRole::PromptOptionSelected)
                    && !line.has_role(ComponentRole::PromptOption)
                    && !line.has_role(ComponentRole::PromptButtonFocused)
                    && !line.has_role(ComponentRole::PromptButton)
            })
        {
            lines[slot] = error;
        }
        if let Some(help) = first_help_line(view, width)
            && !lines
                .iter()
                .any(|line| line.has_role(ComponentRole::PromptHelp))
            && let Some(slot) = lines.iter().rposition(|line| {
                !line.has_role(ComponentRole::Accent)
                    && !line.has_role(ComponentRole::PromptCursor)
                    && !line.has_role(ComponentRole::PromptOptionSelected)
                    && !line.has_role(ComponentRole::PromptOption)
                    && !line.has_role(ComponentRole::PromptButtonFocused)
                    && !line.has_role(ComponentRole::PromptButton)
                    && !line.has_role(ComponentRole::PromptError)
            })
        {
            lines[slot] = help;
        }
    }

    LaidOutView { lines, cursor }
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
                .push(span.role, character);
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
            clipped.push(span.role, character);
            used += character_width;
            seen += character_width;
        }
    }
    (clipped, actual_offset.unwrap_or(seen))
}

fn first_error_line(view: &PromptView, width: usize) -> Option<RenderedLine> {
    view.lines
        .iter()
        .filter(|line| {
            line.spans
                .iter()
                .any(|span| span.role == ComponentRole::PromptError)
        })
        .flat_map(|line| wrap_line(line, width))
        .next()
}

fn first_help_line(view: &PromptView, width: usize) -> Option<RenderedLine> {
    view.lines
        .iter()
        .filter(|line| {
            line.spans
                .iter()
                .any(|span| span.role == ComponentRole::PromptHelp)
        })
        .map(|line| clip_line(line, 0, width).0)
        .next()
}
