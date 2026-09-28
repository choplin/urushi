//! The one pass: a [`View`] and an [`Available`] area in, one rectangle out.
//!
//! Resolution is a pure function of those two inputs — no terminal state, no
//! capability profile, no escape sequences. Its output is a [`ResolvedView`]:
//! a size and rows of graphemes carrying logical styles and the widths this
//! pass decided, which is the single thing both backends draw. That is why the
//! ANSI string and the Ratatui buffer cannot disagree about geometry.
//!
//! The pass is three phases, in the order `docs/design/layout-resolution.md`
//! fixes and in the only order the dependencies allow. [`width`](super::width)
//! settles every width, because wrapping needs a width to wrap to.
//! [`height`](super::height) then fits the text and counts the rows, because a
//! height is what wrapping produced. [`assemble`](super::assemble) then builds
//! the rectangle those numbers describe. At a Canvas leaf, each command
//! rasterizes against the settled dimensions without access to the destination
//! surface and is immediately passed to the common compositor. This module is
//! the entry point that runs the phases and the degenerate-case safety net that
//! bounds the result.

use crate::text::Grapheme;
use crate::{Key, TextStyle, View};

use super::assemble::{AssemblyCache, assemble, assemble_retained};
use super::geometry::{Available, Constraint, Size};
use super::height::{fit, heights};
use super::width::widths;

/// An axis for which a finite Canvas extent was required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Width,
    Height,
}

/// The input whose finite extent is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutErrorKind {
    CanvasExtent,
    ViewAllocation,
    ViewportExtent,
}

/// A layout request that cannot produce finite geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutError {
    kind: LayoutErrorKind,
    axis: Axis,
}

impl LayoutError {
    pub const fn axis(&self) -> Axis {
        self.axis
    }
    pub const fn kind(&self) -> LayoutErrorKind {
        self.kind
    }
    pub(crate) const fn missing_extent(axis: Axis) -> Self {
        Self {
            kind: LayoutErrorKind::CanvasExtent,
            axis,
        }
    }
    pub(crate) const fn missing_allocation(axis: Axis) -> Self {
        Self {
            kind: LayoutErrorKind::ViewAllocation,
            axis,
        }
    }
    pub(crate) const fn missing_viewport_extent(axis: Axis) -> Self {
        Self {
            kind: LayoutErrorKind::ViewportExtent,
            axis,
        }
    }
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            LayoutErrorKind::CanvasExtent => write!(
                f,
                "Canvas requires an explicit {:?} on an unbounded axis",
                self.axis
            ),
            LayoutErrorKind::ViewAllocation => write!(
                f,
                "a Canvas View command with Fill requires a finite {:?} allocation",
                self.axis
            ),
            LayoutErrorKind::ViewportExtent => write!(
                f,
                "Viewport projection requires a finite {:?} allocation",
                self.axis
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

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
    /// Creates one styled grapheme, measuring the cells it occupies.
    ///
    /// The width is derived rather than supplied, so a token cannot claim a
    /// width its symbol does not have. Taking a [`Grapheme`] rather than a
    /// string closes the other half: a token holds one cluster, which is what
    /// a backend assumes when it writes the symbol into the cell its width
    /// starts at.
    ///
    /// Where a terminal-dependent measure would enter is
    /// [`text::width`](crate::text), the crate's one definition, and not this
    /// constructor. The pass still decides the width once and the renderers
    /// still read it here rather than measuring again.
    pub(crate) fn new(symbol: &Grapheme, style: TextStyle) -> Self {
        Self {
            symbol: symbol.as_str().to_owned(),
            width: symbol.width(),
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

    pub(super) fn space(style: TextStyle) -> Self {
        Self::new(Grapheme::space(), style)
    }
}

/// The part of an anchor rectangle that survived every enclosing clip.
///
/// Coordinates remain relative to the final [`ResolvedView`]. A clipped
/// rectangle is never moved onto an edge; this value is the true intersection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VisibleRect {
    x: i64,
    y: i64,
    width: usize,
    height: usize,
}

impl VisibleRect {
    pub const fn x(&self) -> i64 {
        self.x
    }

    pub const fn y(&self) -> i64 {
        self.y
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }
}

/// One anchor's complete logical rectangle and its visible intersection.
///
/// The logical rectangle is stated relative to the resolved view's own
/// top-left cell. It is translated but never clipped or rounded onto an edge.
/// [`visible`](Self::visible) separately reports what survived Block, Canvas,
/// Viewport, and final safety clips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchoredRect {
    key: Key,
    x: i64,
    y: i64,
    width: usize,
    height: usize,
    visible: Option<VisibleRect>,
}

impl AnchoredRect {
    pub(super) const fn new(key: Key, x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            key,
            x: x as i64,
            y: y as i64,
            width,
            height,
            visible: Some(VisibleRect {
                x: x as i64,
                y: y as i64,
                width,
                height,
            }),
        }
    }

