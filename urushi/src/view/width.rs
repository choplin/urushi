//! The width phase: two passes that settle every node's width.
//!
//! Width comes first because wrapping needs a width to wrap to. That makes the
//! dependency one-way — width, then height, then assembly — and this module is
//! the end of it that must not look forward: it never wraps a line, never
//! counts a row, and reaches nothing the height phase or assembly owns. The
//! rules it applies are `docs/design/box-sizing.md`'s and
//! `docs/design/area-sharing.md`'s, in the order `docs/design/layout-resolution.md`
//! fixes.
//!
//! It runs in two passes over the tree, each visiting a node once.
//!
//! - Pass A, bottom-up, fills [`Metrics`]: the width a node takes unbounded,
//!   the width below which it cannot go, the height below which it cannot be
//!   shrunk, and whether anything inside it spans its area. Each is `O(1)` from
//!   the children's, so the whole pass is one walk.
//! - Pass B, top-down, hands each node an area and reads pass A's numbers to
//!   settle the width it uses, descending into every child exactly once.
//!
//! Asking a subtree for its extent used to be a walk of its own, so a chain of
//! blocks measured the same leaves once per level. Folding those questions into
//! a table is what makes the phase linear; nothing about what they answer
//! changed.

use crate::{
    Align, BlockStyle, BlockTitle, Canvas, GridStyle, Key, Length, Overflow, Sides, StyledText,
    TextStyle, VerticalAlign, View,
};

use super::grid;

use super::sizing::{
    Claim, Kind, border_extent, degrade, distribute, horizontal, kind_of, vertical, width_axis,
};

/// The overflow policy a text leaf outside a block is fitted under.
static WRAP: Overflow = Overflow::Wrap;

/// What a subtree fixes about its own size, whatever area it is later given.
///
/// These are the numbers the clamp needs before the content is laid out. They
/// depend on the subtree alone, so pass A can read them off the tree once and
/// pass B can consume them without walking anything again.
#[derive(Debug)]
pub(super) struct Metrics {
    /// The width the node takes when no area bounds it — its max-content size.
    natural: usize,
    /// The width below which it cannot go without splitting a grapheme.
    floor: usize,
    /// The height below which it cannot be shrunk.
    height_floor: usize,
    /// Whether anything inside it spans its area rather than taking a size.
    ///
    /// A `Fill` length needs an area to divide, so a box containing one spans
    /// its own extent. The height axis gets this for free — a box sizes its
    /// height after its content — but the width is decided before the content
    /// is laid out, which is why the width axis has to ask.
    fills: bool,
    width_kind: Kind,
    height_kind: Kind,
    /// One entry per child; a block has exactly one.
    children: Vec<Metrics>,
}

