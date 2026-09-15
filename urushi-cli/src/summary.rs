//! Titled, aligned result summaries.

use urushi::{
    Align, BlockStyle, Border, GridStyle, Length, Overflow, TextStyle, VerticalAlign, View,
};

use crate::CliRole;

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
    /// characters and may split them when wrapping or truncating. Raw ANSI is not accepted as component text.
    ///
    /// Style the component through its [`CliTheme`](crate::CliTheme) rather
    /// than by pre-rendering its content.
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
}

/// Presentation policy used to compose a [`Summary`] into a [`View`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryPresentation {
    muted: TextStyle,
    accent: TextStyle,
    body: TextStyle,
}

impl SummaryPresentation {
    /// Creates the canonical summary presentation from its three text roles.
    pub fn new(muted: TextStyle, accent: TextStyle, body: TextStyle) -> Self {
        Self {
            muted,
            accent,
            body,
        }
    }

    /// Composes summary data into renderer-neutral layout primitives.
    pub fn compose(&self, summary: &Summary) -> View {
        let fields = View::grid(
            GridStyle::new().columns([None, Some(Length::Cells(2)), None]),
            summary.fields.iter().map(|field| {
                [
                    filled_text(&field.label, &self.muted),
                    filled_text("  ", &self.muted),
                    View::text(field.value.clone(), self.body.clone()),
                ]
            }),
        );

        View::column(
            Align::Left,
            [
                View::text("│", self.muted.clone()),
                View::row(
                    VerticalAlign::Top,
                    [
                        View::text("◇", self.accent.clone()),
                        View::text("  ", self.muted.clone()),
                        View::text(summary.title.clone(), self.accent.clone()),
                    ],
                ),
                View::block(
                    BlockStyle::from_text_style(self.muted.clone())
                        .border(Border {
                            left: '│',
                            ..Border::HIDDEN
                        })
                        .border_top(false)
                        .border_right(false)
                        .border_bottom(false)
                        .border_text_style(self.muted.clone())
                        .padding((0, 0, 0, 2)),
                    fields,
                ),
            ],
        )
    }

    pub(crate) fn set_style(&mut self, role: CliRole, style: TextStyle) {
        match role {
            CliRole::Muted => self.muted = style,
            CliRole::Accent => self.accent = style,
            CliRole::Body => self.body = style,
            CliRole::Warning => {}
        }
    }
}

fn filled_text(text: impl Into<String>, style: &TextStyle) -> View {
    View::block(
        BlockStyle::from_text_style(style.clone())
            .width(Length::fill(1))
            .height(Length::fill(1))
            .overflow(Overflow::clip()),
        View::text(text, style.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use urushi::{Available, Color, SemanticTokens, StyledGrapheme, Theme, measure, resolve};

    use crate::test_support::{plain, style_at};

    fn plain_at(view: &View, width: usize) -> String {
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

    fn theme() -> Theme {
        Theme::from_tokens(SemanticTokens {
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
        })
    }

    #[test]
    fn selected_width_reflows_cjk_values_without_recomposing() {
        let theme = theme();
        let view = crate::CliTheme::from_theme(&theme).summary(
            &Summary::new("Result")
                .field("Name", "日本語日本語")
                .field("State", "ready"),
        );

        assert_eq!(
            plain_at(&view, 22),
            "│\n◇  Result\n│  Name   日本語日本語\n│  State  ready"
        );
        assert_eq!(
            plain_at(&view, 12),
            "│\n◇  Result\n│  Na  日本\n│      語日\n│      本語\n│  St  ready"
        );
    }

    #[test]
    fn aligns_long_cjk_labels_and_preserves_multiline_values() {
        let theme = theme();
        let view = crate::CliTheme::from_theme(&theme)
            .summary(&Summary::new("結果").field("項目名称", "first\n日本語\n"));

        assert_eq!(
            plain_at(&view, 14),
            "│\n◇  結果\n│  項目   firs\n│         t\n│         日本\n│         語"
        );
    }

    #[test]
    fn multiline_titles_and_labels_keep_later_fields_below_them() {
        let theme = theme();
        let view = crate::CliTheme::from_theme(&theme).summary(
            &Summary::new("First title line\nSecond title line")
                .field("First label line\nSecond label line", "value")
                .field("Next", "field"),
        );

        assert_eq!(
            plain_at(&view, 24),
            "│\n◇  First title line\n   Second title line\n│  First label li  value\n│  Second label l\n│  Next            field"
        );
    }

    #[test]
    fn an_empty_summary_keeps_the_rail_and_title() {
        let theme = theme();
        let view = crate::CliTheme::from_theme(&theme).summary(&Summary::new("Done"));

        assert_eq!(plain(&view), "│\n◇  Done");
        assert_eq!(measure(&view).height(), 2);
    }

    #[test]
    fn preserves_rail_title_label_gap_and_value_roles() {
        let muted = TextStyle::new().background(Color::BLUE);
        let accent = TextStyle::new().background(Color::GREEN);
        let body = TextStyle::new().background(Color::RED);
        let presentation = SummaryPresentation::new(muted.clone(), accent.clone(), body.clone());
        let view = presentation.compose(
            &Summary::new("Result")
                .field("A", "first")
                .field("Name", "second"),
        );

        assert_eq!(style_at(&view, 0, 0), muted);
        assert_eq!(style_at(&view, 1, 0), accent);
        assert_eq!(style_at(&view, 1, 1), muted);
        assert_eq!(style_at(&view, 1, 3), accent);
        assert_eq!(style_at(&view, 2, 0), muted);
        assert_eq!(style_at(&view, 2, 3), muted);
        assert_eq!(style_at(&view, 2, 4), muted);
        assert_eq!(style_at(&view, 2, 7), muted);
        assert_eq!(style_at(&view, 2, 9), body);
    }

    #[test]
    fn presentation_and_composed_view_equality_include_styles_and_data() {
        let theme = theme();
        let summary = Summary::new("Result").field("Name", "urushi");
        let cli_theme = crate::CliTheme::from_theme(&theme);
        let base = cli_theme.summary_presentation().clone();
        let changed = SummaryPresentation::new(
            TextStyle::new().underlined(),
            TextStyle::new(),
            TextStyle::new(),
        );

        assert_eq!(base.compose(&summary), base.clone().compose(&summary));
        assert_ne!(base.compose(&summary), changed.compose(&summary));
        assert_ne!(
            base.compose(&summary),
            base.compose(&Summary::new("Other").field("Name", "urushi"))
        );
    }
}
