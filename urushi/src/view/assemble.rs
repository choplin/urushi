//! The assembly phase: the rectangle the settled sizes describe.
//!
//! Nothing here decides a size. Every number this module uses — a width, a row
//! count, the rows a box keeps — was settled by the [`width`](super::width) and
//! [`height`](super::height) phases; assembly turns text into graphemes, places
//! it inside the frame, and draws the border and the margin around it. That is
//! what makes the two backends agree: the geometry existed before either of
//! them saw it.

use crate::text::PrintableText;
use crate::{Align, Sides, TextStyle, VerticalAlign};

use super::geometry::Size;
use super::height::{Sized, SizedNode};
use super::resolve::StyledGrapheme;

/// A rectangle under construction: every row is exactly `width` cells wide.
#[derive(Debug, Clone, Default)]
pub(super) struct Rect {
    pub width: usize,
    pub rows: Vec<Vec<StyledGrapheme>>,
}

impl Rect {
    pub fn size(&self) -> Size {
        Size::new(self.width, self.rows.len())
    }

    fn blank_row(&self, style: &TextStyle) -> Vec<StyledGrapheme> {
        blank(self.width, style)
    }

    /// Cuts the rectangle to `max_width` cells, grapheme-atomically.
    ///
    /// This is the degenerate-case safety net, not a layout rule: it fires only
    /// when a rectangle could not be made to fit at all.
    pub fn crop_width(&mut self, max_width: usize, fill: &TextStyle) {
        if self.width <= max_width {
            return;
        }
        for row in &mut self.rows {
            *row = truncate_row(std::mem::take(row), max_width, fill);
        }
        self.width = max_width;
    }

    pub fn crop_height(&mut self, max_height: usize) {
        if self.rows.len() > max_height {
            self.rows.truncate(max_height);
        }
    }
}

/// Builds the rectangle `sized` describes.
pub(super) fn assemble(sized: &Sized<'_>) -> Rect {
    let rect = match &sized.node {
        SizedNode::Text {
            lines,
            style,
            align,
            fill,
        } => Rect {
            width: sized.width,
            rows: lines
                .iter()
                .map(|line| align_row(graphemes(line, style), sized.width, *align, fill))
                .collect(),
        },
        SizedNode::Block { .. } => block(sized),
        SizedNode::Row(align, children) => row(*align, children, sized.width),
        SizedNode::Column(align, children) => column(*align, children, sized.width),
    };
    debug_assert_eq!(
        rect.size(),
        Size::new(sized.width, sized.height),
        "assembly builds exactly the rectangle the sizing phases decided"
    );
    rect
}

/// The style filling padding a parent introduces around a node.
fn fill_style(sized: &Sized<'_>) -> TextStyle {
    match &sized.node {
        SizedNode::Block { style, .. } => style.text().clone(),
        _ => TextStyle::new(),
    }
}

/// Builds a block: place the content, then close the frame around it.
fn block(sized: &Sized<'_>) -> Rect {
    let SizedNode::Block {
        style,
        padding,
        margin,
        content_width,
        content_height,
        child,
    } = &sized.node
    else {
        unreachable!("a block")
    };
    let (content_width, content_height) = (*content_width, *content_height);
    let fill = style.text().clone();
    let (pl, pr, pt, pb) = (
        usize::from(padding.left),
        usize::from(padding.right),
        usize::from(padding.top),
        usize::from(padding.bottom),
    );

    let content = assemble(child);
    debug_assert!(
        content.width <= content_width,
        "a box is never widened by what it contains: {} > {content_width}",
        content.width
    );

    // 1. The content, aligned inside the width the box left it.
    let mut content_rows: Vec<Vec<StyledGrapheme>> = content
        .rows
        .into_iter()
        .map(|row| align_row(row, content_width, style.horizontal_alignment(), &fill))
        .collect();

    // 2. The rows the box keeps, which the height phase decided; the vertical
    //    alignment places whatever slack is left, and has nothing to say when
    //    there is none.
    content_rows.truncate(content_height);
    let gap = content_height - content_rows.len();
    let (above, below) = match style.vertical_alignment() {
        VerticalAlign::Top => (0, gap),
        // The odd extra row goes below, the opposite of a Row's Center bias.
        VerticalAlign::Center => (gap / 2, gap - gap / 2),
        VerticalAlign::Bottom => (gap, 0),
    };

    // 3. Padding, applied with the block's own fill.
    let total = pl + content_width + pr;
    let mut rect = Rect {
        width: total,
        rows: Vec::with_capacity(content_height + pt + pb),
    };
    let blank_row = blank(total, &fill);
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

    // 4. Border, drawn at the used size.
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
    if *margin != Sides::default() {
        rect = spaced(rect, *margin);
    }
    rect
}

/// Surrounds a rectangle with plain, unstyled margin.
fn spaced(rect: Rect, margin: Sides) -> Rect {
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
        let mut padded = blank(ml, &plain);
        padded.extend(row);
        padded.extend(blank(mr, &plain));
        out.rows.push(padded);
    }
    for _ in 0..margin.bottom {
        out.rows.push(blank(outer, &plain));
    }
    out
}

/// Places children side by side, padding the shorter ones by `align`.
fn row(align: VerticalAlign, children: &[Sized<'_>], width: usize) -> Rect {
    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| (assemble(child), fill_style(child)))
        .collect();
    let height = rects
        .iter()
        .map(|(rect, _)| rect.rows.len())
        .max()
        .unwrap_or(0);

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

/// Stacks children, padding the narrower ones to `width` by `align`.
fn column(align: Align, children: &[Sized<'_>], width: usize) -> Rect {
    let mut rows = Vec::new();
    for child in children {
        let fill = fill_style(child);
        for row in assemble(child).rows {
            rows.push(align_row(row, width, align, &fill));
        }
    }
    Rect { width, rows }
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
    PrintableText::new(text)
        .graphemes()
        .map(|grapheme| StyledGrapheme::new(grapheme.as_str(), grapheme.width(), style.clone()))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn symbols(row: &[StyledGrapheme]) -> String {
        row.iter().map(StyledGrapheme::symbol).collect()
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
