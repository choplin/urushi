//! Titled, aligned result summaries.

use crate::{ComponentRole, ComponentStyles, Line, View, text::wrap_text, visible_width};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryField {
    label: String,
    value: String,
}

impl SummaryField {
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
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            fields: Vec::new(),
        }
    }

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
            .map(|field| visible_width(field.label()))
            .max()
            .unwrap_or(0);
        let label_width = natural_label_width.min(width.saturating_sub(8) / 2);
        let muted = styles.style(ComponentRole::Muted).clone();
        let accent = styles.style(ComponentRole::Accent).clone();
        let body = styles.style(ComponentRole::Body).clone();
        let mut view = View::line(Line::styled("│", muted.clone())).push(
            Line::new()
                .span("◇", accent.clone())
                .span("  ", muted.clone())
                .span(self.title.clone(), accent),
        );
        for field in &self.fields {
            let value_width = width.saturating_sub(label_width + 5).max(1);
            for (index, value) in wrap_text(field.value(), value_width).iter().enumerate() {
                let label = if index == 0 { field.label() } else { "" };
                let padding = " ".repeat(label_width.saturating_sub(visible_width(label)));
                view = view.push(
                    Line::new()
                        .span("│", muted.clone())
                        .span("  ", muted.clone())
                        .span(label, muted.clone())
                        .span(padding, muted.clone())
                        .span("  ", muted.clone())
                        .span(value, body.clone()),
                );
            }
        }
        view
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, SemanticTokens, Theme};

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
        assert!(view.lines().len() > 3);
        assert_eq!(view.lines()[2].spans()[2].text(), "Name");
        assert_eq!(view.lines()[3].spans()[2].text(), "");
    }
}
