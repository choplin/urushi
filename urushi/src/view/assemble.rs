//! The assembly phase: the rectangle the settled sizes describe.
//!
//! Nothing here decides a size. Every number this module uses — a width, a row
//! count, the rows a box keeps — was settled by the [`width`](super::width) and
//! [`height`](super::height) phases; assembly turns text into graphemes, places
//! it inside the frame, draws the border and margin around it, and rasterizes
//! and composes Canvas commands. That is what makes the two backends agree: the
//! geometry existed before either of them saw it.

use std::sync::Arc;

use crate::text::{PrintableText, StyledTextGrapheme};
use crate::{
    Align, BlockStyle, BlockTitle, Canvas, GridStyle, Key, Projection, ProjectionBoundary, Sides,
    StyledText, TextStyle, VerticalAlign, View, Viewport,
};

use super::canvas::compose_canvas;
use super::geometry::Size;
use super::height::{Sized, SizedCell, SizedNode};
use super::resolve::{AnchoredRect, LayoutError, StyledGrapheme};

/// A rectangle under construction: every row is exactly `width` cells wide.
///
/// It carries the anchors resolved inside it, at positions relative to its own
/// top-left cell. A parent that nests this rectangle translates them by the
/// offset it introduces, which is why assembly is where anchors are
/// collected: it is the phase that knows every offset.
#[derive(Debug, Clone, Default)]
pub(super) struct Rect {
    pub width: usize,
    pub rows: Vec<Vec<StyledGrapheme>>,
    pub anchors: Vec<AnchoredRect>,
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
        clip_anchors(&mut self.anchors, 0, 0, self.width, self.rows.len());
    }

    pub fn crop_height(&mut self, max_height: usize) {
        if self.rows.len() > max_height {
            self.rows.truncate(max_height);
        }
        clip_anchors(&mut self.anchors, 0, 0, self.width, self.rows.len());
    }
}

/// Materialized subtree rectangles retained by [`Resolver`](super::resolve::Resolver).
///
/// Only entries reached by the current frame survive `finish_frame`. This
/// keeps unchanged branches reusable while dropping content invalidated by a
/// changed View or settled layout input.
#[derive(Debug, Default)]
pub(super) struct AssemblyCache {
    previous: Vec<Option<Arc<CachedRect>>>,
    current: Vec<Option<Arc<CachedRect>>>,
    previous_view: Option<View>,
    previous_subtree_ends: Vec<usize>,
    current_subtree_ends: Vec<usize>,
    equal_run: Vec<usize>,
}

#[derive(Debug, Clone)]
struct CachedRect {
    input: AssemblyInput,
    rect: Arc<Rect>,
}

impl AssemblyCache {
    pub const fn new() -> Self {
        Self {
            previous: Vec::new(),
            current: Vec::new(),
            previous_view: None,
            previous_subtree_ends: Vec::new(),
            current_subtree_ends: Vec::new(),
            equal_run: Vec::new(),
        }
    }

    pub fn begin_frame(&mut self, view: &View) {
        let current = ViewSnapshot::from_view(view);
        self.current.clear();
        self.current.resize_with(current.nodes.len(), || None);
        self.current_subtree_ends = current.subtree_ends;
        self.equal_run.clear();
        let Some(previous_view) = &self.previous_view else {
            return;
        };
        let previous = ViewSnapshot::from_view(previous_view);
        debug_assert_eq!(previous.subtree_ends, self.previous_subtree_ends);
        self.equal_run.resize(current.nodes.len(), 0);
        let shared = previous.nodes.len().min(current.nodes.len());
        let mut run = 0;
        for index in (0..shared).rev() {
            if previous.nodes[index] == current.nodes[index] {
                run += 1;
            } else {
                run = 0;
            }
            self.equal_run[index] = run;
        }
    }

    pub fn finish_frame(&mut self, view: &View) {
        self.previous = std::mem::take(&mut self.current);
        self.previous_view = Some(view.clone());
        self.previous_subtree_ends = std::mem::take(&mut self.current_subtree_ends);
    }

