//! Titled, aligned result summaries.

use std::sync::Arc;

use crate::text::{PrintableLines, wrapped_line_count};
use crate::view::{CanvasMeasure, CanvasRequirements};
use crate::{
    BlockStyle, Canvas, CanvasContext, CanvasItem, CanvasSizing, ComponentRole, Length, Overflow,
    Position, TextStyle, View,
};

const FIELD_PREFIX_WIDTH: usize = 3;
const FIELD_GAP_WIDTH: usize = 2;
const FIELD_FIXED_WIDTH: usize = FIELD_PREFIX_WIDTH + FIELD_GAP_WIDTH;

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
    /// Style the component through its [`ComponentTheme`](crate::ComponentTheme)
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

    /// Composes summary data into an intrinsically sized, renderer-neutral Canvas.
    ///
    /// Composition binds an owned snapshot without receiving an available
    /// area. During resolution, the bound item chooses the label column and
    /// wraps values from the final local Canvas width.
    pub fn compose(&self, summary: &Summary) -> View {
        let item = SummaryItem(Arc::new(SummaryFrame {
            summary: summary.clone(),
            presentation: self.clone(),
        }));
        View::canvas(Canvas::new().sizing(item.sizing()).item(item))
    }

    pub(crate) fn set_component_style(&mut self, role: ComponentRole, style: TextStyle) {
        match role {
            ComponentRole::Muted => self.muted = style,
            ComponentRole::Accent => self.accent = style,
            ComponentRole::Body => self.body = style,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryFrame {
    summary: Summary,
    presentation: SummaryPresentation,
}

impl SummaryFrame {
    fn natural_label_width(&self) -> usize {
        self.summary
            .fields
            .iter()
            .map(|field| text_requirements(&field.label).0)
            .max()
            .unwrap_or(0)
    }

    fn label_width(&self, width: usize) -> usize {
        self.natural_label_width()
            .min(width.saturating_sub(FIELD_FIXED_WIDTH) / 2)
    }

    fn value_width(&self, width: usize) -> usize {
        width
            .saturating_sub(FIELD_FIXED_WIDTH)
            .saturating_sub(self.label_width(width))
    }

    fn title_height(&self, width: usize) -> usize {
        wrapped_line_count(
            PrintableLines::new(&self.summary.title),
            width.saturating_sub(FIELD_PREFIX_WIDTH),
        )
    }

    fn field_height(&self, field: &SummaryField, width: usize) -> usize {
        let label_height = PrintableLines::new(&field.label).lines().len();
        let value_height =
            wrapped_line_count(PrintableLines::new(&field.value), self.value_width(width));
        label_height.max(value_height)
    }

    fn width_requirements(&self) -> CanvasRequirements {
        let (title_demand, title_floor) = text_requirements(&self.summary.title);
        let mut demand = FIELD_PREFIX_WIDTH.saturating_add(title_demand).max(1);
        let mut floor = FIELD_PREFIX_WIDTH.saturating_add(title_floor).max(1);

        if !self.summary.fields.is_empty() {
            let (label_demand, label_floor) =
                self.summary
                    .fields
                    .iter()
                    .fold((0usize, 0usize), |(demand, floor), field| {
                        let next = text_requirements(&field.label);
                        (demand.max(next.0), floor.max(next.1))
                    });
            let (value_demand, value_floor) =
                self.summary
                    .fields
                    .iter()
                    .fold((0usize, 0usize), |(demand, floor), field| {
                        let next = text_requirements(&field.value);
                        (demand.max(next.0), floor.max(next.1))
                    });
            demand = demand.max(
                FIELD_FIXED_WIDTH
                    .saturating_add(label_demand)
                    .saturating_add(label_demand.max(value_demand)),
            );
            floor = floor.max(
                FIELD_FIXED_WIDTH
                    .saturating_add(label_floor)
                    .saturating_add(label_floor.max(value_floor)),
            );
        }

        CanvasRequirements::new(demand, floor)
    }

    fn height_at(&self, width: usize) -> usize {
        self.summary.fields.iter().fold(
            1usize.saturating_add(self.title_height(width)),
            |height, field| height.saturating_add(self.field_height(field, width)),
        )
    }

    fn draw(&self, context: &mut CanvasContext) {
        let muted = &self.presentation.muted;
        let accent = &self.presentation.accent;
        let body = &self.presentation.body;
        context.text(Position::new(0, 0), "│", muted.clone());
        context.text(Position::new(0, 1), "◇", accent.clone());
        context.text(Position::new(1, 1), "  ", muted.clone());
        let width = context.size().width();
        let title_height = self.title_height(width);
        context.view(
            Position::new(3, 1),
            View::text(self.summary.title.clone(), accent.clone()),
            Some(width.saturating_sub(FIELD_PREFIX_WIDTH)),
            Some(title_height),
        );

        let label_width = self.label_width(width);
        let value_width = self.value_width(width);
        let value_x = FIELD_PREFIX_WIDTH
            .saturating_add(label_width)
            .saturating_add(FIELD_GAP_WIDTH);
        let mut y = 1usize.saturating_add(title_height);
        for field in &self.summary.fields {
            let field_height = self.field_height(field, width);
            for row in 0..field_height {
                let row_y = y.saturating_add(row);
                context.text(Position::new(0, position(row_y)), "│  ", muted.clone());
                context.text(
                    Position::new(
                        position(FIELD_PREFIX_WIDTH.saturating_add(label_width)),
                        position(row_y),
                    ),
                    "  ",
                    muted.clone(),
                );
            }

            if label_width > 0 {
                let label = View::block(
                    BlockStyle::from_text_style(muted.clone())
                        .width(Length::fill(1))
                        .height(Length::fill(1))
                        .overflow(Overflow::clip()),
                    View::text(field.label.clone(), muted.clone()),
                );
                context.view(
                    Position::new(position(FIELD_PREFIX_WIDTH), position(y)),
                    label,
                    Some(label_width),
                    Some(field_height),
                );
            }
            context.view(
                Position::new(position(value_x), position(y)),
                View::text(field.value.clone(), body.clone()),
                Some(value_width),
                Some(field_height),
            );
            y = y.saturating_add(field_height);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SummaryItem(Arc<SummaryFrame>);

impl SummaryItem {
    fn sizing(&self) -> CanvasSizing {
        CanvasSizing::intrinsic(self.clone())
    }
}

impl CanvasMeasure for SummaryItem {
    fn width_requirements(&self) -> CanvasRequirements {
        self.0.width_requirements()
    }

    fn height_requirements(&self, width: usize) -> CanvasRequirements {
        CanvasRequirements::new(self.0.height_at(width), 0)
    }
}

impl CanvasItem for SummaryItem {
    fn draw(&self, context: &mut CanvasContext) {
        self.0.draw(context);
    }
}

fn text_requirements(text: &str) -> (usize, usize) {
    PrintableLines::new(text)
        .lines()
        .iter()
        .fold((0usize, 0usize), |(demand, floor), line| {
            (
                demand.max(line.width()),
                floor.max(
                    line.graphemes()
                        .map(crate::Grapheme::width)
                        .max()
                        .unwrap_or(0),
                ),
            )
        })
}

fn position(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{plain, style_at};
    use crate::{Available, Color, SemanticTokens, StyledGrapheme, Theme, measure, resolve};

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
        let view = theme.summary(
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
            "│\n◇  Result\n│  Nam  日本\n│       語日\n│       本語\n│  Sta  read\n│       y"
        );
    }

    #[test]
    fn aligns_long_cjk_labels_and_preserves_multiline_values() {
        let view = theme().summary(&Summary::new("結果").field("項目名称", "first\n日本語\n"));

        assert_eq!(
            plain_at(&view, 14),
            "│\n◇  結果\n│  項目  first\n│        日本\n│        語"
        );
    }

    #[test]
    fn multiline_titles_and_labels_keep_later_fields_below_them() {
        let view = theme().summary(
            &Summary::new("First title line\nSecond title line")
                .field("First label line\nSecond label line", "value")
                .field("Next", "field"),
        );

        assert_eq!(
            plain_at(&view, 24),
            "│\n◇  First title line\n   Second title line\n│  First lab  value\n│  Second la\n│  Next       field"
        );
    }

    #[test]
    fn an_empty_summary_keeps_the_rail_and_title() {
        let view = theme().summary(&Summary::new("Done"));

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
        let base = theme.components().summary().clone();
        let changed = SummaryPresentation::new(
            TextStyle::new().underline(),
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