    /// The key the anchor carried.
    pub const fn key(&self) -> Key {
        self.key
    }

    /// Cells from the resolved view's left edge.
    pub const fn x(&self) -> i64 {
        self.x
    }

    /// Rows from the resolved view's top edge.
    pub const fn y(&self) -> i64 {
        self.y
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    pub const fn height(&self) -> usize {
        self.height
    }

    /// Returns whether the region covers no cells, as a cursor anchor does.
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Returns whether the [`ResolvedView`] this came from contains the whole
    /// region.
    ///
    /// A partially visible or outside region returns false. A zero-sized
    /// cursor point returns true only when it lies inside every half-open clip;
    /// a point on the right or bottom edge is outside.
    pub const fn is_within_resolved_view(&self) -> bool {
        match self.visible {
            Some(_) if self.width == 0 && self.height == 0 => true,
            Some(visible) => {
                visible.x == self.x
                    && visible.y == self.y
                    && visible.width == self.width
                    && visible.height == self.height
            }
            None => false,
        }
    }

    /// The part of the logical rectangle that survived every enclosing clip.
    pub const fn visible(&self) -> Option<VisibleRect> {
        self.visible
    }

    /// Settles
    /// [`is_within_resolved_view`](Self::is_within_resolved_view) against the
    /// rectangle this is reported with.
    pub(super) fn locate(mut self, resolved: Size) -> Self {
        self.clip(0, 0, resolved.width(), resolved.height());
        self
    }

    /// Moves the rectangle by the offset a parent nests it at.
    ///
    /// This is what assembly applies as it nests a rectangle inside a larger
    /// one: every offset a parent introduces — padding, a border, a margin, a
    /// sibling to the left, an alignment gap — moves the anchors within it.
    pub(super) const fn offset(mut self, x: i64, y: i64) -> Self {
        self.x = self.x.saturating_add(x);
        self.y = self.y.saturating_add(y);
        if let Some(visible) = &mut self.visible {
            visible.x = visible.x.saturating_add(x);
            visible.y = visible.y.saturating_add(y);
        }
        self
    }

    pub(super) fn clip(&mut self, x: i64, y: i64, width: usize, height: usize) {
        let Some(visible) = self.visible else {
            return;
        };
        let right = x.saturating_add(i64::try_from(width).unwrap_or(i64::MAX));
        let bottom = y.saturating_add(i64::try_from(height).unwrap_or(i64::MAX));

        if visible.width == 0 && visible.height == 0 {
            if visible.x < x || visible.x >= right || visible.y < y || visible.y >= bottom {
                self.visible = None;
            }
            return;
        }

        let visible_right = visible
            .x
            .saturating_add(i64::try_from(visible.width).unwrap_or(i64::MAX));
        let visible_bottom = visible
            .y
            .saturating_add(i64::try_from(visible.height).unwrap_or(i64::MAX));
        let left = visible.x.max(x);
        let top = visible.y.max(y);
        let clipped_right = visible_right.min(right);
        let clipped_bottom = visible_bottom.min(bottom);
        if clipped_right <= left || clipped_bottom <= top {
            self.visible = None;
            return;
        }
        self.visible = Some(VisibleRect {
            x: left,
            y: top,
            width: usize::try_from(clipped_right - left).unwrap_or(usize::MAX),
            height: usize::try_from(clipped_bottom - top).unwrap_or(usize::MAX),
        });
    }
}

/// A view resolved to one rectangle of styled graphemes.
///
/// Every row's widths sum to `size.width()`, and the row count equals
/// `size.height()`. Styles are logical: a
/// [`RenderSettings`](crate::RenderSettings) is applied when a renderer
/// serializes the rectangle, so capability resolution stays at the output
/// boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedView {
    size: Size,
    rows: Vec<Vec<StyledGrapheme>>,
    anchors: Vec<AnchoredRect>,
}