/// Pass A: the bottom-up walk that fills [`Metrics`] for every node.
fn metrics(view: &View) -> Metrics {
    match view {
        View::Text(text) => {
            let lines = text.lines();
            Metrics {
                natural: lines
                    .iter()
                    .map(|line| line.iter().map(|grapheme| grapheme.width()).sum())
                    .max()
                    .unwrap_or(0),
                floor: lines
                    .iter()
                    .flat_map(|line| line.iter().map(|grapheme| grapheme.width()))
                    .max()
                    .unwrap_or(0),
                height_floor: 0,
                fills: false,
                width_kind: Kind::Auto,
                height_kind: Kind::Auto,
                children: Vec::new(),
            }
        }
        View::Canvas(canvas) => {
            let width = canvas.width_requirements();
            let kind = if canvas.uses_viewport_sizing() {
                Kind::fill(1)
            } else {
                Kind::Auto
            };
            Metrics {
                natural: width.demand(),
                floor: width.floor(),
                height_floor: 0,
                fills: canvas.uses_viewport_sizing(),
                width_kind: kind,
                height_kind: kind,
                children: Vec::new(),
            }
        }
        // An anchor measures as the block it is: the key says where to report,
        // never how large to be.
        View::Block(style, title, child) | View::AnchorBlock(_, style, title, child) => {
            let inner = metrics(child);
            let frame = style.frame_size();
            let margin = style.margin_sides();
            let intrinsic = inner
                .natural
                .max(title_width(style, title.as_ref()).saturating_sub(frame.width()));
            // The undegraded frame: no area is known yet, and degradation is
            // what an area does to a frame.
            let natural = width_axis(style, frame.width()).used(None, intrinsic, inner.floor)
                + horizontal(margin);
            let floor = style
                .minimum_width()
                .map_or(0, usize::from)
                .max(frame.width() + inner.floor)
                + horizontal(margin);
            let height_floor = style
                .minimum_height()
                .map_or(0, usize::from)
                .max(frame.height() + inner.height_floor)
                + vertical(margin);
            Metrics {
                natural,
                floor,
                height_floor,
                fills: match style.width_length() {
                    Some(Length::Fill(_)) => true,
                    Some(Length::Cells(_)) => false,
                    None => inner.fills,
                },
                width_kind: kind_of(style.width_length()),
                height_kind: kind_of(style.height_length()),
                children: vec![inner],
            }
        }
        View::Row(_, children) => {
            let children: Vec<Metrics> = children.iter().map(metrics).collect();
            Metrics {
                natural: children.iter().map(|child| child.natural).sum(),
                floor: children.iter().map(|child| child.floor).sum(),
                height_floor: children
                    .iter()
                    .map(|child| child.height_floor)
                    .max()
                    .unwrap_or(0),
                fills: children.iter().any(|child| child.fills),
                width_kind: Kind::Auto,
                height_kind: Kind::Auto,
                children,
            }
        }
        View::Column(_, children) => {
            let children: Vec<Metrics> = children.iter().map(metrics).collect();
            Metrics {
                natural: children
                    .iter()
                    .map(|child| child.natural)
                    .max()
                    .unwrap_or(0),
                floor: children.iter().map(|child| child.floor).max().unwrap_or(0),
                height_floor: children.iter().map(|child| child.height_floor).sum(),
                fills: children.iter().any(|child| child.fills),
                width_kind: Kind::Auto,
                height_kind: Kind::Auto,
                children,
            }
        }
        View::Grid(style, rows) => {
            let columns = grid::columns(rows);
            let mut cells = Vec::with_capacity(rows.len() * columns);
            for row in 0..rows.len() {
                for column in 0..columns {
                    cells.push(metrics(grid::cell(rows, row, column)));
                }
            }
            let claims = column_claims(style, rows, &cells, columns);
            Metrics {
                natural: claims.iter().map(|claim| claim.demand).sum(),
                floor: claims.iter().map(|claim| claim.floor).sum(),
                height_floor: (0..rows.len())
                    .map(|row| row_floor(style, rows, &cells, columns, row))
                    .sum(),
                fills: cells.iter().any(|cell| cell.fills)
                    || (0..columns)
                        .any(|column| matches!(style.column_length(column), Some(Length::Fill(_)))),
                width_kind: Kind::Auto,
                height_kind: Kind::Auto,
                children: cells,
            }
        }
    }
}

/// The claim each column of a grid makes on the grid's width.
///
/// The kind comes from the column's stated [`Length`], the demand and the
/// floor from the cells beneath it, each measured with the padding that cell
/// takes. Only per-cell metrics are read, so a column's claim never depends on
/// what a sibling resolved to — the no-solver boundary
/// `docs/design/layout-resolution.md` fixes.
fn column_claims(
    style: &GridStyle,
    rows: &[Vec<View>],
    cells: &[Metrics],
    columns: usize,
) -> Vec<Claim> {
    (0..columns)
        .map(|column| {
            let (mut demand, mut floor) = (0, 0);
            for row in 0..rows.len() {
                let padding = horizontal(grid::cell_padding(style, grid::cell(rows, row, column)));
                let cell = &cells[row * columns + column];
                demand = demand.max(cell.natural + padding);
                floor = floor.max(cell.floor + padding);
            }
            let length = style.column_length(column);
            Claim {
                kind: kind_of(length),
                // A stated size is a demand of its own, and the floor still
                // wins over it — the clamp a `Row` child applies to itself,
                // applied here from outside, because a cell cannot know the
                // length its column states.
                demand: match length {
                    Some(Length::Cells(cells)) => usize::from(cells).max(floor),
                    _ => demand,
                },
                floor,
            }
        })
        .collect()
}

