//! Shared traversal and layout for hierarchical components.

use crate::{Line, Style, View, visible_width};

/// The private recursive boundary shared by hierarchical component models.
pub(super) trait Traversable {
    fn value(&self) -> &str;
    fn visible_children(&self) -> Vec<&Self>;
}

#[derive(Debug, Clone)]
pub(super) struct TraversalStyles {
    pub(super) item: Style,
    pub(super) enumerator: Style,
    pub(super) indenter: Style,
}

#[derive(Debug, Clone)]
struct PrefixPart {
    text: String,
    style: Style,
}

pub(super) fn render<T, P>(
    view: View,
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
    render_children(view, children, &[], styles, position, enumerator, indenter)
}

fn render_children<T, P>(
    mut view: View,
    children: &[&T],
    prefix: &[PrefixPart],
    styles: &TraversalStyles,
    position: fn(usize, usize) -> P,
    enumerator: fn(P) -> String,
    indenter: fn(P) -> String,
) -> View
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
        let mut line = append_prefix(Line::new(), prefix)
            .span(enum_text, styles.enumerator.clone())
            .span(first, styles.item.clone());
        view = view.push(line);

        for continuation in value_lines {
            line = append_prefix(Line::new(), prefix)
                .span(indent_text.clone(), styles.indenter.clone())
                .span(continuation, styles.item.clone());
            view = view.push(line);
        }

        let nested = child.visible_children();
        if !nested.is_empty() {
            let mut nested_prefix = prefix.to_vec();
            nested_prefix.push(PrefixPart {
                text: indent_text,
                style: styles.indenter.clone(),
            });
            view = render_children(
                view,
                &nested,
                &nested_prefix,
                styles,
                position,
                enumerator,
                indenter,
            );
        }
    }
    view
}

fn append_prefix(mut line: Line, prefix: &[PrefixPart]) -> Line {
    for part in prefix {
        line = line.span(part.text.clone(), part.style.clone());
    }
    line
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
