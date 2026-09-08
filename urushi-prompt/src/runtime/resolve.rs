//! The Resolve stage: prompt context and field bodies sized against the
//! drawing width by the generic view model.
//!
//! Resolution itself belongs to [`urushi`] and knows nothing about prompts.
//! This module is only the call site. It resolves each complete field body
//! once, then translates the body's anchors into prompt-owned row ranges for
//! Frame. Generic resolution remains unaware of field semantics.

use urushi::{Available, ResolvedView, resolve};

use super::{FieldRegionKind, LineKind, PromptView, ViewCursor};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedRegion {
    pub kind: FieldRegionKind,
    pub start: usize,
    pub end: usize,
}

/// One context entry or complete field body resolved against the prompt width.
pub(crate) struct ResolvedLine {
    pub view: ResolvedView,
    pub kind: LineKind,
    /// Whether this line belongs to the field the form has focused.
    pub active: bool,
    pub field: bool,
    pub regions: Vec<ResolvedRegion>,
    pub cursor: Option<ViewCursor>,
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
            .map(|line| {
                let resolved = resolve(&line.view, available);
                let regions = line
                    .regions
                    .iter()
                    .filter_map(|region| {
                        let anchor = resolved.anchor(region.key)?;
                        Some(ResolvedRegion {
                            kind: region.kind,
                            start: anchor.y(),
                            end: anchor.y().saturating_add(anchor.height().max(1)),
                        })
                    })
                    .collect();
                let cursor = line.cursor.and_then(|key| {
                    let anchor = resolved.anchor(key)?;
                    Some(ViewCursor {
                        row: anchor.y().min(usize::from(u16::MAX)) as u16,
                        column: anchor.x().min(usize::from(u16::MAX)) as u16,
                    })
                });
                ResolvedLine {
                    view: resolved,
                    kind: line.kind,
                    active: line.active,
                    field: line.field,
                    regions,
                    cursor,
                }
            })
            .collect(),
        cursor: view.cursor,
    }
}
