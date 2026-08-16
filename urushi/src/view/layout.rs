//! The one layout pass: a [`View`] tree resolves to a rectangle.
//!
//! Display width is decided here, once, and carried per grapheme in the
//! [`ResolvedView`]. No renderer re-measures what this pass already decided.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Align, BlockStyle, TextStyle, VerticalAlign, View, wrap_text};

use super::rendered::RenderedBlock;

/// The size of a resolved rectangle, in terminal cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Size {
    width: usize,
    height: usize,
}

impl Size {
    /// The empty rectangle.
    pub const ZERO: Self = Self {
        width: 0,
        height: 0,
    };

    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns whether the rectangle occupies no cells.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// An outer clip applied after intrinsic layout.
///
/// A terminal width or a Ratatui `Rect` becomes a `Limits`. Clipping is
/// layered: [`BlockStyle`]'s `max_width` and `max_height` crop a block during
/// resolution, and these limits are applied last, so the smaller bound wins.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    max_width: Option<usize>,
    max_height: Option<usize>,
}

impl Limits {
    /// Imposes no outer bound.
    pub const NONE: Self = Self {
        max_width: None,
        max_height: None,
    };

    pub const fn new(max_width: Option<usize>, max_height: Option<usize>) -> Self {
        Self {
            max_width,
            max_height,
        }
    }

    /// Bounds the width only.
    pub const fn width(max_width: usize) -> Self {
        Self::new(Some(max_width), None)
    }

    /// Bounds both dimensions.
    pub const fn size(max_width: usize, max_height: usize) -> Self {
        Self::new(Some(max_width), Some(max_height))
    }

    pub const fn max_width(&self) -> Option<usize> {
        self.max_width
    }

    pub const fn max_height(&self) -> Option<usize> {
        self.max_height
    }
}

/// One grapheme, the width it occupies, and its logical style.
///
/// A renderer cannot split a grapheme cluster or a wide character, because it
/// never sees text below this granularity, and it cannot disagree with the
/// layout pass about a width, because the width it needs is in the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledGrapheme {
    symbol: String,
    width: usize,
    style: TextStyle,
}

impl StyledGrapheme {
    pub fn new(symbol: impl Into<String>, width: usize, style: TextStyle) -> Self {
        Self {
            symbol: symbol.into(),
            width,
            style,
        }
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn style(&self) -> &TextStyle {
        &self.style
    }

    fn space(style: TextStyle) -> Self {
        Self::new(" ", 1, style)
    }

    pub(crate) fn map_style(mut self, map: impl FnOnce(&TextStyle) -> TextStyle) -> Self {
        self.style = map(&self.style);
        self
    }
}

/// A view resolved to one rectangle of styled graphemes.
///
/// Every row's widths sum to `size.width()`, and the row count equals
/// `size.height()`. Styles are logical: a
/// [`TerminalProfile`](crate::TerminalProfile) is applied when a renderer
/// serializes the rectangle, so capability resolution stays at the output
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedView {
    size: Size,
    rows: Vec<Vec<StyledGrapheme>>,
}

impl ResolvedView {
    pub(crate) fn new(size: Size, rows: Vec<Vec<StyledGrapheme>>) -> Self {
        Self { size, rows }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub fn rows(&self) -> &[Vec<StyledGrapheme>] {
        &self.rows
    }

    /// Replaces every grapheme style, keeping the geometry untouched.
    pub(crate) fn map_styles(mut self, map: impl Fn(&TextStyle) -> TextStyle) -> Self {
        self.rows = self
            .rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|grapheme| grapheme.map_style(&map))
                    .collect()
            })
            .collect();
        self
    }

    /// Serializes this rectangle, coalescing adjacent graphemes of equal style
    /// into one SGR scope.
    pub(crate) fn into_rendered_block(self) -> RenderedBlock {
        let text = self
            .rows
            .iter()
            .map(|row| serialize_row(row))
            .collect::<Vec<_>>()
            .join("\n");
        RenderedBlock::measured(text, self.size)
    }
}

