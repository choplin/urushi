//! Renderer-neutral terminal output.

use crate::{Align, BlockStyle, Canvas, GridStyle, Key, StyledText, TextStyle, VerticalAlign};

/// One styled line embedded in a block's top border.
///
/// A title is content owned by a block, not part of its [`BlockStyle`]. Its
/// text carries complete styles, and its alignment and padding apply only
/// within the top edge between enabled side borders.
///
/// ```
/// use urushi::{Align, BlockTitle, Color, StyledText, TextSpan, TextStyle};
///
/// let title = StyledText::try_from_spans([
///     TextSpan::new("F", TextStyle::new().foreground(Color::CYAN)),
///     TextSpan::from("iles"),
/// ])
/// .unwrap();
/// let title = BlockTitle::new(title).align(Align::Center).padding(0);
///
/// assert_eq!(title.text().as_str(), "Files");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTitle {
    text: StyledText,
    align: Align,
    padding: u16,
}

impl BlockTitle {
    /// Creates a left-aligned title with one blank cell on each side.
    ///
    /// # Panics
    ///
    /// Panics when `text` contains a line break. A border title occupies
    /// exactly one row and never wraps.
    pub fn new(text: impl Into<StyledText>) -> Self {
        let text = text.into();
        assert!(
            !text.as_str().contains('\n'),
            "a block title must be exactly one line"
        );
        Self {
            text,
            align: Align::Left,
            padding: 1,
        }
    }

    /// Sets the title's alignment between the block's enabled side borders.
    #[must_use]
    pub const fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the preferred blank cells on both sides of the title.
    ///
    /// A narrow border gives space to title text before this padding.
    #[must_use]
    pub const fn padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Returns the title's single styled line.
    pub const fn text(&self) -> &StyledText {
        &self.text
    }

    /// Returns the title's alignment between enabled side borders.
    pub const fn alignment(&self) -> Align {
        self.align
    }

    /// Returns the preferred blank cells on both sides of the title.
    pub const fn horizontal_padding(&self) -> u16 {
        self.padding
    }
}

impl From<&str> for BlockTitle {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for BlockTitle {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

impl From<StyledText> for BlockTitle {
    fn from(text: StyledText) -> Self {
        Self::new(text)
    }
}

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
    /// One text flow whose grapheme-aligned segments carry complete styles.
    /// Source tabs are replaced under its layout policy before measurement;
    /// escape sequences and cursor movement remain invalid.
    Text(StyledText),
    /// One [`BlockStyle`] and optional [`BlockTitle`] around exactly one child.
    Block(BlockStyle, Option<BlockTitle>, Box<View>),
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
    AnchorBlock(Key, BlockStyle, Option<BlockTitle>, Box<View>),
}

impl Default for View {
    fn default() -> Self {
        Self::empty()
    }
}

impl View {
    /// Creates a text leaf.
    ///
    /// The text may contain newline and horizontal tab. Tabs use the default
    /// four-space layout policy; construct a [`StyledText`] to select another
    /// policy. Escape sequences and cursor movement break the contract and
    /// panic during construction in every build profile. Raw ANSI is not a
    /// valid `Text` payload.
    pub fn text(text: impl Into<String>, style: TextStyle) -> Self {
        Self::Text(StyledText::new(text, style))
    }

    /// Creates a text leaf carrying multiple styled segments in one flow.
    pub const fn styled_text(text: StyledText) -> Self {
        Self::Text(text)
    }

    /// Wraps one child in a block.
    pub fn block(style: BlockStyle, child: Self) -> Self {
        Self::Block(style, None, Box::new(child))
    }

    /// Wraps one child in a block with a styled title in its top border.
    ///
    /// The title participates in automatic width demand but never increases
    /// the box past an explicit or available width. It is clipped without
    /// wrapping when the top edge is narrower than its text and padding.
    ///
    /// ```
    /// use urushi::{BlockStyle, Border, View, measure};
    ///
    /// let panel = View::titled_block(
    ///     BlockStyle::new().border(Border::NORMAL),
    ///     "Files",
    ///     View::empty(),
    /// );
    ///
    /// assert_eq!(measure(&panel).width(), 9);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when `style` has no top border edge.
    pub fn titled_block(style: BlockStyle, title: impl Into<BlockTitle>, child: Self) -> Self {
        assert_title_edge(&style);
        Self::Block(style, Some(title.into()), Box::new(child))
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
    /// let resolved = resolve(&chart, Available::NONE).unwrap();
    ///
    /// let region = resolved.anchor("chart").expect("the anchor resolved");
    /// assert_eq!((region.width(), region.height()), (20, 8));
    /// ```
    pub fn anchor_block(key: impl Into<Key>, style: BlockStyle, child: Self) -> Self {
        Self::AnchorBlock(key.into(), style, None, Box::new(child))
    }

    /// Wraps one child in a titled block and reports its content rectangle.
    ///
    /// Geometry and title behavior are identical to [`titled_block`](Self::titled_block);
    /// the key adds only the same report as [`anchor_block`](Self::anchor_block).
    ///
    /// # Panics
    ///
    /// Panics when `style` has no top border edge.
    pub fn titled_anchor_block(
        key: impl Into<Key>,
        style: BlockStyle,
        title: impl Into<BlockTitle>,
        child: Self,
    ) -> Self {
        assert_title_edge(&style);
        Self::AnchorBlock(key.into(), style, Some(title.into()), Box::new(child))
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
    /// let resolved = resolve(&prompt, Available::NONE).unwrap();
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

fn assert_title_edge(style: &BlockStyle) {
    assert!(
        style.border_kind().is_some() && style.is_border_top_enabled(),
        "a titled block requires an enabled top border"
    );
}
