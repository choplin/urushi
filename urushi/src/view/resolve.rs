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
//! height is what wrapping produced. [`assemble`](super::assemble) builds the
//! rectangle those numbers describe, and decides nothing. This module is the
//! entry point that runs them and the degenerate-case safety net that bounds
//! the result.

use crate::text::Grapheme;
use crate::{Key, TextStyle, View};

use super::assemble::assemble;
use super::geometry::{Available, Size};
use super::height::{fit, heights};
use super::rendered::RenderedBlock;
use super::width::widths;

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

    pub(crate) fn map_style(mut self, map: impl FnOnce(&TextStyle) -> TextStyle) -> Self {
        self.style = map(&self.style);
        self
    }
}

/// One anchor's rectangle: its key, and where layout put it.
///
/// The rectangle is stated relative to the resolved view's own top-left cell,
/// so `x` and `y` index its rows; where that view sits on the terminal is the
/// caller's to add.
///
/// It reports what layout produced, not what survived into the rows: a region
/// whose cells the rectangle does not contain keeps the extent it was given,
/// and says so through
/// [`is_within_resolved_view`](Self::is_within_resolved_view). Rounding it
/// inwards instead would report a cursor scrolled ten rows out of view as
/// sitting on the last one, which is the one thing a caller placing a cursor
/// must not be told.
///
/// This crate never looks at what belongs in the region. A backend that knows
/// draws into the rectangle; one that does not draws the blanks the anchor
/// resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchoredRect {
    key: Key,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    within_resolved_view: bool,
}

impl AnchoredRect {
    pub(super) const fn new(key: Key, x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            key,
            x,
            y,
            width,
            height,
            // Nothing to compare against until the rectangle this is reported
            // against exists; `resolve` settles it once, at the end.
            within_resolved_view: false,
        }
    }

    /// The key the anchor carried.
    pub const fn key(&self) -> Key {
        self.key
    }

    /// Cells from the resolved view's left edge.
    pub const fn x(&self) -> usize {
        self.x
    }

    /// Rows from the resolved view's top edge.
    pub const fn y(&self) -> usize {
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
    /// False when layout put the region where the resolved rectangle does not
    /// reach — a cursor below content taller than the area it was given, say.
    /// The region still reports where it went; what that means is the
    /// caller's: a full-screen runtime hides a cursor it cannot show or
    /// scrolls to it, and a caller drawing into a region intersects it with
    /// the resolved size first.
    ///
    /// An empty region sitting one cell past the content is still within the
    /// resolved view: that is where a cursor belongs when it follows the last
    /// grapheme.
    pub const fn is_within_resolved_view(&self) -> bool {
        self.within_resolved_view
    }

    /// Settles
    /// [`is_within_resolved_view`](Self::is_within_resolved_view) against the
    /// rectangle this is reported with.
    pub(super) const fn locate(mut self, resolved: Size) -> Self {
        self.within_resolved_view =
            self.x + self.width <= resolved.width() && self.y + self.height <= resolved.height();
        self
    }

    /// Moves the rectangle by the offset a parent nests it at.
    ///
    /// This is what assembly applies as it nests a rectangle inside a larger
    /// one: every offset a parent introduces — padding, a border, a margin, a
    /// sibling to the left, an alignment gap — moves the anchors within it.
    pub(super) const fn offset(mut self, x: usize, y: usize) -> Self {
        self.x += x;
        self.y += y;
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
    anchors: Vec<AnchoredRect>,
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
    /// let resolved = resolve(&view, Available::NONE);
    ///
    /// assert_eq!(resolved.anchor("cursor").unwrap().x(), 2);
    /// assert!(resolved.anchor("elsewhere").is_none());
    /// ```
    pub fn anchor(&self, key: impl Into<Key>) -> Option<&AnchoredRect> {
        let key = key.into();
        self.anchors.iter().find(|anchor| anchor.key == key)
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
/// This is [`resolve`] under [`Available::NONE`], not a second set of rules —
/// the same two sizing phases, stopping before the rectangle they describe is
/// built. No rectangle is allocated.
pub fn measure(view: &View) -> Size {
    let fitted = fit(widths(view, None));
    let sized = heights(&fitted, None);
    Size::new(sized.width, sized.height)
}

/// Resolves `view` into one rectangle sized under `available`.
///
/// Every node resolves its own size under the area, so a bound reshapes a box
/// rather than cutting it. The crop below is the degenerate-case safety net:
/// it fires only when a rectangle could not be made to fit — an area that
/// cannot hold a frame at all — and it cuts grapheme-atomically.
pub fn resolve(view: &View, available: Available) -> ResolvedView {
    let fitted = fit(widths(view, available.width()));
    let sized = heights(&fitted, available.height());
    let mut rect = assemble(&sized);
    if let Some(width) = available.width() {
        rect.crop_width(width, &TextStyle::new());
    }
    if let Some(height) = available.height() {
        rect.crop_height(height);
    }
    debug_assert_unique(&rect.anchors);
    let size = rect.size();
    let anchors = rect
        .anchors
        .into_iter()
        .map(|anchor| anchor.locate(size))
        .collect();
    ResolvedView::new(size, rect.rows, anchors)
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