fn serialize_row(row: &[StyledGrapheme]) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < row.len() {
        let style = row[index].style();
        let mut run = String::new();
        while index < row.len() && row[index].style() == style {
            run.push_str(row[index].symbol());
            index += 1;
        }
        output.push_str(&style.paint(&run));
    }
    output
}

/// Returns the rectangle `view` occupies without laying it out for output.
pub fn measure(view: &View) -> Size {
    layout(view).size()
}

/// Resolves `view` into one rectangle, cropped to `limits`.
pub fn resolve(view: &View, limits: Limits) -> ResolvedView {
    let mut rect = layout(view);
    if let Some(max_width) = limits.max_width() {
        rect.crop_width(max_width);
    }
    if let Some(max_height) = limits.max_height() {
        rect.crop_height(max_height);
    }
    ResolvedView::new(rect.size(), rect.rows)
}

/// A rectangle under construction: every row is exactly `width` cells wide.
#[derive(Debug, Clone, Default)]
struct Rect {
    width: usize,
    rows: Vec<Vec<StyledGrapheme>>,
}

impl Rect {
    fn size(&self) -> Size {
        Size::new(self.width, self.rows.len())
    }

    fn blank_row(&self, style: &TextStyle) -> Vec<StyledGrapheme> {
        blank(self.width, style)
    }

    fn crop_width(&mut self, max_width: usize) {
        if self.width <= max_width {
            return;
        }
        for row in &mut self.rows {
            *row = truncate_row(std::mem::take(row), max_width);
        }
        self.width = max_width;
    }

    fn crop_height(&mut self, max_height: usize) {
        if self.rows.len() > max_height {
            self.rows.truncate(max_height);
        }
    }
}

/// Truncates one row to `max_width` cells.
///
/// A grapheme that would straddle the bound is dropped rather than split, and
/// the freed cells become blanks so every row keeps the rectangle's width.
fn truncate_row(row: Vec<StyledGrapheme>, max_width: usize) -> Vec<StyledGrapheme> {
    let mut output = Vec::with_capacity(row.len());
    let mut consumed = 0;
    for grapheme in row {
        if consumed + grapheme.width() > max_width {
            break;
        }
        consumed += grapheme.width();
        output.push(grapheme);
    }
    output.extend(blank(max_width - consumed, &TextStyle::new()));
    output
}

fn blank(width: usize, style: &TextStyle) -> Vec<StyledGrapheme> {
    (0..width)
        .map(|_| StyledGrapheme::space(style.clone()))
        .collect()
}

/// Splits plain text into styled graphemes with their display widths.
fn graphemes(text: &str, style: &TextStyle) -> Vec<StyledGrapheme> {
    text.graphemes(true)
        .map(|grapheme| {
            StyledGrapheme::new(grapheme, UnicodeWidthStr::width(grapheme), style.clone())
        })
        .collect()
}

fn row_width(row: &[StyledGrapheme]) -> usize {
    row.iter().map(StyledGrapheme::width).sum()
}

/// Pads `row` to `width` cells, distributing the gap according to `align`.
fn align_row(
    mut row: Vec<StyledGrapheme>,
    width: usize,
    align: Align,
    fill: &TextStyle,
) -> Vec<StyledGrapheme> {
    let gap = width.saturating_sub(row_width(&row));
    let (left, right) = match align {
        Align::Left => (0, gap),
        Align::Center => (gap / 2, gap - gap / 2),
        Align::Right => (gap, 0),
    };
    let mut output = blank(left, fill);
    output.append(&mut row);
    output.extend(blank(right, fill));
    output
}

fn layout(view: &View) -> Rect {
    match view {
        View::Text(text, style) => layout_text(text, style, None, Align::Left, style),
        View::Block(style, child) => layout_block(style, child),
        View::Row(align, children) => layout_row(*align, children),
        View::Column(align, children) => layout_column(*align, children),
    }
}

