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
                        let bottom = anchor.y().saturating_add(
                            i64::try_from(anchor.height().max(1)).unwrap_or(i64::MAX),
                        );
                        Some(ResolvedRegion {
                            kind: region.kind,
                            start: nonnegative_index(anchor.y()),
                            end: nonnegative_index(bottom),
                        })
                    })
                    .collect();
                let cursor = line.cursor.and_then(|key| {
                    let anchor = resolved.anchor(key)?;
                    Some(ViewCursor {
                        row: terminal_coordinate(anchor.y()),
                        column: terminal_coordinate(anchor.x()),
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

fn nonnegative_index(value: i64) -> usize {
    usize::try_from(value).unwrap_or(if value < 0 { 0 } else { usize::MAX })
}

fn terminal_coordinate(value: i64) -> u16 {
    value.clamp(0, i64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use urushi::{
        BlockStyle, Canvas, CanvasContext, CanvasItem, Length, Position, Size, TextStyle, View,
    };

    use super::*;
    use crate::runtime::{FieldRegion, PromptLine};

    #[derive(Debug, Clone, PartialEq)]
    struct PartlyVisibleAnchor;

    impl CanvasItem for PartlyVisibleAnchor {
        fn draw(&self, context: &mut CanvasContext) {
            context.view(
                Position::new(-2, -1),
                View::anchor_block(
                    "region",
                    BlockStyle::new()
                        .width(Length::Cells(4))
                        .height(Length::Cells(3)),
                    View::text("x", TextStyle::new()),
                ),
                None,
                None,
            );
        }
    }

    #[test]
    fn signed_anchors_are_projected_into_the_nonnegative_prompt_domain() {
        let key = urushi::Key::from("region");
        let view = PromptView {
            lines: vec![PromptLine {
                view: View::canvas(
                    Canvas::new()
                        .extent(Size::new(4, 3))
                        .item(PartlyVisibleAnchor),
                ),
                kind: LineKind::Content,
                active: true,
                regions: vec![FieldRegion {
                    kind: FieldRegionKind::Focus,
                    key,
                }],
                cursor: Some(key),
                field: true,
            }],
            cursor: None,
        };

        let resolved = resolve_prompt(4, &view);
        assert_eq!(
            (
                resolved.lines[0].regions[0].start,
                resolved.lines[0].regions[0].end
            ),
            (0, 2)
        );
        assert_eq!(
            resolved.lines[0].cursor,
            Some(ViewCursor { row: 0, column: 0 })
        );
    }
}