/// The height below which one row of a grid cannot be shrunk.
fn row_floor(
    style: &GridStyle,
    rows: &[Vec<View>],
    cells: &[Metrics],
    columns: usize,
    row: usize,
) -> usize {
    (0..columns)
        .map(|column| {
            let padding = vertical(grid::cell_padding(style, grid::cell(rows, row, column)));
            cells[row * columns + column].height_floor + padding
        })
        .max()
        .unwrap_or(0)
}

/// One node with its width settled, and what the later phases still need.
#[derive(Debug)]
pub(super) struct Widths<'a> {
    /// The outer width this node occupies, margin included.
    pub width: usize,
    /// How the node claims space on an enclosing `Column`'s axis.
    pub height_kind: Kind,
    /// The height below which the node cannot be shrunk.
    pub height_floor: usize,
    pub node: WidthNode<'a>,
}

#[derive(Debug)]
pub(super) enum WidthNode<'a> {
    Text(TextBox<'a>),
    Block(BlockBox<'a>),
    Row(VerticalAlign, Vec<Widths<'a>>),
    Column(Align, Vec<Widths<'a>>),
    Grid(GridBox<'a>),
    Canvas(&'a Canvas, bool, super::canvas::CanvasRequirements),
}

/// A grid whose column widths are settled.
///
/// `columns` is one width per column, padding included, and every cell of
/// column `j` was resolved under `columns[j]` less its own padding. The two
/// cannot disagree: the second is what the first was handed down as.
#[derive(Debug)]
pub(super) struct GridBox<'a> {
    pub columns: Vec<usize>,
    pub rows: Vec<Vec<GridCell<'a>>>,
}

/// One cell of a grid, with the padding it takes around it.
#[derive(Debug)]
pub(super) struct GridCell<'a> {
    /// The cell's own padding where it states one, the grid's otherwise. Its
    /// horizontal sides carry this phase's degradation; the vertical ones are
    /// still as stated, because only the height phase knows the area that
    /// degrades those.
    pub padding: Sides,
    /// How the cell places itself in a column or a row wider than it is. A
    /// cell that is not a box states none of this and takes the defaults.
    pub align: Align,
    pub vertical_align: VerticalAlign,
    pub child: Widths<'a>,
}

/// A text leaf and everything needed to fit its lines, once a width exists.
#[derive(Debug)]
pub(super) struct TextBox<'a> {
    pub text: &'a StyledText,
    /// The area the leaf was given, which is what the policy fits against —
    /// not the width it resolved to, which a grapheme it cannot split may
    /// widen.
    pub area: Option<usize>,
    pub overflow: &'a Overflow,
    /// Per-line alignment and the style filling the gap, both supplied by an
    /// enclosing block.
    pub align: Align,
    pub fill: TextStyle,
}

/// A block whose horizontal frame and content width are settled.
///
/// `padding` and `margin` carry the horizontal degradation this phase decided;
/// their vertical sides are still as the style stated them, because only the
/// height phase knows the area that degrades those.
#[derive(Debug)]
pub(super) struct BlockBox<'a> {
    pub style: &'a BlockStyle,
    pub title: Option<&'a BlockTitle>,
    /// The key to report this box's content rectangle under, if it is an
    /// anchor.
    pub anchor: Option<Key>,
    pub padding: Sides,
    pub margin: Sides,
    pub content_width: usize,
    pub child: Box<Widths<'a>>,
}

/// What an enclosing block fits a text leaf it directly contains under.
#[derive(Clone, Copy)]
struct TextFit<'a> {
    align: Align,
    fill: &'a TextStyle,
    overflow: &'a Overflow,
}

