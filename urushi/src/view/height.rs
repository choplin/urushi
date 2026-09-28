//! The height phase: how many rows each node occupies, once every width is
//! settled.
//!
//! Height is the result of wrapping, so it is defined only after a width
//! exists. That is why it is a phase of its own rather than a second half of
//! the [`width`](super::width) one, and why it may look back at the widths that
//! phase decided but never revise them.
//!
//! It runs in two steps, and the split is what keeps a line from being fitted
//! twice:
//!
//! - [`fit`] fits every text leaf to the width it was given, exactly once.
//!   Fitting depends on the width alone, so no height bound can change its
//!   result.
//! - [`heights`] hands height bounds down and counts rows. A `Column` whose
//!   area cannot hold its children asks each for its demand and then settles a
//!   short child at its assignment — the second visit recomputes numbers over
//!   the fitted lines and never fits them again.

use crate::text::{
    PrintableLines, PrintableText, StyledTextGrapheme, wrap_styled_lines, wrap_text,
};
use crate::{
    Align, BlockStyle, BlockTitle, Canvas, Key, Overflow, Sides, StyledText, TextStyle,
    VerticalAlign, Viewport,
};

use super::geometry::Constraint;
use super::sizing::{
    Claim, Kind, border_extent, degrade, distribute_constraints, height_axis, text_lines, vertical,
};
use super::width::{WidthNode, Widths};

/// One node with its width settled and its text already fitted to it.
#[derive(Debug)]
pub(super) struct Fitted<'a> {
    pub width: usize,
    height_kind: Kind,
    height_floor: usize,
    height_projection: bool,
    node: FittedNode<'a>,
}

