//! The Resolve stage: prompt lines sized against the prompt's drawing width by
//! the generic view model.
//!
//! Resolution itself belongs to [`urushi`] and knows nothing about prompts.
//! This module is only the call site: it hands each logical line the width the
//! form selected and collects the rectangles that come back.
//!
//! Lines resolve one at a time rather than as a single tree, because the Frame
//! stage selects rows by what the line they came from *is* — a choice, the
//! validation error, the help row, part of the focused field. Resolving the
//! prompt as one rectangle would flatten that classification away, and
//! recovering it afterwards would mean guessing which resolved row belongs to
//! which line.

use urushi::{Available, ResolvedView, resolve};

use super::{LineKind, PromptView, ViewCursor};

/// One logical line resolved against the prompt's available width, with the
/// classification the Frame stage selects rows by.
pub(crate) struct ResolvedLine {
    pub view: ResolvedView,
    pub kind: LineKind,
    /// Whether this line belongs to the field the form has focused.
    pub active: bool,
}

/// A whole prompt view resolved against the prompt's available width.
pub(crate) struct ResolvedPrompt {
    pub lines: Vec<ResolvedLine>,
    /// The cursor, still addressed by logical line. Frame maps it onto a row.
    pub cursor: Option<ViewCursor>,
}

/// Resolves every line of `view` into the rectangle `columns` cells allow.
pub(crate) fn resolve_prompt(columns: u16, view: &PromptView) -> ResolvedPrompt {
    let available = Available::columns(usize::from(columns.max(1)));
    ResolvedPrompt {
        lines: view
            .lines
            .iter()
            .map(|line| ResolvedLine {
                view: resolve(&line.view, available),
                kind: line.kind,
                active: line.active,
            })
            .collect(),
        cursor: view.cursor,
    }
}
