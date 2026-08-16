//! Shared traversal and layout for hierarchical components.

use crate::{Align, TextStyle, VerticalAlign, View, visible_width};

/// The private recursive boundary shared by hierarchical component models.
pub(super) trait Traversable {
    fn value(&self) -> &str;
    fn visible_children(&self) -> Vec<&Self>;
}

#[derive(Debug, Clone)]
pub(super) struct TraversalStyles {
    pub(super) item: TextStyle,
    pub(super) enumerator: TextStyle,
    pub(super) indenter: TextStyle,
}

#[derive(Debug, Clone)]
struct PrefixPart {
    text: String,
    style: TextStyle,
}

pub(super) fn render<T, P>(
    rows: Vec<View>,
    children: &[&T],
    styles: &TraversalStyles,
    position: fn(usize, usize) -> P,
    enumerator: fn(P) -> String,
    indenter: fn(P) -> String,
) -> View
where
    T: Traversable,
    P: Copy,
{
    View::column(
        Align::Left,
        render_children(rows, children, &[], styles, position, enumerator, indenter),
    )
}

fn render_children<T, P>(
    mut rows: Vec<View>,
    children: &[&T],
    prefix: &[PrefixPart],
    styles: &TraversalStyles,
    position: fn(usize, usize) -> P,
    enumerator: fn(P) -> String,
    indenter: fn(P) -> String,
) -> Vec<View>
where
    T: Traversable,
    P: Copy,
{
    let markers = (0..children.len())
        .map(|index| {
            let position = position(index, children.len());
            (
                normalize_marker(enumerator(position)),
                normalize_marker(indenter(position)),
            )
        })
        .collect::<Vec<_>>();
    let segment_width = markers
        .iter()
        .flat_map(|(enumerator, indenter)| [visible_width(enumerator), visible_width(indenter)])
        .max()
        .unwrap_or(0);

    for (index, child) in children.iter().enumerate() {
        let (enum_marker, indent_marker) = &markers[index];
        let enum_text = align_right(enum_marker.clone(), segment_width);
        let indent_text = align_left(indent_marker.clone(), segment_width);
        let mut value_lines = child.value().split('\n');
        let first = value_lines.next().unwrap_or_default();
        let mut cells = prefix_cells(prefix);
        cells.push(View::text(enum_text, styles.enumerator.clone()));
        cells.push(View::text(first, styles.item.clone()));
        rows.push(View::row(VerticalAlign::Top, cells));

        for continuation in value_lines {
            let mut cells = prefix_cells(prefix);
            cells.push(View::text(indent_text.clone(), styles.indenter.clone()));
            cells.push(View::text(continuation, styles.item.clone()));
            rows.push(View::row(VerticalAlign::Top, cells));
        }

        let nested = child.visible_children();
        if !nested.is_empty() {
            let mut nested_prefix = prefix.to_vec();
            nested_prefix.push(PrefixPart {
                text: indent_text,
                style: styles.indenter.clone(),
            });
            rows = render_children(
                rows,
                &nested,
                &nested_prefix,
                styles,
                position,
                enumerator,
                indenter,
            );
        }
    }
    rows
}

fn prefix_cells(prefix: &[PrefixPart]) -> Vec<View> {
    prefix
        .iter()
        .map(|part| View::text(part.text.clone(), part.style.clone()))
        .collect()
}

fn align_right(text: String, width: usize) -> String {
    let padding = width.saturating_sub(visible_width(&text));
    format!("{}{text}", " ".repeat(padding))
}

fn align_left(mut text: String, width: usize) -> String {
    text.push_str(&" ".repeat(width.saturating_sub(visible_width(&text))));
    text
}

fn normalize_marker(marker: String) -> String {
    let mut output = String::with_capacity(marker.len());
    let mut characters = marker.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                output.push(' ');
            }
            '\n' | '\u{2028}' | '\u{2029}' => output.push(' '),
            other => output.push(other),
        }
    }
    output
}