/// An opt-in evaluator that reuses unchanged View evaluation across frames.
///
/// `Resolver` has the same observable result as [`resolve`]. It retains only
/// core reconciliation inputs and materialized output; the current View and
/// available area still decide layout. Application state, clocks, redraw
/// scheduling, terminal capabilities, and graphics protocol state stay with
/// the host that composes those independent capabilities.
///
/// The free [`resolve`] function remains the ordinary stateless path and does
/// not construct this cache metadata.
pub struct Resolver {
    previous_available: Option<Available>,
    previous_result: Option<ResolvedView>,
    assembly: AssemblyCache,
}

impl Resolver {
    /// Creates an empty retained evaluator.
    pub const fn new() -> Self {
        Self {
            previous_available: None,
            previous_result: None,
            assembly: AssemblyCache::new(),
        }
    }

    /// Resolves one immutable View snapshot, reusing unchanged subtree output.
    ///
    /// A changed Viewport origin reprojects its retained child. Changes to
    /// content or to a settled layout input invalidate the affected artifact,
    /// while independent unchanged subtrees remain reusable.
    pub fn resolve(
        &mut self,
        view: &View,
        available: Available,
    ) -> Result<ResolvedView, LayoutError> {
        if self.assembly.matches_previous_view(view)
            && self.previous_available == Some(available)
            && let Some(result) = &self.previous_result
        {
            return Ok(result.clone());
        }

        let fitted = fit(widths(view, available.width()));
        let sized = heights(&fitted, Constraint::available(available.height()));
        validate_canvas_extents(&sized)?;

        self.assembly.begin_frame(view);
        let mut rect = match assemble_retained(&sized, &mut self.assembly) {
            Ok(rect) => rect,
            Err(error) => {
                self.assembly.abort_frame();
                return Err(error);
            }
        };
        crop_to_available(&mut rect, available);
        let result = resolved(rect);
        self.assembly.finish_frame(view);
        self.previous_available = Some(available);
        self.previous_result = Some(result.clone());
        Ok(result)
    }

    /// Drops every retained evaluation artifact.
    pub fn clear(&mut self) {
        self.previous_available = None;
        self.previous_result = None;
        self.assembly.clear();
    }
}

impl Default for Resolver {
    fn default() -> Self {
        Self::new()
    }
}

