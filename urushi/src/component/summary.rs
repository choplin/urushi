//! Titled, aligned result summaries.

use crate::{
    Align, ComponentRole, ComponentStyles, VerticalAlign, View,
    text::{PrintableLines, PrintableText, wrap_text},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryField {
    label: String,
    value: String,
}

impl SummaryField {
    /// Creates one labelled field.
    ///
    /// `label` and `value` are plain text. Escape sequences and cursor movement in it break that contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Adopt
    /// already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    ///
    /// Style the component through its [`ComponentStyles`](crate::ComponentStyles)
    /// rather than by pre-rendering its content.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    title: String,
    fields: Vec<SummaryField>,
}

impl Summary {
    /// `title` is plain text: escape sequences and cursor movement in it break
    /// that contract, and debug builds panic on them.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: Vec::new(),
        }
    }

    /// Appends one labelled field, whose label and value are plain text.
    #[must_use]
    pub fn field(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.fields.push(SummaryField::new(label, value));
        self
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn fields(&self) -> &[SummaryField] {
        &self.fields
    }

    pub fn view(&self, styles: &ComponentStyles, width: usize) -> View {
        let width = width.max(10);
        let natural_label_width = self
            .fields
            .iter()
            .map(|field| PrintableText::new(field.label()).width())
            .max()
            .unwrap_or(0);
        let label_width = natural_label_width.min(width.saturating_sub(8) / 2);
        let muted = styles.text_style(ComponentRole::Muted).clone();
        let accent = styles.text_style(ComponentRole::Accent).clone();
        let body = styles.text_style(ComponentRole::Body).clone();
        let mut rows = vec![
            View::text("│", muted.clone()),
            View::row(
                VerticalAlign::Top,
                [
                    View::text("◇", accent.clone()),
                    View::text("  ", muted.clone()),
                    View::text(self.title.clone(), accent),
                ],
            ),
        ];
        for field in &self.fields {
            let value_width = width.saturating_sub(label_width + 5).max(1);
            for (index, value) in wrap_text(PrintableLines::new(field.value()), value_width)
                .iter()
                .enumerate()
            {
                let label = if index == 0 { field.label() } else { "" };
                let padding =
                    " ".repeat(label_width.saturating_sub(PrintableText::new(label).width()));
                rows.push(View::row(
                    VerticalAlign::Top,
                    [
                        View::text("│", muted.clone()),
                        View::text("  ", muted.clone()),
                        View::text(label, muted.clone()),
                        View::text(padding, muted.clone()),
                        View::text("  ", muted.clone()),
                        View::text(value, body.clone()),
                    ],
                ));
            }
        }
        View::column(Align::Left, rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::plain_rows;
    use crate::{Color, SemanticTokens, Theme, measure};

    #[test]
    fn wraps_cjk_values_and_aligns_continuations() {
        let theme = Theme::from_tokens(SemanticTokens {
            text: Color::Ansi(7),
            text_muted: Color::Ansi(8),
            background: Color::Ansi(0),
            surface: Color::Ansi(0),
            accent: Color::Ansi(6),
            accent_text: Color::Ansi(0),
            success: Color::Ansi(2),
            warning: Color::Ansi(3),
            error: Color::Ansi(1),
            border: Color::Ansi(8),
        });
        let view = Summary::new("Result")
            .field("Name", "日本語日本語")
            .view(theme.components(), 12);
        let rows = plain_rows(&view);
        assert!(measure(&view).height() > 3);
        assert_eq!(rows[2].trim_end(), "│  Name  日本");
        assert_eq!(
            rows[3].trim_end(),
            "│      語日",
            "a wrapped continuation keeps the label column blank"
        );
    }
}