#[derive(Debug)]
enum FittedNode<'a> {
    Text {
        lines: Vec<Vec<StyledTextGrapheme<'a>>>,
        align: Align,
        fill: TextStyle,
    },
    Block {
        style: &'a BlockStyle,
        title: Option<&'a BlockTitle>,
        anchor: Option<Key>,
        padding: Sides,
        margin: Sides,
        content_width: usize,
        child: Box<Fitted<'a>>,
    },
    Row(VerticalAlign, Vec<Fitted<'a>>),
    Column(Align, Vec<Fitted<'a>>),
    Grid {
        columns: Vec<usize>,
        rows: Vec<Vec<FittedCell<'a>>>,
    },
    Canvas(&'a Canvas, bool, super::canvas::CanvasRequirements),
    Viewport(&'a Viewport, bool, Box<Fitted<'a>>),
}

/// One cell of a grid, with its text already fitted to its column's width.
#[derive(Debug)]
pub(super) struct FittedCell<'a> {
    padding: Sides,
    align: Align,
    vertical_align: VerticalAlign,
    child: Fitted<'a>,
}

/// Fits every text leaf to its settled width.
///
/// This is the only place a line is ever wrapped or cut, and it happens once
/// per leaf: what a leaf produces depends on its width and its policy, both of
/// which the width phase already fixed.
pub(super) fn fit(widths: Widths<'_>) -> Fitted<'_> {
    let Widths {
        width,
        height_kind,
        height_floor,
        height_projection,
        node,
    } = widths;
    let node = match node {
        WidthNode::Text(text) => {
            let lines = fit_styled_text_lines(text.text, text.area, text.overflow);
            FittedNode::Text {
                lines,
                align: text.align,
                fill: text.fill,
            }
        }
        WidthNode::Block(block) => FittedNode::Block {
            style: block.style,
            title: block.title,
            anchor: block.anchor,
            padding: block.padding,
            margin: block.margin,
            content_width: block.content_width,
            child: Box::new(fit(*block.child)),
        },
        WidthNode::Row(align, children) => {
            FittedNode::Row(align, children.into_iter().map(fit).collect())
        }
        WidthNode::Column(align, children) => {
            FittedNode::Column(align, children.into_iter().map(fit).collect())
        }
        WidthNode::Grid(box_) => FittedNode::Grid {
            columns: box_.columns,
            rows: box_
                .rows
                .into_iter()
                .map(|row| {
                    row.into_iter()
                        .map(|cell| FittedCell {
                            padding: cell.padding,
                            align: cell.align,
                            vertical_align: cell.vertical_align,
                            child: fit(cell.child),
                        })
                        .collect()
                })
                .collect(),
        },
        WidthNode::Canvas(canvas, width_bounded, height) => {
            FittedNode::Canvas(canvas, width_bounded, height)
        }
        WidthNode::Viewport(viewport, width_bounded, child) => {
            FittedNode::Viewport(viewport, width_bounded, Box::new(fit(*child)))
        }
    };
    Fitted {
        width,
        height_kind,
        height_floor,
        height_projection,
        node,
    }
}

/// Fits plain text to a settled width under one overflow policy.
pub(crate) fn fit_text_lines(text: &str, width: Option<usize>, overflow: &Overflow) -> Vec<String> {
    let mut lines = match (width, overflow) {
        (Some(width), Overflow::Wrap) => wrap_text(PrintableLines::new(text), width),
        (Some(width), Overflow::Clip(marker)) => text_lines(text)
            .iter()
            .map(|line| clip_line(line, width, marker))
            .collect(),
        (None, _) => text.lines().map(str::to_owned).collect(),
    };
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn fit_styled_text_lines<'a>(
    text: &'a StyledText,
    width: Option<usize>,
    overflow: &'a Overflow,
) -> Vec<Vec<StyledTextGrapheme<'a>>> {
    let lines = text.lines();
    match (width, overflow) {
        (Some(width), Overflow::Wrap) => wrap_styled_lines(lines, width),
        (Some(width), Overflow::Clip(marker)) => lines
            .into_iter()
            .map(|line| clip_styled_line(line, width, marker))
            .collect(),
        (None, _) => lines,
    }
}

fn clip_styled_line<'a>(
    line: Vec<StyledTextGrapheme<'a>>,
    width: usize,
    marker: &'a str,
) -> Vec<StyledTextGrapheme<'a>> {
    if line.iter().map(|grapheme| grapheme.width()).sum::<usize>() <= width {
        return line;
    }
    if width == 0 {
        return Vec::new();
    }
    let marker = PrintableText::new(marker);
    let marker_width = marker.width();
    let marker = (marker_width < width).then_some(marker);
    let budget = width - marker.map_or(0, PrintableText::width);

    let mut output = Vec::new();
    let mut consumed = 0;
    let mut omitted_style = None;
    for grapheme in line {
        if consumed + grapheme.width() > budget {
            omitted_style = Some(grapheme.style);
            break;
        }
        consumed += grapheme.width();
        output.push(grapheme);
    }
    if let (Some(marker), Some(style)) = (marker, omitted_style) {
        output.extend(
            marker
                .graphemes()
                .map(|grapheme| StyledTextGrapheme { grapheme, style }),
        );
    }
    output
}

/// Cuts one line to `width` cells between graphemes, ending it with `marker`.
///
/// The marker occupies cells of its own, so the text keeps the width less the
/// marker's own. A marker the box cannot hold is dropped: a silent cut is
/// closer to the request than a box filled with the marker alone.
fn clip_line(line: &PrintableText, width: usize, marker: &str) -> String {
    if line.width() <= width {
        return line.as_str().to_owned();
    }
    if width == 0 {
        return String::new();
    }
    let marker = PrintableText::new(marker);
    let marker = if marker.width() < width {
        marker
    } else {
        PrintableText::new("")
    };
    let budget = width - marker.width();

    let mut output = line.truncate(budget).as_str().to_owned();
    output.push_str(marker.as_str());
    output
}

/// One node with both of its sizes settled: the rectangle, before it exists.
#[derive(Debug)]
pub(super) struct Sized<'f> {
    pub width: usize,
    pub height: usize,
    pub node: SizedNode<'f>,
}

