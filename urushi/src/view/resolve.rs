//! The one pass: a [`View`] and an [`Available`] area in, one rectangle out.
//!
//! Resolution is a pure function of those two inputs — no terminal state, no
//! capability profile, no escape sequences. Its output is a [`ResolvedView`]:
//! a size and rows of graphemes carrying logical styles and the widths this
//! pass decided, which is the single thing both backends draw. That is why the
//! ANSI string and the Ratatui buffer cannot disagree about geometry.
//!
//! Sizes come from [`sizing`](super::sizing); this module assembles the
//! rectangle they describe. The procedure is specified in
//! `docs/view-model.md`.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{Align, BlockStyle, Overflow, Sides, TextStyle, VerticalAlign, View, wrap_text};

use super::geometry::{Available, Size};
use super::rendered::RenderedBlock;
use super::sizing::{
    Degraded, border_extent, degrade, height_axis, horizontal, max_content_width,
    min_content_width, text_lines, vertical, width_axis,
};

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

/// Returns the intrinsic rectangle `view` occupies: its size when no area
/// bounds it.
///
/// This is [`resolve`] under [`Available::NONE`], not a second set of rules.
pub fn measure(view: &View) -> Size {
    layout(view, Available::NONE).size()
}

/// Resolves `view` into one rectangle sized under `available`.
///
/// Every node resolves its own size under the area, so a bound reshapes a box
/// rather than cutting it. The crop below is the degenerate-case safety net:
/// it fires only when a rectangle could not be made to fit — an area that
/// cannot hold a frame at all — and it cuts grapheme-atomically.
pub fn resolve(view: &View, available: Available) -> ResolvedView {
    let mut rect = layout(view, available);
    if let Some(width) = available.width() {
        rect.crop_width(width, &TextStyle::new());
    }
    if let Some(height) = available.height() {
        rect.crop_height(height);
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

    fn crop_width(&mut self, max_width: usize, fill: &TextStyle) {
        if self.width <= max_width {
            return;
        }
        for row in &mut self.rows {
            *row = truncate_row(std::mem::take(row), max_width, fill);
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
fn truncate_row(
    row: Vec<StyledGrapheme>,
    max_width: usize,
    fill: &TextStyle,
) -> Vec<StyledGrapheme> {
    let mut output = Vec::with_capacity(row.len());
    let mut consumed = 0;
    for grapheme in row {
        if consumed + grapheme.width() > max_width {
            break;
        }
        consumed += grapheme.width();
        output.push(grapheme);
    }
    output.extend(blank(max_width - consumed, fill));
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

fn layout(view: &View, available: Available) -> Rect {
    match view {
        // A bare text leaf wraps under a width bound: the same default a
        // block's content gets. Another policy requires a block, because the
        // policy is a box property.
        View::Text(text, style) => layout_text(
            text,
            style,
            available.width(),
            Align::Left,
            style,
            &Overflow::Wrap,
        ),
        View::Block(style, child) => layout_block(style, child, available),
        View::Row(align, children) => layout_row(*align, children, available),
        View::Column(align, children) => layout_column(*align, children, available),
    }
}

/// Lays out a text leaf, absorbing overflow and aligning to a target width.
///
/// `target`, `align`, and `overflow` are supplied by an enclosing block: the
/// block resolves its content width first, and the text is fitted to it under
/// the block's overflow policy. Alignment is per line, which aligning the
/// finished rectangle as a unit would lose.
fn layout_text(
    text: &str,
    style: &TextStyle,
    target: Option<usize>,
    align: Align,
    fill: &TextStyle,
    overflow: &Overflow,
) -> Rect {
    let mut lines: Vec<String> = match (target, overflow) {
        (Some(width), Overflow::Wrap) => wrap_text(text, width),
        (Some(width), Overflow::Clip(marker)) => text_lines(text)
            .iter()
            .map(|line| clip_line(line, width, marker))
            .collect(),
        (None, _) => text.lines().map(str::to_owned).collect(),
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

/// Cuts one line to `width` cells between graphemes, ending it with `marker`.
///
/// The marker occupies cells of its own, so the text keeps the width less the
/// marker's own. A marker the box cannot hold is dropped: a silent cut is
/// closer to the request than a box filled with the marker alone.
fn clip_line(line: &str, width: usize, marker: &str) -> String {
    if UnicodeWidthStr::width(line) <= width {
        return line.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let marker_width = UnicodeWidthStr::width(marker);
    let marker = if marker_width < width { marker } else { "" };
    let budget = width - UnicodeWidthStr::width(marker);

    let mut output = String::new();
    let mut consumed = 0;
    for grapheme in line.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if consumed + grapheme_width > budget {
            break;
        }
        consumed += grapheme_width;
        output.push_str(grapheme);
    }
    output.push_str(marker);
    output
}

/// Resolves a block under `available`: size the box, then build it inward.
///
/// The box's used size is settled before the content is laid out, so the
/// frame closes at that size and the content is fitted inside it. Nothing
/// here cuts an assembled rectangle; the degenerate safety net in [`resolve`]
/// is the only crop left in the model.
fn layout_block(style: &BlockStyle, child: &View, available: Available) -> Rect {
    let fill = style.text().clone();
    let border = border_extent(style);
    let mut padding = style.padding_sides();
    let mut margin = style.margin_sides();
    let min_child = min_content_width(child);

    // 1. Degrade the frame to the area: margin first, then padding.
    let across = degrade(
        available.width(),
        horizontal(margin),
        border.width(),
        horizontal(padding),
        min_child,
    );
    let down = degrade(
        available.height(),
        vertical(margin),
        border.height(),
        vertical(padding),
        0,
    );
    apply(&mut margin, &mut padding, across, down);

    let (pl, pr, pt, pb) = (
        usize::from(padding.left),
        usize::from(padding.right),
        usize::from(padding.top),
        usize::from(padding.bottom),
    );
    let width = width_axis(style, border.width() + pl + pr);
    let height = height_axis(style, border.height() + pt + pb);
    // Every sizing property measures the box; margin lies outside it.
    let box_width = available
        .width()
        .map(|area| area.saturating_sub(horizontal(margin)));
    let box_height = available
        .height()
        .map(|area| area.saturating_sub(vertical(margin)));

    // 2. The used width, settled before anything is assembled.
    let used_width = width.used(box_width, max_content_width(child), min_child);
    let content_width = used_width - width.frame;

    // 3. The content, resolved inside what the box leaves it. A directly
    // contained text leaf is fitted per line under the block's overflow
    // policy; any other child is a rectangle already, aligned as a unit. The
    // policy is a text-fitting rule, so it does not apply to that child: a
    // child view absorbs its own overflow when it resolves.
    let content = match child {
        View::Text(text, text_style) => layout_text(
            text,
            text_style,
            Some(content_width),
            style.horizontal_alignment(),
            &fill,
            style.overflow_policy(),
        ),
        other => {
            let child_rect = layout(
                other,
                Available::new(Some(content_width), height.content_bound(box_height)),
            );
            let width = content_width.max(child_rect.width);
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
    // An unsplittable grapheme wider than the content width widens the box
    // rather than being cut.
    let content_width = content.width;

    // 4. The used height, now that step 3 has decided how many rows there are.
    let used_height = height.used(box_height, content.rows.len(), 0);
    let content_height = used_height - height.frame;

    // 5. Horizontal padding, applied with the block's own fill.
    let total = pl + content_width + pr;
    let mut rect = Rect {
        width: total,
        rows: Vec::new(),
    };
    let blank_row = blank(total, &fill);

    // 6. Vertical padding and the vertical alignment gap, which sits outside
    // the padding. Content taller than the box clips inside the frame; the
    // alignment places slack, so it has nothing to say when there is none.
    let mut content_rows = content.rows;
    content_rows.truncate(content_height);
    let vertical_gap = content_height - content_rows.len();
    let (above, below) = match style.vertical_alignment() {
        VerticalAlign::Top => (0, vertical_gap),
        // The odd extra row goes below, the opposite of a Row's Center bias.
        VerticalAlign::Center => (vertical_gap / 2, vertical_gap - vertical_gap / 2),
        VerticalAlign::Bottom => (vertical_gap, 0),
    };
    for _ in 0..above + pt {
        rect.rows.push(blank_row.clone());
    }
    for row in content_rows {
        let mut padded = blank(pl, &fill);
        padded.extend(row);
        padded.extend(blank(pr, &fill));
        rect.rows.push(padded);
    }
    for _ in 0..pb + below {
        rect.rows.push(blank_row.clone());
    }

    // 7. Border, drawn at the used size.
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

    // 8. Margin: plain, unstyled space outside the border.
    if margin != Sides::default() {
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

    rect
}

/// Zeroes the sides the degradation decided each axis cannot keep.
fn apply(margin: &mut Sides, padding: &mut Sides, across: Degraded, down: Degraded) {
    if across.margin {
        margin.left = 0;
        margin.right = 0;
    }
    if across.padding {
        padding.left = 0;
        padding.right = 0;
    }
    if down.margin {
        margin.top = 0;
        margin.bottom = 0;
    }
    if down.padding {
        padding.top = 0;
        padding.bottom = 0;
    }
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

/// Places children side by side.
///
/// The main axis is not distributed yet: every child takes its intrinsic
/// width, and a `Length::Fill` child resolves like an automatic one. Only the
/// cross axis passes the area down, where there is nothing to divide.
fn layout_row(align: VerticalAlign, children: &[View], available: Available) -> Rect {
    if children.is_empty() {
        return Rect::default();
    }

    let child_available = Available::new(None, available.height());
    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| (layout(child, child_available), child.fill_style()))
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

/// Stacks children.
///
/// As in [`layout_row`], the main axis — height here — is not distributed
/// yet; the cross axis passes the area down unchanged.
fn layout_column(align: Align, children: &[View], available: Available) -> Rect {
    if children.is_empty() {
        return Rect::default();
    }

    let child_available = Available::new(available.width(), None);
    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| (layout(child, child_available), child.fill_style()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(row: &[StyledGrapheme]) -> String {
        row.iter().map(StyledGrapheme::symbol).collect()
    }

    #[test]
    fn a_clip_keeps_the_marker_inside_the_width() {
        assert_eq!(clip_line("hello world", 5, ""), "hello");
        assert_eq!(clip_line("hello world", 5, "…"), "hell…");
        assert_eq!(clip_line("hello world", 5, "..."), "he...");
        assert_eq!(
            clip_line("hello", 9, "…"),
            "hello",
            "a line that fits is untouched"
        );
    }

    #[test]
    fn a_clip_never_splits_a_grapheme_and_measures_the_marker_in_cells() {
        assert_eq!(
            clip_line("日本語", 5, ""),
            "日本",
            "the third would straddle"
        );
        assert_eq!(
            clip_line("日本語", 4, "→"),
            "日→",
            "a wide marker costs two"
        );
        assert_eq!(clip_line("e\u{301}xyz", 2, ""), "e\u{301}x");
    }

    #[test]
    fn a_marker_that_cannot_fit_is_dropped() {
        assert_eq!(clip_line("hello", 3, "..."), "hel", "no room for content");
        assert_eq!(clip_line("hello", 1, "…"), "h", "the marker fills the box");
        assert_eq!(clip_line("hello", 0, "…"), "", "nothing fits at all");
    }

    #[test]
    fn truncating_a_row_drops_a_straddling_grapheme_and_blanks_its_cells() {
        let plain = TextStyle::new();
        let row = graphemes("a日b", &plain);

        assert_eq!(symbols(&truncate_row(row.clone(), 3, &plain)), "a日");
        assert_eq!(
            symbols(&truncate_row(row.clone(), 2, &plain)),
            "a ",
            "the wide grapheme goes, and its cell is blanked"
        );
        assert_eq!(symbols(&truncate_row(row, 4, &plain)), "a日b");
    }

    #[test]
    fn aligning_a_row_distributes_the_gap_and_biases_center_left() {
        let plain = TextStyle::new();
        let row = || graphemes("ab", &plain);

        assert_eq!(symbols(&align_row(row(), 5, Align::Left, &plain)), "ab   ");
        assert_eq!(symbols(&align_row(row(), 5, Align::Right, &plain)), "   ab");
        assert_eq!(
            symbols(&align_row(row(), 5, Align::Center, &plain)),
            " ab  ",
            "the odd cell goes right"
        );
        assert_eq!(
            symbols(&align_row(row(), 1, Align::Left, &plain)),
            "ab",
            "a row wider than the target is never cut here"
        );
    }
}