/// Settles every node's width under `area`.
pub(super) fn widths(view: &View, area: Option<usize>) -> Widths<'_> {
    place(view, &metrics(view), area, area.is_some(), None)
}

/// Pass B: hands `area` down and reads pass A's numbers to settle each width.
fn place<'a>(
    view: &'a View,
    metrics: &Metrics,
    area: Option<usize>,
    bounded: bool,
    fit: Option<TextFit<'a>>,
) -> Widths<'a> {
    match view {
        View::Text(text) => {
            let (align, fill, overflow) = fit.map_or_else(
                || {
                    // Preserve the uniform leaf's fill behavior. Mixed text
                    // has no one style that can own geometry, so its bare
                    // alignment gap is terminal-default.
                    (
                        Align::Left,
                        text.uniform_style().cloned().unwrap_or_else(TextStyle::new),
                        &WRAP,
                    )
                },
                |fit| (fit.align, fit.fill.clone(), fit.overflow),
            );
            Widths {
                width: text_width(area, overflow, metrics),
                height_kind: metrics.height_kind,
                height_floor: metrics.height_floor,
                node: WidthNode::Text(TextBox {
                    text,
                    area,
                    overflow,
                    align,
                    fill,
                }),
            }
        }
        View::Canvas(canvas) => {
            let width = area
                .map(|area| area.max(metrics.floor))
                .unwrap_or(metrics.natural);
            let height = canvas.height_requirements(width);
            Widths {
                width,
                height_kind: metrics.height_kind,
                height_floor: height.floor(),
                node: WidthNode::Canvas(canvas, bounded, height),
            }
        }
        View::Block(style, title, child) | View::AnchorBlock(_, style, title, child) => {
            let inner = &metrics.children[0];
            let border = border_extent(style);
            let mut padding = style.padding_sides();
            let mut margin = style.margin_sides();

            // 1. Degrade the frame to the area — the horizontal half of it.
            //    The axes degrade independently, so the height phase decides
            //    the other half against the area it is given.
            let across = degrade(
                area,
                horizontal(margin),
                border.width(),
                horizontal(padding),
                inner.floor,
            );
            if across.margin {
                margin.left = 0;
                margin.right = 0;
            }
            if across.padding {
                padding.left = 0;
                padding.right = 0;
            }

            // 2. Resolve the width by the clamp. The content has not been laid
            //    out yet: the width comes from the intrinsic width, the bounds,
            //    and the area, never from what wrapping is about to do.
            let axis = width_axis(
                style,
                border.width() + usize::from(padding.left) + usize::from(padding.right),
            );
            // Every sizing property measures the box; margin lies outside it.
            let box_width = area.map(|area| area.saturating_sub(horizontal(margin)));
            let title_intrinsic = title_width(style, title.as_ref()).saturating_sub(axis.frame);
            let intrinsic = if inner.fills
                && let Some(area) = box_width
            {
                area.saturating_sub(axis.frame)
            } else {
                inner.natural.max(title_intrinsic)
            };
            let used = axis.used(box_width, intrinsic, inner.floor);
            let content_width = used - axis.frame;

            // 3. The child, under what the box leaves it. A directly contained
            //    text leaf meets the block's overflow policy here; any other
            //    child absorbs its own overflow when it resolves.
            let child = place(
                child,
                inner,
                Some(content_width),
                bounded || matches!(axis.length, Some(Length::Cells(_))) || axis.max.is_some(),
                Some(TextFit {
                    align: style.horizontal_alignment(),
                    fill: style.text(),
                    overflow: style.overflow_policy(),
                }),
            );
            debug_assert!(
                child.width <= content_width,
                "a child never resolves wider than the box that assigned it: \
                 {} > {content_width}",
                child.width
            );
            let height_floor = style
                .minimum_height()
                .map_or(0, usize::from)
                .max(border.height() + vertical(padding) + child.height_floor)
                + vertical(margin);

            Widths {
                width: used + horizontal(margin),
                height_kind: metrics.height_kind,
                height_floor,
                node: WidthNode::Block(BlockBox {
                    style,
                    title: title.as_ref(),
                    anchor: match view {
                        View::AnchorBlock(key, _, _, _) => Some(*key),
                        _ => None,
                    },
                    padding,
                    margin,
                    content_width,
                    child: Box::new(child),
                }),
            }
        }
        View::Row(align, children) => {
            // The main axis is divided: stated widths, then intrinsic ones,
            // then `Fill` weights over what remains. With no width to divide —
            // under `measure` — every child takes its intrinsic width.
            let shares = area.map(|area| {
                let claims: Vec<Claim> = metrics
                    .children
                    .iter()
                    .map(|child| Claim {
                        kind: child.width_kind,
                        demand: child.natural,
                        floor: child.floor,
                    })
                    .collect();
                distribute(area, &claims)
            });
            let children: Vec<Widths<'a>> = children
                .iter()
                .zip(&metrics.children)
                .enumerate()
                .map(|(index, (child, inner))| {
                    place(
                        child,
                        inner,
                        shares.as_ref().map(|share| share[index]),
                        bounded,
                        None,
                    )
                })
                .collect();
            Widths {
                // Nothing is renegotiated: a child that resolves narrower than
                // its assignment leaves the remainder unused.
                width: children.iter().map(|child| child.width).sum(),
                height_kind: metrics.height_kind,
                height_floor: children
                    .iter()
                    .map(|child| child.height_floor)
                    .max()
                    .unwrap_or(0),
                node: WidthNode::Row(*align, children),
            }
        }
        View::Column(align, children) => {
            // The cross axis has nothing to divide: every child receives the
            // column's own width.
            let children: Vec<Widths<'a>> = children
                .iter()
                .zip(&metrics.children)
                .map(|(child, inner)| place(child, inner, area, bounded, None))
                .collect();
            Widths {
                width: children.iter().map(|child| child.width).max().unwrap_or(0),
                height_kind: metrics.height_kind,
                height_floor: children.iter().map(|child| child.height_floor).sum(),
                node: WidthNode::Column(*align, children),
            }
        }
        View::Grid(style, rows) => {
            let columns = grid::columns(rows);
            let claims = column_claims(style, rows, &metrics.children, columns);

            // One `distribute` settles every column, and column j's width is
            // what every cell of column j resolves under.
            let mut widths: Vec<usize> = match area {
                Some(area) => distribute(area, &claims),
                None => claims.iter().map(|claim| claim.demand).collect(),
            };

            let mut placed = Vec::with_capacity(rows.len());
            for row in 0..rows.len() {
                let mut cells = Vec::with_capacity(columns);
                for (column, width) in widths.iter_mut().enumerate() {
                    let view = grid::cell(rows, row, column);
                    let inner = &metrics.children[row * columns + column];
                    let share = *width;

                    let mut padding = grid::cell_padding(style, view);
                    let across = degrade(Some(share), 0, 0, horizontal(padding), inner.floor);
                    if across.padding {
                        padding.left = 0;
                        padding.right = 0;
                    }

                    let child = place(
                        view,
                        inner,
                        Some(share.saturating_sub(horizontal(padding))),
                        bounded || matches!(claims[column].kind, Kind::Cells),
                        None,
                    );
                    // A cell wider than its share is a grapheme that could not
                    // be split, and it widens the column exactly as such a
                    // child widens a `Row`. Every cell of the column is placed
                    // in the wider one, so they still line up.
                    *width = (*width).max(child.width + horizontal(padding));
                    let (align, vertical_align) = grid::cell_alignment(view);
                    cells.push(GridCell {
                        padding,
                        align,
                        vertical_align,
                        child,
                    });
                }
                placed.push(cells);
            }

            let height_floor = placed
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.child.height_floor + vertical(cell.padding))
                        .max()
                        .unwrap_or(0)
                })
                .sum();

            Widths {
                width: widths.iter().sum(),
                height_kind: metrics.height_kind,
                height_floor,
                node: WidthNode::Grid(GridBox {
                    columns: widths,
                    rows: placed,
                }),
            }
        }
    }
}