#[derive(Debug)]
pub(super) enum SizedNode<'f> {
    Text {
        lines: &'f [Vec<StyledTextGrapheme<'f>>],
        align: Align,
        fill: &'f TextStyle,
    },
    Block {
        style: &'f BlockStyle,
        title: Option<&'f BlockTitle>,
        anchor: Option<Key>,
        padding: Sides,
        margin: Sides,
        content_width: usize,
        /// The rows the box keeps, decided here so assembly only applies it.
        content_height: usize,
        child: Box<Sized<'f>>,
    },
    Row(VerticalAlign, Vec<Sized<'f>>),
    Column(Align, Vec<Sized<'f>>),
    Grid {
        /// One width per column, padding included.
        columns: &'f [usize],
        /// One height per row, the padding of its cells included.
        heights: Vec<usize>,
        rows: Vec<Vec<SizedCell<'f>>>,
    },
    Canvas {
        canvas: &'f Canvas,
        width_bounded: bool,
        height_bounded: bool,
    },
    Viewport {
        viewport: &'f Viewport,
        width_bounded: bool,
        height_bounded: bool,
        child: Box<Sized<'f>>,
    },
}

/// One cell of a grid, with both of its sizes settled.
#[derive(Debug)]
pub(super) struct SizedCell<'f> {
    /// Both axes of the cell's padding, degraded to what its column and its
    /// row could hold.
    pub padding: Sides,
    /// How the cell places itself in a column or a row wider than it is.
    pub align: Align,
    pub vertical_align: VerticalAlign,
    pub child: Sized<'f>,
}

/// Counts the rows every node occupies under `area`.
pub(super) fn heights<'f>(fitted: &'f Fitted<'_>, constraint: Constraint) -> Sized<'f> {
    match &fitted.node {
        FittedNode::Text { lines, align, fill } => Sized {
            width: fitted.width,
            // A text leaf has no height of its own to bound: it produced its
            // rows when it was fitted, and clipping them is a box's rule.
            height: lines.len(),
            node: SizedNode::Text {
                lines,
                align: *align,
                fill,
            },
        },
        FittedNode::Block {
            style,
            title,
            anchor,
            padding,
            margin,
            content_width,
            child,
        } => {
            let border = border_extent(style);
            let (mut padding, mut margin) = (*padding, *margin);

            // 1. Degrade the frame to the area — the vertical half of it; the
            //    width phase decided the other half.
            let down = degrade(
                constraint.cap,
                vertical(margin),
                border.height(),
                vertical(padding),
                0,
            );
            if down.margin {
                margin.top = 0;
                margin.bottom = 0;
            }
            if down.padding {
                padding.top = 0;
                padding.bottom = 0;
            }

            let axis = height_axis(
                style,
                border.height() + usize::from(padding.top) + usize::from(padding.bottom),
            );
            let box_constraint = Constraint {
                reference: constraint
                    .reference
                    .map(|area| area.saturating_sub(vertical(margin))),
                cap: constraint
                    .cap
                    .map(|area| area.saturating_sub(vertical(margin))),
            };

            // 2. The content, under what the box leaves it, and then the used
            //    height by the same clamp the width took — now that the rows
            //    are known, because the content is what decided how many.
            let local_bound =
                matches!(axis.length, Some(crate::Length::Cells(_))) || axis.max.is_some();
            let mut child_constraint = axis.content_constraint(box_constraint);
            if !local_bound && constraint.reference.is_none() {
                child_constraint.reference = None;
            }
            if !local_bound && constraint.cap.is_none() {
                child_constraint.cap = None;
            }
            let child = heights(child, child_constraint);
            let used = axis.used(box_constraint, child.height, 0);

            Sized {
                width: fitted.width,
                height: used + vertical(margin),
                node: SizedNode::Block {
                    style,
                    title: *title,
                    anchor: *anchor,
                    padding,
                    margin,
                    content_width: *content_width,
                    content_height: used - axis.frame,
                    child: Box::new(child),
                },
            }
        }
        FittedNode::Row(align, children) => {
            // The cross axis has nothing to divide: every child receives the
            // row's own height, and a `Fill` there stretches to it.
            let children: Vec<Sized<'f>> = children
                .iter()
                .map(|child| heights(child, constraint))
                .collect();
            Sized {
                width: fitted.width,
                height: children.iter().map(|child| child.height).max().unwrap_or(0),
                node: SizedNode::Row(*align, children),
            }
        }
        FittedNode::Column(align, children) => {
            let children = divide(children, constraint);
            Sized {
                width: fitted.width,
                height: children.iter().map(|child| child.height).sum(),
                node: SizedNode::Column(*align, children),
            }
        }
        FittedNode::Grid { columns, rows } => {
            let (heights, rows) = rows_of(rows, constraint);
            Sized {
                width: fitted.width,
                height: heights.iter().sum(),
                node: SizedNode::Grid {
                    columns,
                    heights,
                    rows,
                },
            }
        }
        FittedNode::Canvas(canvas, width_bounded, height) => Sized {
            width: fitted.width,
            height: constraint
                .reference
                .map(|area| area.max(height.floor()))
                .unwrap_or(height.demand()),
            node: SizedNode::Canvas {
                canvas,
                width_bounded: *width_bounded,
                height_bounded: constraint.reference.is_some(),
            },
        },
        FittedNode::Viewport(viewport, width_bounded, child) => {
            let vertical = viewport.vertical_projection().is_some();
            let extent = constraint.reference;
            let child_constraint = if vertical {
                Constraint {
                    reference: extent,
                    cap: None,
                }
            } else {
                constraint
            };
            let child = heights(child, child_constraint);
            Sized {
                width: fitted.width,
                height: if vertical {
                    extent.unwrap_or(child.height)
                } else {
                    child.height
                },
                node: SizedNode::Viewport {
                    viewport,
                    width_bounded: *width_bounded,
                    height_bounded: extent.is_some(),
                    child: Box::new(child),
                },
            }
        }
    }
}

