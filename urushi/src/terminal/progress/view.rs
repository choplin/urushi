//! Stable renderer-neutral views for progress state.

use crate::{ComponentRole, ComponentStyles, Line, Style, View};

#[derive(Debug, Clone, Copy)]
pub(super) enum FinishKind {
    Success,
    Neutral,
    Error,
}

pub(super) fn work(styles: &ComponentStyles, marker: &str, message: &str) -> View {
    View::line(
        Line::new()
            .span(marker, styles.style(ComponentRole::Muted).clone())
            .span("  ", Style::new())
            .span(message, styles.style(ComponentRole::Body).clone()),
    )
}

pub(super) fn progress(styles: &ComponentStyles, position: u64, total: u64, message: &str) -> View {
    View::line(
        Line::new()
            .span("…", styles.style(ComponentRole::Muted).clone())
            .span("  ", Style::new())
            .span(format!("{position}/{total} "), Style::new())
            .span(message, styles.style(ComponentRole::Body).clone()),
    )
}

pub(super) fn finished(styles: &ComponentStyles, kind: FinishKind, message: &str) -> View {
    let (marker, role) = match kind {
        FinishKind::Success => ("✓", ComponentRole::Success),
        FinishKind::Neutral => ("◇", ComponentRole::Muted),
        FinishKind::Error => ("×", ComponentRole::Error),
    };
    View::line(
        Line::new()
            .span(marker, styles.style(role).clone())
            .span("  ", Style::new())
            .span(message, styles.style(ComponentRole::Body).clone()),
    )
}
