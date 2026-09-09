//! Renderer-neutral terminal output.

use crate::{Align, BlockStyle, Canvas, GridStyle, Key, StyledText, TextStyle, VerticalAlign};

/// A fully composed, renderer-neutral terminal view.
///
/// A view tree combines text, boxes, linear and grid layout, and finite Canvas
/// drawing surfaces. Every node resolves to a rectangle, so a bordered block
/// or Canvas composes inside a row the same way a word does. An anchor is a box
/// that also reports where its content landed, for a caller that draws there
/// something this crate does not produce.
///
/// Components return a `View`; output adapters resolve it once
/// ([`resolve`](crate::resolve)) and serialize the resulting
/// [`ResolvedView`](crate::ResolvedView).
///
/// ```
/// use urushi::{Align, BlockStyle, Border, TextStyle, VerticalAlign, View, measure};
///
/// let badge = View::block(
///     BlockStyle::new().border(Border::ROUNDED),
///     View::text("ok", TextStyle::new()),
/// );
/// let row = View::row(VerticalAlign::Center, [View::text("status: ", TextStyle::new()), badge]);
///
/// assert_eq!(measure(&row).height(), 3);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    /// One plain-text flow whose grapheme-aligned segments carry complete
    /// styles. Never holds escape sequences or cursor movement: the layout
    /// pass measures its graphemes without scanning for them.
    Text(StyledText),
    /// One [`BlockStyle`] around exactly one child.
    Block(BlockStyle, Box<View>),
    /// Children placed side by side, aligned vertically.
    Row(VerticalAlign, Vec<View>),
    /// Children stacked, aligned horizontally.
    Column(Align, Vec<View>),
    /// A rectangle of cells sharing one width per column.
    ///
    /// Every row holds the same number of cells: a grid has no style to fill
    /// an invented one with, so whatever composes it supplies the empty cell.
    /// Debug builds panic on a ragged grid; release builds resolve a missing
    /// cell as an empty view.
    Grid(GridStyle, Vec<Vec<View>>),
    /// A finite free-positioned drawing surface.
    Canvas(Canvas),
    /// A block that also reports where it landed, named by a key.
    ///
    /// It is a [`Block`](Self::Block) in every respect layout cares about —
    /// the same one child, the same style, the same sizing — and the name says
    /// so. [`resolve`](crate::resolve) reports its content rectangle beside
    /// the resolved rows, for the caller that knows what belongs there.
    AnchorBlock(Key, BlockStyle, Box<View>),
}

impl Default for View {
    fn default() -> Self {
        Self::empty()
    }
}

impl View {
    /// Creates a text leaf.
    ///
    /// The text is plain. Escape sequences and cursor movement in it break that contract:
    /// debug builds panic, and release builds measure them as ordinary
    /// characters and may split them when wrapping or truncating. Adopt
    /// already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    pub fn text(text: impl Into<String>, style: TextStyle) -> Self {
        Self::Text(StyledText::new(text, style))
    }

    /// Creates a text leaf carrying multiple styled segments in one flow.
    pub const fn styled_text(text: StyledText) -> Self {
        Self::Text(text)
    }

    /// Wraps one child in a block.
    pub fn block(style: BlockStyle, child: Self) -> Self {
        Self::Block(style, Box::new(child))
    }

    /// Places children side by side.
    pub fn row(align: VerticalAlign, children: impl IntoIterator<Item = Self>) -> Self {
        Self::Row(align, children.into_iter().collect())
    }

    /// Stacks children.
    pub fn column(align: Align, children: impl IntoIterator<Item = Self>) -> Self {
        Self::Column(align, children.into_iter().collect())
    }

    /// Wraps one child in a block that reports where its content landed.
    ///
    /// An anchor carries no geometry of its own: it is a block, so its size is
    /// whatever `style` and its content decide, by the rules every other block
    /// follows. What the key adds is a report — an
    /// [`AnchoredRect`](crate::AnchoredRect) of the rectangle inside the frame
    /// — for a caller that draws there something this crate does not produce.
    ///
    /// The key is opaque here; this crate never looks at what belongs in the
    /// region. One key names one region: two anchors carrying the same key are
    /// a contract violation, which debug builds assert.
    ///
    /// ```
    /// use urushi::{Available, BlockStyle, Length, View, resolve};
    ///
    /// // A region for a foreign renderer: the box states the size, and the
    /// // empty content resolves to the blanks a backend without one draws.
    /// let chart = View::anchor_block(
    ///     "chart",
    ///     BlockStyle::new().width(Length::Cells(20)).height(Length::Cells(8)),
    ///     View::empty(),
    /// );
    /// let resolved = resolve(&chart, Available::NONE);
    ///
    /// let region = resolved.anchor("chart").expect("the anchor resolved");
    /// assert_eq!((region.width(), region.height()), (20, 8));
    /// ```
    pub fn anchor_block(key: impl Into<Key>, style: BlockStyle, child: Self) -> Self {
        Self::AnchorBlock(key.into(), style, Box::new(child))
    }

    /// Creates an anchor with no box around it: an empty region, named.
    ///
    /// This is the cursor case of [`anchor_block`](Self::anchor_block). The
    /// region covers no cells, so it changes no layout and draws nothing; what
    /// the caller reads is its origin.
    ///
    /// ```
    /// use urushi::{Available, TextStyle, VerticalAlign, View, resolve};
    ///
    /// let prompt = View::row(
    ///     VerticalAlign::Top,
    ///     [View::text("> ", TextStyle::new()), View::anchor("cursor")],
    /// );
    /// let resolved = resolve(&prompt, Available::NONE);
    ///
    /// let cursor = resolved.anchor("cursor").expect("the anchor resolved");
    /// assert_eq!((cursor.x(), cursor.y()), (2, 0));
    /// assert!(cursor.is_empty());
    /// ```
    pub fn anchor(key: impl Into<Key>) -> Self {
        Self::anchor_block(key, BlockStyle::new(), Self::empty())
    }

    /// Lines cells up in shared columns.
    ///
    /// Every row must hold the same number of cells; see [`View::Grid`].
    pub fn grid<R>(style: GridStyle, rows: impl IntoIterator<Item = R>) -> Self
    where
        R: IntoIterator<Item = Self>,
    {
        Self::Grid(
            style,
            rows.into_iter()
                .map(|row| row.into_iter().collect())
                .collect(),
        )
    }

    /// Creates a finite drawing surface from ordered, owned items.
    pub const fn canvas(canvas: Canvas) -> Self {
        Self::Canvas(canvas)
    }

    /// Creates a view that resolves to an empty rectangle.
    pub const fn empty() -> Self {
        Self::Column(Align::Left, Vec::new())
    }
}