/// Divides a grid's height among its rows and settles every cell in one.
///
/// A row demands the greatest height among its cells, as a `Row` does, and the
/// rows then divide the grid's height by the rule a `Column` applies to its
/// children — the same claim, the same shrink, the same second visit for a row
/// assigned less than it asked for. A row wider in rows than its assignment is
/// what a cell could not give up, and it grows the grid rather than being cut,
/// exactly as an over-wide cell grows its column.
fn rows_of<'f>(
    rows: &'f [Vec<FittedCell<'_>>],
    constraint: Constraint,
) -> (Vec<usize>, Vec<Vec<SizedCell<'f>>>) {
    let asked: Vec<Vec<Sized<'f>>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| heights(&cell.child, Constraint::unbounded()))
                .collect()
        })
        .collect();
    let claims: Vec<Claim> = rows
        .iter()
        .zip(&asked)
        .map(|(row, asked)| Claim {
            kind: if row.iter().any(|cell| cell.child.height_projection) {
                Kind::fill(1)
            } else {
                Kind::Auto
            },
            demand: row
                .iter()
                .zip(asked)
                .map(|(cell, sized)| sized.height + vertical(cell.padding))
                .max()
                .unwrap_or(0),
            floor: row
                .iter()
                .map(|cell| cell.child.height_floor + vertical(cell.padding))
                .max()
                .unwrap_or(0),
        })
        .collect();
    let shares = distribute_constraints(constraint.reference, constraint.cap, &claims);

    let mut settled_heights = shares.clone();
    let mut settled = Vec::with_capacity(rows.len());
    for ((index, row), asked) in rows.iter().enumerate().zip(asked) {
        let share = shares[index];
        let mut cells = Vec::with_capacity(row.len());
        for (cell, asked) in row.iter().zip(asked) {
            let mut padding = cell.padding;
            let down = degrade(
                Some(share),
                0,
                0,
                vertical(padding),
                cell.child.height_floor,
            );
            if down.padding {
                padding.top = 0;
                padding.bottom = 0;
            }
            let inner = share.saturating_sub(vertical(padding));
            let child = if constraint.cap.is_none() && asked.height <= inner {
                asked
            } else {
                heights(&cell.child, Constraint::established(inner))
            };
            settled_heights[index] = settled_heights[index].max(child.height + vertical(padding));
            cells.push(SizedCell {
                padding,
                align: cell.align,
                vertical_align: cell.vertical_align,
                child,
            });
        }
        settled.push(cells);
    }
    (settled_heights, settled)
}

