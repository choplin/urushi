//! Stable renderer-neutral views for progress state.

use crate::{ComponentRole, ComponentStyles, TextStyle, VerticalAlign, View};

#[derive(Debug, Clone, Copy)]
pub(super) enum FinishKind {
    Success,
    Neutral,
    Error,
}

pub(super) fn work(styles: &ComponentStyles, marker: &str, message: &str) -> View {
    View::row(
        VerticalAlign::Top,
        [
            View::text(marker, styles.text_style(ComponentRole::Muted).clone()),
            View::text("  ", TextStyle::new()),
            View::text(message, styles.text_style(ComponentRole::Body).clone()),
        ],
    )
}

pub(super) fn progress(styles: &ComponentStyles, position: u64, total: u64, message: &str) -> View {
    View::row(
        VerticalAlign::Top,
        [
            View::text("…", styles.text_style(ComponentRole::Muted).clone()),
            View::text("  ", TextStyle::new()),
            View::text(format!("{position}/{total} "), TextStyle::new()),
            View::text(message, styles.text_style(ComponentRole::Body).clone()),
        ],
    )
}

pub(super) fn finished(styles: &ComponentStyles, kind: FinishKind, message: &str) -> View {
    let (marker, role) = match kind {
        FinishKind::Success => ("✓", ComponentRole::Success),
        FinishKind::Neutral => ("◇", ComponentRole::Muted),
        FinishKind::Error => ("×", ComponentRole::Error),
    };
    View::row(
        VerticalAlign::Top,
        [
            View::text(marker, styles.text_style(role).clone()),
            View::text("  ", TextStyle::new()),
            View::text(message, styles.text_style(ComponentRole::Body).clone()),
        ],
    )
}