impl ResolvedView {
    pub(crate) fn new(
        size: Size,
        rows: Vec<Vec<StyledGrapheme>>,
        anchors: Vec<AnchoredRect>,
    ) -> Self {
        Self {
            size,
            rows,
            anchors,
        }
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub fn rows(&self) -> &[Vec<StyledGrapheme>] {
        &self.rows
    }

    /// Where each anchor in the tree landed, in tree order — a box before
    /// what it encloses.
    ///
    /// A tree carrying no anchor reports nothing, which is every view built
    /// before anchors existed.
    pub fn anchors(&self) -> &[AnchoredRect] {
        &self.anchors
    }

    /// The region reported under `key`.
    ///
    /// This is the ordinary read: a caller filling one region asks for it by
    /// name rather than scanning [`anchors`](Self::anchors).
    ///
    /// ```
    /// use urushi::{Available, BlockStyle, TextStyle, VerticalAlign, View, resolve};
    ///
    /// let view = View::row(
    ///     VerticalAlign::Top,
    ///     [View::text("> ", TextStyle::new()), View::anchor("cursor")],
    /// );
    /// let resolved = resolve(&view, Available::NONE).unwrap();
    ///
    /// assert_eq!(resolved.anchor("cursor").unwrap().x(), 2);
    /// assert!(resolved.anchor("elsewhere").is_none());
    /// ```
    pub fn anchor(&self, key: impl Into<Key>) -> Option<&AnchoredRect> {
        let key = key.into();
        self.anchors.iter().find(|anchor| anchor.key == key)
    }
}

/// Returns the intrinsic rectangle `view` occupies: its size when no area
/// bounds it.
///
/// This is [`resolve`] under [`Available::NONE`], not a second set of rules —
/// the same two sizing phases, stopping before the rectangle they describe is
/// built. No rectangle is allocated.
pub fn measure(view: &View) -> Size {
    try_measure(view).expect("intrinsic measurement requires every finite extent to be stated")
}

/// Tries to measure a view, reporting any required finite extent that is not
/// established within the tree.
pub fn try_measure(view: &View) -> Result<Size, LayoutError> {
    let fitted = fit(widths(view, None));
    let sized = heights(&fitted, Constraint::unbounded());
    validate_canvas_extents(&sized)?;
    Ok(Size::new(sized.width, sized.height))
}

/// Resolves `view` into one rectangle sized under `available`.
///
/// Every node resolves its own size under the area, so a bound reshapes a box
/// rather than cutting it. The crop below is the degenerate-case safety net:
/// it fires only when a rectangle could not be made to fit — an area that
/// cannot hold a frame at all — and it cuts grapheme-atomically.
pub fn resolve(view: &View, available: Available) -> Result<ResolvedView, LayoutError> {
    let fitted = fit(widths(view, available.width()));
    let sized = heights(&fitted, Constraint::available(available.height()));
    validate_canvas_extents(&sized)?;
    let mut rect = assemble(&sized)?;
    crop_to_available(&mut rect, available);
    Ok(resolved(rect))
}

fn crop_to_available(rect: &mut super::assemble::Rect, available: Available) {
    if let Some(width) = available.width() {
        rect.crop_width(width, &TextStyle::new());
    }
    if let Some(height) = available.height() {
        rect.crop_height(height);
    }
}

fn resolved(rect: super::assemble::Rect) -> ResolvedView {
    debug_assert_unique(&rect.anchors);
    let size = rect.size();
    let anchors = rect
        .anchors
        .into_iter()
        .map(|anchor| anchor.locate(size))
        .collect();
    ResolvedView::new(size, rect.rows, anchors)
}

fn validate_canvas_extents(sized: &super::height::Sized<'_>) -> Result<(), LayoutError> {
    use super::height::SizedNode;
    match &sized.node {
        SizedNode::Canvas {
            canvas,
            width_bounded,
            height_bounded,
            ..
        } => {
            if canvas.uses_viewport_sizing() && !width_bounded && canvas.explicit_width().is_none()
            {
                return Err(LayoutError::missing_extent(Axis::Width));
            }
            if canvas.uses_viewport_sizing()
                && !height_bounded
                && canvas.explicit_height().is_none()
            {
                return Err(LayoutError::missing_extent(Axis::Height));
            }
            Ok(())
        }
        SizedNode::Block { child, .. } => validate_canvas_extents(child),
        SizedNode::Viewport {
            viewport,
            width_bounded,
            height_bounded,
            child,
        } => {
            if viewport.horizontal_projection().is_some() && !width_bounded {
                return Err(LayoutError::missing_viewport_extent(Axis::Width));
            }
            if viewport.vertical_projection().is_some() && !height_bounded {
                return Err(LayoutError::missing_viewport_extent(Axis::Height));
            }
            validate_canvas_extents(child)
        }
        SizedNode::Row(_, children) | SizedNode::Column(_, children) => {
            children.iter().try_for_each(validate_canvas_extents)
        }
        SizedNode::Grid { rows, .. } => rows
            .iter()
            .flatten()
            .try_for_each(|cell| validate_canvas_extents(&cell.child)),
        SizedNode::Text { .. } => Ok(()),
    }
}

/// Asserts that no key names two regions.
///
/// One key, one region: a caller reads a region by the name it gave, and two
/// answers to one name is a mistake in the tree rather than something layout
/// can resolve. As with escape sequences in a `Text` node, this is a contract
/// violation detected as a development aid — debug builds assert, release
/// builds report both regions in tree order and leave the caller to whatever
/// it makes of them.
fn debug_assert_unique(anchors: &[AnchoredRect]) {
    debug_assert!(
        {
            let mut seen = std::collections::HashSet::with_capacity(anchors.len());
            anchors.iter().all(|anchor| seen.insert(anchor.key()))
        },
        "two anchors carry one key; a key names one region"
    );
}
