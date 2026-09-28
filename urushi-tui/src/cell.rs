//! Shared traversal of resolved cell rectangles.

use urushi::{ResolvedView, StyledGrapheme};

/// Visits every leading grapheme cell in row-major order.
///
/// Backend-specific coordinates and clipping stay with the caller. Keeping the
/// logical traversal here makes plain widgets and the runtime renderer consume
/// a [`ResolvedView`] identically.
pub(crate) fn visit_resolved(
    resolved: &ResolvedView,
    mut visit: impl FnMut(usize, usize, &StyledGrapheme),
) {
    for (row, graphemes) in resolved.rows().iter().enumerate() {
        let mut column = 0;
        for grapheme in graphemes {
            visit(column, row, grapheme);
            column = column.saturating_add(grapheme.width());
        }
    }
}
