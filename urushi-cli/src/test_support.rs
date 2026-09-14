//! Shared helpers for unit tests that inspect resolved views.

use urushi::{Available, StyledGrapheme, TextStyle, View, resolve};

pub(crate) fn plain_at(view: &View, width: usize) -> String {
    resolve(view, Available::columns(width))
        .unwrap()
        .rows()
        .iter()
        .map(|row| {
            row.iter()
                .map(StyledGrapheme::symbol)
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn plain(view: &View) -> String {
    let width = urushi::measure(view).width();
    plain_at(view, width)
}

pub(crate) fn style_at(view: &View, row: usize, column: usize) -> TextStyle {
    let width = urushi::measure(view).width();
    style_at_width(view, width, row, column)
}

pub(crate) fn style_at_width(view: &View, width: usize, row: usize, column: usize) -> TextStyle {
    resolve(view, Available::columns(width)).unwrap().rows()[row][column]
        .style()
        .clone()
}
