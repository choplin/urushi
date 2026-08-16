//! Shared helpers for unit tests that inspect resolved views.

use crate::{Available, StyledGrapheme, View, resolve};

/// Resolves `view` and returns its rows, exactly as laid out.
pub(crate) fn plain_rows(view: &View) -> Vec<String> {
    resolve(view, Available::NONE)
        .rows()
        .iter()
        .map(|row| row.iter().map(StyledGrapheme::symbol).collect())
        .collect()
}

/// Resolves `view` and returns its rows joined, exactly as laid out.
pub(crate) fn plain_exact(view: &View) -> String {
    plain_rows(view).join("\n")
}

/// Resolves `view` and returns its rows without the alignment padding a
/// rectangle adds on the right, so an assertion can state the content alone.
pub(crate) fn plain(view: &View) -> String {
    plain_rows(view)
        .iter()
        .map(|row| row.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolves `view` and returns the logical style of one grapheme.
pub(crate) fn style_at(view: &View, row: usize, column: usize) -> crate::TextStyle {
    resolve(view, Available::NONE).rows()[row][column]
        .style()
        .clone()
}