/// Lays out a text leaf, optionally wrapped and aligned to a target width.
///
/// `target` and `align` are supplied by an enclosing block: a block's `width`
/// wraps the text it directly contains, and its `align` places each wrapped
/// line inside the content box. That keeps per-line alignment, which aligning
/// the finished rectangle as a unit would lose.
fn layout_text(
    text: &str,
    style: &TextStyle,
    target: Option<usize>,
    align: Align,
    fill: &TextStyle,
) -> Rect {
    let mut lines: Vec<String> = match target {
        Some(width) => wrap_text(text, width),
        None => text.lines().map(str::to_owned).collect(),
    };
    if lines.is_empty() {
        lines.push(String::new());
    }

    let rows: Vec<Vec<StyledGrapheme>> = lines
        .iter()
        .map(|line| graphemes(line, style))
        .collect::<Vec<_>>();
    // A wide grapheme that cannot fit the target may still overflow it, so the
    // natural width keeps the rectangle consistent.
    let natural = rows.iter().map(|row| row_width(row)).max().unwrap_or(0);
    let width = target.map_or(natural, |target| target.max(natural));

    Rect {
        width,
        rows: rows
            .into_iter()
            .map(|row| align_row(row, width, align, fill))
            .collect(),
    }
}

/// Resolves a block: wrap, align and pad, border, margin, then crop.
///
/// This is the box model, in the order [`BlockStyle::render`] documents.
fn layout_block(style: &BlockStyle, child: &View) -> Rect {
    let padding = style.padding_sides();
    let (pl, pr, pt, pb) = (
        usize::from(padding.left),
        usize::from(padding.right),
        usize::from(padding.top),
        usize::from(padding.bottom),
    );
    let fill = style.text().clone();
    let inner_target = style
        .fixed_width()
        .map(|width| usize::from(width).saturating_sub(pl + pr).max(1));

    // 1. The content box. A directly contained text leaf wraps and aligns per
    // line; any other child is a rectangle already, aligned as a unit.
    let content = match child {
        View::Text(text, text_style) => layout_text(
            text,
            text_style,
            inner_target,
            style.horizontal_alignment(),
            &fill,
        ),
        other => {
            let child_rect = layout(other);
            let width =
                inner_target.map_or(child_rect.width, |target| target.max(child_rect.width));
            Rect {
                width,
                rows: child_rect
                    .rows
                    .into_iter()
                    .map(|row| align_row(row, width, style.horizontal_alignment(), &fill))
                    .collect(),
            }
        }
    };

    // 2. Horizontal padding, applied with the block's own fill.
    let total = pl + content.width + pr;
    let mut rect = Rect {
        width: total,
        rows: Vec::new(),
    };
    let blank_row = blank(total, &fill);

    // 3. Vertical padding and the vertical alignment gap, which sits outside
    // the padding.
    let natural_height = pt + content.rows.len() + pb;
    let target_height = style.fixed_height().map_or(natural_height, |height| {
        usize::from(height).max(natural_height)
    });
    let vertical_gap = target_height - natural_height;
    let (above, below) = match style.vertical_alignment() {
        VerticalAlign::Top => (0, vertical_gap),
        // The odd extra row goes below, the opposite of a Row's Center bias.
        VerticalAlign::Center => (vertical_gap / 2, vertical_gap - vertical_gap / 2),
        VerticalAlign::Bottom => (vertical_gap, 0),
    };
    for _ in 0..above + pt {
        rect.rows.push(blank_row.clone());
    }
    for row in content.rows {
        let mut padded = blank(pl, &fill);
        padded.extend(row);
        padded.extend(blank(pr, &fill));
        rect.rows.push(padded);
    }
    for _ in 0..pb + below {
        rect.rows.push(blank_row.clone());
    }

    // 4. Border.
    if let Some(border) = style.border_kind() {
        let border_style = style.border_style();
        let left = style.is_border_left_enabled();
        let right = style.is_border_right_enabled();
        let mut bordered = Rect {
            width: rect.width + usize::from(left) + usize::from(right),
            rows: Vec::with_capacity(rect.rows.len() + 2),
        };
        if style.is_border_top_enabled() {
            bordered.rows.push(edge_row(
                border.top_left,
                border.top,
                border.top_right,
                rect.width,
                left,
                right,
                &border_style,
            ));
        }
        for row in rect.rows {
            let mut edged = Vec::with_capacity(row.len() + 2);
            if left {
                edged.extend(graphemes(&border.left.to_string(), &border_style));
            }
            edged.extend(row);
            if right {
                edged.extend(graphemes(&border.right.to_string(), &border_style));
            }
            bordered.rows.push(edged);
        }
        if style.is_border_bottom_enabled() {
            bordered.rows.push(edge_row(
                border.bottom_left,
                border.bottom,
                border.bottom_right,
                rect.width,
                left,
                right,
                &border_style,
            ));
        }
        rect = bordered;
    }

    // 5. Margin: plain, unstyled space outside the border.
    let margin = style.margin_sides();
    if margin != crate::Sides::default() {
        let plain = TextStyle::new();
        let (ml, mr) = (usize::from(margin.left), usize::from(margin.right));
        let outer = ml + rect.width + mr;
        let mut out = Rect {
            width: outer,
            rows: Vec::with_capacity(rect.rows.len() + usize::from(margin.top + margin.bottom)),
        };
        for _ in 0..margin.top {
            out.rows.push(blank(outer, &plain));
        }
        for row in rect.rows {
            let mut spaced = blank(ml, &plain);
            spaced.extend(row);
            spaced.extend(blank(mr, &plain));
            out.rows.push(spaced);
        }
        for _ in 0..margin.bottom {
            out.rows.push(blank(outer, &plain));
        }
        rect = out;
    }

    // 6. Maximum dimensions crop the final block. This is a crop, not another
    // layout pass, so fixed dimensions may be larger while the block still
    // obeys these hard limits.
    if let Some(max_width) = style.maximum_width().filter(|width| *width > 0) {
        rect.crop_width(usize::from(max_width));
    }
    if let Some(max_height) = style.maximum_height().filter(|height| *height > 0) {
        rect.crop_height(usize::from(max_height));
    }
    rect
}

