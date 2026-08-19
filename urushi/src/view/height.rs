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

use crate::text::{PrintableLines, PrintableText, wrap_text};
use crate::{Align, BlockStyle, Key, Overflow, Sides, TextStyle, VerticalAlign};

use super::sizing::{
    Claim, Kind, border_extent, degrade, distribute, height_axis, text_lines, vertical,
};
use super::width::{WidthNode, Widths};

/// One node with its width settled and its text already fitted to it.
#[derive(Debug)]
pub(super) struct Fitted<'a> {
    pub width: usize,
    height_kind: Kind,
    height_floor: usize,
    node: FittedNode<'a>,
}

#[derive(Debug)]
enum FittedNode<'a> {
    Text {
        lines: Vec<String>,
        style: &'a TextStyle,
        align: Align,
        fill: &'a TextStyle,
    },
    Block {
        style: &'a BlockStyle,
        anchor: Option<Key>,
        padding: Sides,
        margin: Sides,
        content_width: usize,
        child: Box<Fitted<'a>>,
    },
    Row(VerticalAlign, Vec<Fitted<'a>>),
    Column(Align, Vec<Fitted<'a>>),
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
        node,
    } = widths;
    let node = match node {
        WidthNode::Text(text) => {
            let mut lines: Vec<String> = match (text.target, text.overflow) {
                (Some(width), Overflow::Wrap) => wrap_text(PrintableLines::new(text.text), width),
                (Some(width), Overflow::Clip(marker)) => text_lines(text.text)
                    .iter()
                    .map(|line| clip_line(line, width, marker))
                    .collect(),
                (None, _) => text.text.lines().map(str::to_owned).collect(),
            };
            if lines.is_empty() {
                lines.push(String::new());
            }
            FittedNode::Text {
                lines,
                style: text.style,
                align: text.align,
                fill: text.fill,
            }
        }
        WidthNode::Block(block) => FittedNode::Block {
            style: block.style,
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
    };
    Fitted {
        width,
        height_kind,
        height_floor,
        node,
    }
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
        lines: &'f [String],
        style: &'f TextStyle,
        align: Align,
        fill: &'f TextStyle,
    },
    Block {
        style: &'f BlockStyle,
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
}

/// Counts the rows every node occupies under `area`.
pub(super) fn heights<'f>(fitted: &'f Fitted<'_>, area: Option<usize>) -> Sized<'f> {
    match &fitted.node {
        FittedNode::Text {
            lines,
            style,
            align,
            fill,
        } => Sized {
            width: fitted.width,
            // A text leaf has no height of its own to bound: it produced its
            // rows when it was fitted, and clipping them is a box's rule.
            height: lines.len(),
            node: SizedNode::Text {
                lines,
                style,
                align: *align,
                fill,
            },
        },
        FittedNode::Block {
            style,
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
                area,
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
            let box_height = area.map(|area| area.saturating_sub(vertical(margin)));

            // 2. The content, under what the box leaves it, and then the used
            //    height by the same clamp the width took — now that the rows
            //    are known, because the content is what decided how many.
            let child = heights(child, axis.content_bound(box_height));
            let used = axis.used(box_height, child.height, 0);

            Sized {
                width: fitted.width,
                height: used + vertical(margin),
                node: SizedNode::Block {
                    style,
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
            let children: Vec<Sized<'f>> =
                children.iter().map(|child| heights(child, area)).collect();
            Sized {
                width: fitted.width,
                height: children.iter().map(|child| child.height).max().unwrap_or(0),
                node: SizedNode::Row(*align, children),
            }
        }
        FittedNode::Column(align, children) => {
            let children = match area {
                None => children
                    .iter()
                    .map(|child| heights(child, None))
                    .collect::<Vec<_>>(),
                Some(area) => divide(children, area),
            };
            Sized {
                width: fitted.width,
                height: children.iter().map(|child| child.height).sum(),
                node: SizedNode::Column(*align, children),
            }
        }
    }
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
fn divide<'f>(children: &'f [Fitted<'_>], area: usize) -> Vec<Sized<'f>> {
    let asked: Vec<Option<Sized<'f>>> = children
        .iter()
        .map(|child| match child.height_kind {
            Kind::Fill(_) => None,
            _ => Some(heights(child, None)),
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
        .zip(distribute(area, &claims))
        .map(|((child, asked), height)| match asked {
            Some(sized) if sized.height <= height => sized,
            _ => heights(child, Some(height)),
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