fn title_width(style: &BlockStyle, title: Option<&BlockTitle>) -> usize {
    let Some(title) = title else {
        return 0;
    };
    debug_assert!(
        style.border_kind().is_some() && style.is_border_top_enabled(),
        "a titled block requires an enabled top border"
    );
    if style.border_kind().is_none() || !style.is_border_top_enabled() {
        return 0;
    }

    if title.text().as_str().is_empty() {
        return 0;
    }
    let text = title
        .text()
        .lines()
        .first()
        .map(|line| line.iter().map(|grapheme| grapheme.width()).sum())
        .unwrap_or(0);
    let border = border_extent(style).width();
    border
        .saturating_add(text)
        .saturating_add(usize::from(title.horizontal_padding()).saturating_mul(2))
}

/// The width a text leaf uses, without fitting a single line.
///
/// Fitting cannot widen a leaf past what the policy and the graphemes already
/// fix, and the area only caps: an area wider than the text never widens it, so
/// both policies stay at or below the natural width. `Overflow::Clip` cuts to
/// the area, with no floor, because a clip may cut a grapheme it cannot split.
/// Under `Overflow::Wrap` a line is at most the area, except where one grapheme
/// is wider than that — and a grapheme that cannot be split is exactly the
/// floor pass A measured. With no area at all, the leaf takes its own lines.
///
/// This is what keeps the phase order honest: the width of a text leaf is
/// decided from the text, never from what wrapping is about to do with it.
fn text_width(area: Option<usize>, overflow: &Overflow, metrics: &Metrics) -> usize {
    match (area, overflow) {
        (None, _) => metrics.natural,
        (Some(area), Overflow::Wrap) => area.min(metrics.natural).max(metrics.floor),
        (Some(area), Overflow::Clip(_)) => area.min(metrics.natural),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Border, TextStyle};

    fn text(content: &str) -> View {
        View::text(content, TextStyle::new())
    }

    fn natural(view: &View) -> usize {
        metrics(view).natural
    }

    fn floor(view: &View) -> usize {
        metrics(view).floor
    }

    fn fills(view: &View) -> bool {
        metrics(view).fills
    }

    #[test]
    fn text_is_measured_by_its_widest_line_and_its_widest_grapheme() {
        assert_eq!(natural(&text("ab\nabcd\nabc")), 4);
        assert_eq!(floor(&text("ab\nabcd\nabc")), 1);

        assert_eq!(natural(&text("日本語")), 6);
        assert_eq!(
            floor(&text("日本語")),
            2,
            "a wide character cannot be split"
        );
        assert_eq!(floor(&text("a👩‍💻")), 2, "nor can a cluster");

        assert_eq!(natural(&text("")), 0);
        assert_eq!(floor(&text("")), 0);
    }

    #[test]
    fn a_block_measures_its_frame_bounds_and_margin() {
        let bordered = View::block(
            BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
            text("abc"),
        );
        assert_eq!(natural(&bordered), 7, "3 content + 2 padding + 2 border");
        assert_eq!(floor(&bordered), 5, "one grapheme plus the frame");

        let bounded = View::block(BlockStyle::new().max_width(2), text("abcd"));
        assert_eq!(natural(&bounded), 2, "the maximum caps the measure");

        let floored = View::block(BlockStyle::new().min_width(9), text("abcd"));
        assert_eq!(natural(&floored), 9);
        assert_eq!(floor(&floored), 9, "the minimum is a floor too");

        let spaced = View::block(BlockStyle::new().margin((0, 2)), text("ab"));
        assert_eq!(natural(&spaced), 6, "margin lies outside the box");
        assert_eq!(floor(&spaced), 5);
    }

    #[test]
    fn a_row_sums_its_children_and_a_column_takes_the_widest() {
        let children = [text("abcd"), text("日本")];
        let row = View::row(VerticalAlign::Top, children.clone());
        let column = View::column(Align::Left, children);

        assert_eq!(natural(&row), 8);
        assert_eq!(floor(&row), 3, "one grapheme from each child");
        assert_eq!(natural(&column), 4);
        assert_eq!(floor(&column), 2);
    }

    #[test]
    fn a_height_floor_is_the_frame_and_a_width_floor_is_the_content() {
        let bordered = View::block(BlockStyle::new().border(Border::NORMAL), text("abc"));

        assert_eq!(metrics(&text("abc")).height_floor, 0, "a row can be absent");
        assert_eq!(metrics(&bordered).height_floor, 2, "the frame cannot");
        assert_eq!(floor(&bordered), 3, "one grapheme plus it");

        let floored = View::block(BlockStyle::new().min_height(5), text("a"));
        assert_eq!(metrics(&floored).height_floor, 5);

        let spaced = View::block(BlockStyle::new().margin((1, 0)), text("a"));
        assert_eq!(metrics(&spaced).height_floor, 2, "margin lies outside");
    }

    #[test]
    fn a_view_fills_its_area_when_anything_inside_it_does() {
        let filling = View::block(BlockStyle::new().width(Length::fill(1)), text("a"));

        assert!(!fills(&text("a")));
        assert!(fills(&filling));
        assert!(
            fills(&View::block(BlockStyle::new(), filling.clone())),
            "an automatic box inherits its content's appetite"
        );
        assert!(
            !fills(&View::block(BlockStyle::new().width(4), filling.clone())),
            "a stated size settles the box, whatever it contains"
        );
        assert!(fills(&View::row(VerticalAlign::Top, [text("a"), filling])));
    }

    #[test]
    fn a_text_leaf_is_capped_by_its_area_and_never_widened_by_it() {
        let wide = metrics(&text("日本"));

        assert_eq!(text_width(None, &Overflow::Wrap, &wide), 4, "its own lines");
        assert_eq!(text_width(Some(3), &Overflow::Wrap, &wide), 3);
        assert_eq!(
            text_width(Some(10), &Overflow::Wrap, &wide),
            4,
            "an area wider than the text is a cap, not a size to take"
        );
        assert_eq!(
            text_width(Some(10), &Overflow::clip(), &wide),
            4,
            "nor is there anything for a clip to cut"
        );
        assert_eq!(
            text_width(Some(1), &Overflow::Wrap, &wide),
            2,
            "one unsplittable grapheme is wider than the area"
        );
        assert_eq!(
            text_width(Some(1), &Overflow::clip(), &wide),
            1,
            "a clip cuts to the area instead of widening"
        );
    }

    #[test]
    fn a_block_hands_its_content_width_down_and_keeps_its_own() {
        let view = View::block(
            BlockStyle::new().border(Border::NORMAL).padding((0, 1)),
            text("abcdefgh"),
        );

        let placed = widths(&view, Some(6));
        assert_eq!(placed.width, 6);
        let WidthNode::Block(block) = &placed.node else {
            panic!("a block");
        };
        assert_eq!(block.content_width, 2, "6 less two borders and two pads");
        assert_eq!(block.child.width, 2);
    }
}