/// Divides `area` among a column's children and settles each at its share.
///
/// A height demand cannot be read off a style the way a width can: it depends
/// on how the content fitted at the cross-axis width, so the demand *is* a
/// height resolution. Because a resolution is a pure function of the node and
/// the area, that answer says nothing about a sibling — which is why the model
/// permits the question (`docs/design/layout-resolution.md`).
///
/// The answer is kept, so the ordinary case settles each child once. Only a
/// child a deficit assigns *less* than it asked for is settled again at that
/// assignment, which closes its frame at the smaller size instead of cutting
/// it. That second visit is arithmetic over lines that were fitted before this
/// function ran; nothing is fitted twice.
fn divide<'f>(children: &'f [Fitted<'_>], constraint: Constraint) -> Vec<Sized<'f>> {
    if constraint.reference.is_none() {
        return children
            .iter()
            .map(|child| heights(child, Constraint::unbounded()))
            .collect();
    }
    let asked: Vec<Option<Sized<'f>>> = children
        .iter()
        .map(|child| match child.height_kind {
            Kind::Fill(_) => None,
            _ => Some(heights(child, Constraint::unbounded())),
        })
        .collect();
    let claims: Vec<Claim> = children
        .iter()
        .zip(&asked)
        .map(|(child, sized)| Claim {
            kind: child.height_kind,
            demand: sized.as_ref().map_or(0, |sized| sized.height),
            floor: child.height_floor,
        })
        .collect();

    children
        .iter()
        .zip(asked)
        .zip(distribute_constraints(
            constraint.reference,
            constraint.cap,
            &claims,
        ))
        .map(|((child, asked), height)| match asked {
            Some(sized) if constraint.cap.is_none() && sized.height <= height => sized,
            _ => heights(child, Constraint::established(height)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clip_keeps_the_marker_inside_the_width() {
        assert_eq!(clip_line(PrintableText::new("hello world"), 5, ""), "hello");
        assert_eq!(
            clip_line(PrintableText::new("hello world"), 5, "…"),
            "hell…"
        );
        assert_eq!(
            clip_line(PrintableText::new("hello world"), 5, "..."),
            "he..."
        );
        assert_eq!(
            clip_line(PrintableText::new("hello"), 9, "…"),
            "hello",
            "a line that fits is untouched"
        );
    }

    #[test]
    fn a_clip_never_splits_a_grapheme_and_measures_the_marker_in_cells() {
        assert_eq!(
            clip_line(PrintableText::new("日本語"), 5, ""),
            "日本",
            "the third would straddle"
        );
        assert_eq!(
            clip_line(PrintableText::new("日本語"), 4, "→"),
            "日→",
            "a wide marker costs two"
        );
        assert_eq!(
            clip_line(PrintableText::new("e\u{301}xyz"), 2, ""),
            "e\u{301}x"
        );
    }

    #[test]
    fn a_marker_that_cannot_fit_is_dropped() {
        assert_eq!(
            clip_line(PrintableText::new("hello"), 3, "..."),
            "hel",
            "no room for content"
        );
        assert_eq!(
            clip_line(PrintableText::new("hello"), 1, "…"),
            "h",
            "the marker fills the box"
        );
        assert_eq!(
            clip_line(PrintableText::new("hello"), 0, "…"),
            "",
            "nothing fits at all"
        );
    }
}