#[allow(clippy::too_many_arguments)]
fn edge_row(
    left_corner: char,
    fill: char,
    right_corner: char,
    width: usize,
    left: bool,
    right: bool,
    style: &TextStyle,
) -> Vec<StyledGrapheme> {
    let mut text = String::new();
    if left {
        text.push(left_corner);
    }
    for _ in 0..width {
        text.push(fill);
    }
    if right {
        text.push(right_corner);
    }
    graphemes(&text, style)
}

fn layout_row(align: VerticalAlign, children: &[View]) -> Rect {
    if children.is_empty() {
        return Rect::default();
    }

    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| (layout(child), child.fill_style()))
        .collect();
    let height = rects
        .iter()
        .map(|(rect, _)| rect.rows.len())
        .max()
        .unwrap_or(0);
    let width = rects.iter().map(|(rect, _)| rect.width).sum();

    let mut rows: Vec<Vec<StyledGrapheme>> = vec![Vec::new(); height];
    for (rect, fill) in rects {
        let gap = height - rect.rows.len();
        // Lip Gloss places the odd extra row of a Center alignment above the
        // shorter child; a BlockStyle's vertical_align places it below.
        let above = match align {
            VerticalAlign::Top => 0,
            VerticalAlign::Center => gap.div_ceil(2),
            VerticalAlign::Bottom => gap,
        };
        let blank_row = rect.blank_row(&fill);
        for (index, row) in rows.iter_mut().enumerate() {
            match index
                .checked_sub(above)
                .and_then(|offset| rect.rows.get(offset))
            {
                Some(source) => row.extend(source.iter().cloned()),
                None => row.extend(blank_row.iter().cloned()),
            }
        }
    }

    Rect { width, rows }
}

fn layout_column(align: Align, children: &[View]) -> Rect {
    if children.is_empty() {
        return Rect::default();
    }

    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| (layout(child), child.fill_style()))
        .collect();
    let width = rects.iter().map(|(rect, _)| rect.width).max().unwrap_or(0);

    let mut rows = Vec::new();
    for (rect, fill) in rects {
        for row in rect.rows {
            rows.push(align_row(row, width, align, &fill));
        }
    }

    Rect { width, rows }
}
