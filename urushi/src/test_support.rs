//! Shared helpers for unit tests that inspect resolved views.

use crate::{Available, StyledGrapheme, StyledText, TextStyle, View, render_text, resolve};

/// Renders one style through the public non-layout text boundary.
pub(crate) fn render_style(style: &TextStyle, text: &str) -> String {
    render_text(
        &StyledText::new(text, style.clone()),
        &crate::RenderSettings::all(),
    )
}

/// Resolves `view` and returns its rows, exactly as laid out.
pub(crate) fn plain_rows(view: &View) -> Vec<String> {
    resolve(view, Available::NONE)
        .unwrap()
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
    resolve(view, Available::NONE).unwrap().rows()[row][column]
        .style()
        .clone()
}

/// Resolves one styled block and renders every output feature.
pub(crate) fn render_block(style: &crate::BlockStyle, content: &str) -> String {
    let view = View::block(style.clone(), View::text(content, style.text().clone()));
    let resolved = resolve(&view, Available::NONE).unwrap();
    crate::render(&resolved, &crate::RenderSettings::all())
}