    pub fn abort_frame(&mut self) {
        self.current.clear();
        self.current_subtree_ends.clear();
        self.equal_run.clear();
    }

    pub fn clear(&mut self) {
        self.previous.clear();
        self.current.clear();
        self.previous_view = None;
        self.previous_subtree_ends.clear();
        self.current_subtree_ends.clear();
        self.equal_run.clear();
    }

    pub fn matches_previous_view(&self, view: &View) -> bool {
        self.previous_view.as_ref() == Some(view)
    }

    fn subtree_end(&self, index: usize) -> usize {
        self.current_subtree_ends[index]
    }

    fn get(&mut self, key: &AssemblyKey) -> Option<Arc<Rect>> {
        if let Some(entry) = self.current.get(key.node_index).and_then(Option::as_ref)
            && entry.input == key.input
        {
            return Some(Arc::clone(&entry.rect));
        }
        if !self.subtree_reusable(key.node_index) {
            return None;
        }
        let entry = self
            .previous
            .get(key.node_index)
            .and_then(Option::as_ref)
            .filter(|entry| entry.input == key.input)?
            .clone();
        let subtree_end = self.current_subtree_ends[key.node_index];
        for index in key.node_index..subtree_end {
            if self.current[index].is_none() {
                self.current[index] = self.previous.get(index).cloned().flatten();
            }
        }
        Some(Arc::clone(&entry.rect))
    }

    fn insert(&mut self, key: AssemblyKey, rect: Arc<Rect>) {
        let slot = &mut self.current[key.node_index];
        if let Some(entry) = slot {
            debug_assert_eq!(entry.input, key.input);
        } else {
            *slot = Some(Arc::new(CachedRect {
                input: key.input,
                rect,
            }));
        }
    }

    fn subtree_reusable(&self, index: usize) -> bool {
        if self.previous_view.is_none() {
            return false;
        }
        let (Some(&previous_end), Some(&current_end)) = (
            self.previous_subtree_ends.get(index),
            self.current_subtree_ends.get(index),
        ) else {
            return false;
        };
        let previous_len = previous_end - index;
        let current_len = current_end - index;
        previous_len == current_len
            && self.equal_run.get(index).copied().unwrap_or(0) >= current_len
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AssemblyKey {
    node_index: usize,
    input: AssemblyInput,
}

#[derive(Debug)]
struct ViewSnapshot<'a> {
    nodes: Vec<SnapshotNode<'a>>,
    subtree_ends: Vec<usize>,
}

impl<'a> ViewSnapshot<'a> {
    fn from_view(view: &'a View) -> Self {
        let mut snapshot = Self {
            nodes: Vec::new(),
            subtree_ends: Vec::new(),
        };
        snapshot.push_view(view);
        snapshot
    }

    fn push_view(&mut self, view: &'a View) {
        let index = self.nodes.len();
        self.nodes.push(SnapshotNode::from_view(view));
        self.subtree_ends.push(0);
        match view {
            View::Text(_) | View::Canvas(_) => {}
            View::Block(_, _, child)
            | View::Viewport(_, child)
            | View::AnchorBlock(_, _, _, child) => self.push_view(child),
            View::Row(_, children) | View::Column(_, children) => {
                for child in children {
                    self.push_view(child);
                }
            }
            View::Grid(_, rows) => {
                let columns = super::grid::columns(rows);
                for row in 0..rows.len() {
                    for column in 0..columns {
                        self.push_view(super::grid::cell(rows, row, column));
                    }
                }
            }
        }
        self.subtree_ends[index] = self.nodes.len();
    }
}

#[derive(Debug, Clone, PartialEq)]
enum SnapshotNode<'a> {
    Text(&'a StyledText),
    Block(&'a BlockStyle, Option<&'a BlockTitle>),
    Row(VerticalAlign, usize),
    Column(Align, usize),
    Grid(&'a GridStyle, usize, usize),
    Canvas(&'a Canvas),
    Viewport(Viewport),
    AnchorBlock(Key, &'a BlockStyle, Option<&'a BlockTitle>),
}

impl<'a> SnapshotNode<'a> {
    fn from_view(view: &'a View) -> Self {
        match view {
            View::Text(text) => Self::Text(text),
            View::Block(style, title, _) => Self::Block(style, title.as_ref()),
            View::Row(align, children) => Self::Row(*align, children.len()),
            View::Column(align, children) => Self::Column(*align, children.len()),
            View::Grid(style, rows) => Self::Grid(style, rows.len(), super::grid::columns(rows)),
            View::Canvas(canvas) => Self::Canvas(canvas),
            View::Viewport(viewport, _) => Self::Viewport(*viewport),
            View::AnchorBlock(key, style, title, _) => {
                Self::AnchorBlock(*key, style, title.as_ref())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AssemblyInput {
    Text {
        size: Size,
        rows: Vec<Vec<StyledGrapheme>>,
        align: Align,
        fill: TextStyle,
    },
    Block {
        size: Size,
        padding: Sides,
        margin: Sides,
        content_width: usize,
        content_height: usize,
        child: Size,
    },
    Row {
        size: Size,
        align: VerticalAlign,
        children: Vec<Size>,
    },
    Column {
        size: Size,
        align: Align,
        children: Vec<Size>,
    },
    Grid {
        size: Size,
        columns: Vec<usize>,
        heights: Vec<usize>,
        cells: Vec<GridCellInput>,
    },
    Canvas {
        size: Size,
        width_bounded: bool,
        height_bounded: bool,
    },
    Viewport {
        size: Size,
        width_bounded: bool,
        height_bounded: bool,
        child: Size,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GridCellInput {
    padding: Sides,
    align: Align,
    vertical_align: VerticalAlign,
    child: Size,
}

impl AssemblyKey {
    fn from_sized(sized: &Sized<'_>, node_index: usize) -> Self {
        let size = Size::new(sized.width, sized.height);
        let input = match &sized.node {
            SizedNode::Text { lines, align, fill } => AssemblyInput::Text {
                size,
                rows: lines.iter().map(|line| styled_graphemes(line)).collect(),
                align: *align,
                fill: (*fill).clone(),
            },
            SizedNode::Block {
                style: _,
                title: _,
                anchor: _,
                padding,
                margin,
                content_width,
                content_height,
                child,
            } => AssemblyInput::Block {
                size,
                padding: *padding,
                margin: *margin,
                content_width: *content_width,
                content_height: *content_height,
                child: Size::new(child.width, child.height),
            },
            SizedNode::Row(align, children) => AssemblyInput::Row {
                size,
                align: *align,
                children: children
                    .iter()
                    .map(|child| Size::new(child.width, child.height))
                    .collect(),
            },
            SizedNode::Column(align, children) => AssemblyInput::Column {
                size,
                align: *align,
                children: children
                    .iter()
                    .map(|child| Size::new(child.width, child.height))
                    .collect(),
            },
            SizedNode::Grid {
                columns,
                heights,
                rows,
            } => AssemblyInput::Grid {
                size,
                columns: columns.to_vec(),
                heights: heights.clone(),
                cells: rows
                    .iter()
                    .flatten()
                    .map(|cell| GridCellInput {
                        padding: cell.padding,
                        align: cell.align,
                        vertical_align: cell.vertical_align,
                        child: Size::new(cell.child.width, cell.child.height),
                    })
                    .collect(),
            },
            SizedNode::Canvas {
                canvas: _,
                width_bounded,
                height_bounded,
            } => AssemblyInput::Canvas {
                size,
                width_bounded: *width_bounded,
                height_bounded: *height_bounded,
            },
            SizedNode::Viewport {
                viewport: _,
                width_bounded,
                height_bounded,
                child,
            } => AssemblyInput::Viewport {
                size,
                width_bounded: *width_bounded,
                height_bounded: *height_bounded,
                child: Size::new(child.width, child.height),
            },
        };
        Self { node_index, input }
    }
}

/// Moves `anchors` by the offset a parent nests their rectangle at.
fn shift(anchors: Vec<AnchoredRect>, x: usize, y: usize) -> Vec<AnchoredRect> {
    anchors
        .into_iter()
        .map(|anchor| anchor.offset(x as i64, y as i64))
        .collect()
}

fn shift_signed(anchors: Vec<AnchoredRect>, x: i64, y: i64) -> Vec<AnchoredRect> {
    anchors
        .into_iter()
        .map(|anchor| anchor.offset(x, y))
        .collect()
}

fn clip_anchors(anchors: &mut [AnchoredRect], x: i64, y: i64, width: usize, height: usize) {
    for anchor in anchors {
        anchor.clip(x, y, width, height);
    }
}

/// The cells `align` leaves before a `gap`-cell shortfall.
const fn align_offset(gap: usize, align: Align) -> usize {
    match align {
        Align::Left => 0,
        // The odd extra cell goes right, as in Lip Gloss.
        Align::Center => gap / 2,
        Align::Right => gap,
    }
}

/// Builds the rectangle `sized` describes.
pub(super) fn assemble(sized: &Sized<'_>) -> Result<Rect, LayoutError> {
    Assembler {
        cache: None,
        next_node: 0,
    }
    .assemble(sized, false)
    .map(Assembled::into_owned)
}

pub(super) fn assemble_retained(
    sized: &Sized<'_>,
    cache: &mut AssemblyCache,
) -> Result<Rect, LayoutError> {
    Assembler {
        cache: Some(cache),
        next_node: 0,
    }
    .assemble(sized, false)
    .map(Assembled::into_owned)
}

enum Assembled {
    Owned(Rect),
    Shared(Arc<Rect>),
}

impl Assembled {
    fn as_rect(&self) -> &Rect {
        match self {
            Self::Owned(rect) => rect,
            Self::Shared(rect) => rect,
        }
    }

    fn into_owned(self) -> Rect {
        match self {
            Self::Owned(rect) => rect,
            Self::Shared(rect) => Arc::unwrap_or_clone(rect),
        }
    }
}

struct Assembler<'cache> {
    cache: Option<&'cache mut AssemblyCache>,
    next_node: usize,
}

impl Assembler<'_> {
    fn assemble(&mut self, sized: &Sized<'_>, retain: bool) -> Result<Assembled, LayoutError> {
        let node_index = self.cache.as_ref().map(|_| {
            let index = self.next_node;
            self.next_node += 1;
            index
        });
        let retain = retain || matches!(&sized.node, SizedNode::Canvas { .. });
        let key = self
            .cache
            .as_ref()
            .zip(node_index)
            .filter(|_| retain)
            .map(|(_, node_index)| AssemblyKey::from_sized(sized, node_index));
        if let (Some(cache), Some(key)) = (self.cache.as_deref_mut(), key.as_ref())
            && let Some(rect) = cache.get(key)
        {
            self.next_node = cache.subtree_end(key.node_index);
            return Ok(Assembled::Shared(rect));
        }

        let rect = match &sized.node {
            SizedNode::Text { lines, align, fill } => Ok(Rect {
                width: sized.width,
                rows: lines
                    .iter()
                    .map(|line| align_row(styled_graphemes(line), sized.width, *align, fill))
                    .collect(),
                anchors: Vec::new(),
            }),
            SizedNode::Block { .. } => block(sized, self),
            SizedNode::Row(align, children) => row(*align, children, sized.width, self),
            SizedNode::Column(align, children) => column(*align, children, sized.width, self),
            SizedNode::Grid { .. } => grid_rect(sized, self),
            SizedNode::Canvas { canvas, .. } => {
                compose_canvas(canvas, Size::new(sized.width, sized.height))
            }
            SizedNode::Viewport { .. } => viewport(sized, self),
        }?;
        debug_assert_eq!(
            rect.size(),
            Size::new(sized.width, sized.height),
            "assembly builds exactly the rectangle the sizing phases decided"
        );
        if let (Some(cache), Some(key)) = (self.cache.as_deref_mut(), key) {
            let rect = Arc::new(rect);
            cache.insert(key, Arc::clone(&rect));
            return Ok(Assembled::Shared(rect));
        }
        Ok(Assembled::Owned(rect))
    }
}

/// The style filling padding a parent introduces around a node.
fn fill_style(sized: &Sized<'_>) -> TextStyle {
    match &sized.node {
        SizedNode::Block { style, .. } => style.text_style().clone(),
        _ => TextStyle::new(),
    }
}

/// Builds a block: place the content, then close the frame around it.
fn block(sized: &Sized<'_>, assembler: &mut Assembler<'_>) -> Result<Rect, LayoutError> {
    let SizedNode::Block {
        style,
        title,
        anchor,
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
    let fill = style.text_style().clone();
    let (pl, pr, pt, pb) = (
        usize::from(padding.left),
        usize::from(padding.right),
        usize::from(padding.top),
        usize::from(padding.bottom),
    );

    let content = assembler.assemble(child, false)?.into_owned();
    debug_assert!(
        content.width <= content_width,
        "a box is never widened by what it contains: {} > {content_width}",
        content.width
    );

    // 1. The content, aligned inside the width the box left it. The alignment
    //    shifts every row by the same cells, so the anchors inside move with
    //    them.
    let mut inner_anchors = shift(
        content.anchors,
        align_offset(
            content_width.saturating_sub(content.width),
            style.get_align(),
        ),
        0,
    );
    let mut content_rows: Vec<Vec<StyledGrapheme>> = content
        .rows
        .into_iter()
        .map(|row| align_row(row, content_width, style.get_align(), &fill))
        .collect();

    // 2. The rows the box keeps, which the height phase decided; the vertical
    //    alignment places whatever slack is left, and has nothing to say when
    //    there is none.
    content_rows.truncate(content_height);
    let gap = content_height - content_rows.len();
    let (above, below) = match style.get_vertical_align() {
        VerticalAlign::Top => (0, gap),
        // The odd extra row goes below, the opposite of a Row's Center bias.
        VerticalAlign::Center => (gap / 2, gap - gap / 2),
        VerticalAlign::Bottom => (gap, 0),
    };
    inner_anchors = shift(inner_anchors, 0, above);
    clip_anchors(&mut inner_anchors, 0, 0, content_width, content_height);

    // 3. Padding, applied with the block's own fill. An anchor reports the
    //    rectangle the frame leaves, which is where a caller that fills it
    //    draws — inside the border and the padding, and before the content
    //    alignment moves anything within it. It comes first, because a box
    //    encloses what it reports.
    let total = pl + content_width + pr;
    let mut anchors = Vec::with_capacity(inner_anchors.len() + 1);
    if let Some(key) = anchor {
        anchors.push(AnchoredRect::new(
            *key,
            pl,
            pt,
            content_width,
            content_height,
        ));
    }
    anchors.extend(shift(inner_anchors, pl, pt));
    let mut rect = Rect {
        width: total,
        rows: Vec::with_capacity(content_height + pt + pb),
        anchors,
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
    if let Some(border) = style.get_border() {
        let border_style = style.border_style();
        let left = style.get_border_left();
        let right = style.get_border_right();
        let mut bordered = Rect {
            width: rect.width + usize::from(left) + usize::from(right),
            rows: Vec::with_capacity(rect.rows.len() + 2),
            anchors: shift(
                std::mem::take(&mut rect.anchors),
                usize::from(left),
                usize::from(style.get_border_top()),
            ),
        };
        if style.get_border_top() {
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
        if style.get_border_bottom() {
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
        if let Some(title) = title
            && style.get_border_top()
        {
            embed_title(
                &mut bordered.rows[0],
                title,
                style.get_border_left(),
                style.get_border_right(),
                &border_style,
            );
        }
        rect = bordered;
    }

    // 5. Margin: plain, unstyled space outside the border.
    if *margin != Sides::default() {
        rect = spaced(rect, *margin);
    }
    Ok(rect)
}

fn embed_title(
    row: &mut Vec<StyledGrapheme>,
    title: &BlockTitle,
    left_border: bool,
    right_border: bool,
    border_style: &TextStyle,
) {
    let band_width = row_width(row)
        .saturating_sub(usize::from(left_border))
        .saturating_sub(usize::from(right_border));
    if band_width == 0 {
        return;
    }

    let lines = title.text().lines();
    let Some(line) = lines.first() else {
        return;
    };
    let mut text = Vec::new();
    let mut text_width = 0;
    for grapheme in line {
        if text_width + grapheme.width() > band_width {
            break;
        }
        text_width += grapheme.width();
        text.push(StyledGrapheme::new(
            grapheme.grapheme,
            grapheme.style.clone(),
        ));
    }
    if text.is_empty() {
        return;
    }

    let preferred = usize::from(title.horizontal_padding());
    let padding = band_width
        .saturating_sub(text_width)
        .min(preferred.saturating_mul(2));
    let (left_padding, right_padding) = match title.alignment() {
        Align::Left => {
            let left = preferred.min(padding);
            (left, padding - left)
        }
        Align::Center => {
            let left = padding / 2;
            (left, padding - left)
        }
        Align::Right => {
            let right = preferred.min(padding);
            (padding - right, right)
        }
    };
    let slot_width = left_padding + text_width + right_padding;
    let start = usize::from(left_border)
        + align_offset(band_width.saturating_sub(slot_width), title.alignment());

    let mut replacement = blank(left_padding, border_style);
    replacement.extend(text);
    replacement.extend(blank(right_padding, border_style));
    row.splice(start..start + slot_width, replacement);
}

/// Surrounds a rectangle with plain, unstyled margin.
fn spaced(rect: Rect, margin: Sides) -> Rect {
    let plain = TextStyle::new();
    let (ml, mr) = (usize::from(margin.left), usize::from(margin.right));
    let outer = ml + rect.width + mr;
    let mut out = Rect {
        width: outer,
        rows: Vec::with_capacity(rect.rows.len() + usize::from(margin.top + margin.bottom)),
        anchors: shift(rect.anchors, ml, usize::from(margin.top)),
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
fn row(
    align: VerticalAlign,
    children: &[Sized<'_>],
    width: usize,
    assembler: &mut Assembler<'_>,
) -> Result<Rect, LayoutError> {
    let retain_children = children.len() > 1;
    let rects: Vec<(Rect, TextStyle)> = children
        .iter()
        .map(|child| {
            Ok((
                assembler.assemble(child, retain_children)?.into_owned(),
                fill_style(child),
            ))
        })
        .collect::<Result<_, LayoutError>>()?;
    let height = rects
        .iter()
        .map(|(rect, _)| rect.rows.len())
        .max()
        .unwrap_or(0);

    let mut rows: Vec<Vec<StyledGrapheme>> = vec![Vec::new(); height];
    let mut anchors = Vec::new();
    // Each child starts where the ones before it ended, which is the offset its
    // anchors move by.
    let mut left = 0;
    for (mut rect, fill) in rects {
        let gap = height - rect.rows.len();
        // Lip Gloss places the odd extra row of a Center alignment above the
        // shorter child; a BlockStyle's vertical_align places it below.
        let above = match align {
            VerticalAlign::Top => 0,
            VerticalAlign::Center => gap.div_ceil(2),
            VerticalAlign::Bottom => gap,
        };
        anchors.extend(shift(std::mem::take(&mut rect.anchors), left, above));
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
        left += rect.width;
    }

    Ok(Rect {
        width,
        rows,
        anchors,
    })
}

/// Stacks children, padding the narrower ones to `width` by `align`.
fn column(
    align: Align,
    children: &[Sized<'_>],
    width: usize,
    assembler: &mut Assembler<'_>,
) -> Result<Rect, LayoutError> {
    let mut rows = Vec::new();
    let mut anchors = Vec::new();
    let retain_children = children.len() > 1;
    for child in children {
        let fill = fill_style(child);
        let rect = assembler.assemble(child, retain_children)?.into_owned();
        // Every row of a child is padded to the column's width by the same
        // alignment, so its anchors shift by that same offset, and by the
        // rows already stacked above it.
        anchors.extend(shift(
            rect.anchors,
            align_offset(width.saturating_sub(rect.width), align),
            rows.len(),
        ));
        for row in rect.rows {
            rows.push(align_row(row, width, align, &fill));
        }
    }
    Ok(Rect {
        width,
        rows,
        anchors,
    })
}

/// Builds a grid by placing every cell in its settled column and row.
fn grid_rect(sized: &Sized<'_>, assembler: &mut Assembler<'_>) -> Result<Rect, LayoutError> {
    let SizedNode::Grid {
        columns,
        heights,
        rows,
    } = &sized.node
    else {
        unreachable!("a grid")
    };
    let retain_children = rows.iter().map(Vec::len).sum::<usize>() > 1;
    let (rows, anchors) = bands(rows, columns, heights, retain_children, assembler)?;
    Ok(Rect {
        width: sized.width,
        rows,
        anchors,
    })
}

/// Every content row of a grid, with cells in adjacent columns touching.
fn bands(
    rows: &[Vec<SizedCell<'_>>],
    columns: &[usize],
    heights: &[usize],
    retain_children: bool,
    assembler: &mut Assembler<'_>,
) -> Result<(Vec<Vec<StyledGrapheme>>, Vec<AnchoredRect>), LayoutError> {
    let mut out = Vec::new();
    let mut anchors = Vec::new();
    let mut top = 0;
    for (cells, height) in rows.iter().zip(heights) {
        let rects: Vec<Rect> = cells
            .iter()
            .zip(columns)
            .map(|(cell, width)| cell_rect(cell, *width, *height, retain_children, assembler))
            .collect::<Result<_, LayoutError>>()?;

        let mut x = 0;
        for rect in &rects {
            anchors.extend(shift(rect.anchors.clone(), x, top));
            x += rect.width;
        }

        for offset in 0..*height {
            let mut row = Vec::new();
            for rect in &rects {
                row.extend(rect.rows[offset].iter().cloned());
            }
            out.push(row);
        }
        top += *height;
    }
    Ok((out, anchors))
}

/// One cell, padded and placed inside the column and row it was assigned.
fn cell_rect(
    cell: &SizedCell<'_>,
    width: usize,
    height: usize,
    retain: bool,
    assembler: &mut Assembler<'_>,
) -> Result<Rect, LayoutError> {
    let (pl, pr, pt, pb) = (
        usize::from(cell.padding.left),
        usize::from(cell.padding.right),
        usize::from(cell.padding.top),
        usize::from(cell.padding.bottom),
    );
    let content_width = width.saturating_sub(pl + pr);
    let content_height = height.saturating_sub(pt + pb);
    let fill = fill_style(&cell.child);

    let content = assembler.assemble(&cell.child, retain)?.into_owned();
    let kept = content.rows.len().min(content_height);
    let gap = content_height - kept;
    let (above, below) = match cell.vertical_align {
        VerticalAlign::Top => (0, gap),
        VerticalAlign::Center => (gap / 2, gap - gap / 2),
        VerticalAlign::Bottom => (gap, 0),
    };

    // The cell's own alignment places it in the column and the row, so the
    // anchors inside it move by exactly what that placement introduced.
    let mut anchors = shift(
        content.anchors,
        pl + align_offset(content_width.saturating_sub(content.width), cell.align),
        pt + above,
    );
    clip_anchors(
        &mut anchors,
        pl as i64,
        pt as i64,
        content_width,
        content_height,
    );
    let mut content_rows: Vec<Vec<StyledGrapheme>> = content
        .rows
        .into_iter()
        .map(|row| align_row(row, content_width, cell.align, &fill))
        .collect();
    content_rows.truncate(content_height);

    let mut rect = Rect {
        width,
        rows: Vec::with_capacity(height),
        anchors,
    };
    let blank_row = blank(width, &fill);
    for _ in 0..pt + above {
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
    Ok(rect)
}

fn viewport(sized: &Sized<'_>, assembler: &mut Assembler<'_>) -> Result<Rect, LayoutError> {
    let SizedNode::Viewport {
        viewport, child, ..
    } = &sized.node
    else {
        unreachable!("a viewport")
    };
    let child = assembler.assemble(child, true)?;
    let child = child.as_rect();
    let horizontal = viewport.horizontal_projection();
    let vertical = viewport.vertical_projection();
    let x = horizontal.map_or(0, |projection| {
        effective_origin(projection, child.width, sized.width)
    });
    let y = vertical.map_or(0, |projection| {
        effective_origin(projection, child.rows.len(), sized.height)
    });

    let mut rows = Vec::with_capacity(sized.height);
    for target_y in 0..sized.height {
        let source_y = i128::try_from(target_y).unwrap_or(i128::MAX) + i128::from(y);
        let row = usize::try_from(source_y)
            .ok()
            .and_then(|source_y| child.rows.get(source_y));
        rows.push(match (row, horizontal) {
            (Some(row), Some(_)) => project_row(row, x, sized.width),
            (Some(row), None) => row.clone(),
            (None, _) => blank(sized.width, &TextStyle::new()),
        });
    }

    let mut anchors = shift_signed(
        child.anchors.clone(),
        x.saturating_neg(),
        y.saturating_neg(),
    );
    clip_anchors(&mut anchors, 0, 0, sized.width, sized.height);
    Ok(Rect {
        width: sized.width,
        rows,
        anchors,
    })
}

fn effective_origin(projection: Projection, content: usize, extent: usize) -> i64 {
    match projection.boundary() {
        ProjectionBoundary::Preserve => projection.origin(),
        ProjectionBoundary::Clamp => {
            let content = i64::try_from(content).unwrap_or(i64::MAX);
            let extent = i64::try_from(extent).unwrap_or(i64::MAX);
            projection
                .origin()
                .clamp(0, content.saturating_sub(extent).max(0))
        }
    }
}

fn project_row(row: &[StyledGrapheme], origin: i64, width: usize) -> Vec<StyledGrapheme> {
    let mut output = Vec::new();
    let mut output_width = 0usize;
    let mut source_x = 0i128;
    let origin = i128::from(origin);
    let edge = i128::try_from(width).unwrap_or(i128::MAX);

    for grapheme in row {
        let grapheme_width = i128::try_from(grapheme.width()).unwrap_or(i128::MAX);
        let left = source_x - origin;
        let right = left.saturating_add(grapheme_width);
        if left >= 0 && right <= edge {
            let left = usize::try_from(left).unwrap_or(width);
            if left > output_width {
                output.extend(blank(left - output_width, &TextStyle::new()));
                output_width = left;
            }
            output.push(grapheme.clone());
            output_width = output_width.saturating_add(grapheme.width());
        }
        source_x = source_x.saturating_add(grapheme_width);
    }
    output.extend(blank(width.saturating_sub(output_width), &TextStyle::new()));
    output
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
        .map(|grapheme| StyledGrapheme::new(grapheme, style.clone()))
        .collect()
}

fn styled_graphemes(graphemes: &[StyledTextGrapheme<'_>]) -> Vec<StyledGrapheme> {
    graphemes
        .iter()
        .map(|grapheme| StyledGrapheme::new(grapheme.grapheme, grapheme.style.clone()))
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
    // The same offset an anchor inside this row moves by; the test below pins
    // where the odd cell of a Center alignment goes.
    let left = align_offset(gap, align);
    let right = gap - left;
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
