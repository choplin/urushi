//! The one pass: a [`View`] and an [`Available`] area in, one rectangle out.
//!
//! Resolution is a pure function of those two inputs — no terminal state, no
//! capability profile, no escape sequences. Its output is a [`ResolvedView`]:
//! a size and rows of graphemes carrying logical styles and the widths this
//! pass decided, which is the single thing both backends draw. That is why the
//! ANSI string and the Ratatui buffer cannot disagree about geometry.
//!
//! The pass is three phases, in the order `docs/view-model.md` fixes and in
//! the only order the dependencies allow. [`width`](super::width) settles every
//! width, because wrapping needs a width to wrap to.
//! [`height`](super::height) then fits the text and counts the rows, because a
//! height is what wrapping produced. [`assemble`](super::assemble) builds the
//! rectangle those numbers describe, and decides nothing. This module is the
//! entry point that runs them and the degenerate-case safety net that bounds
//! the result.

use crate::{TextStyle, View};

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
    /// Creates one styled grapheme.
    ///
    /// `symbol` is one plain-text grapheme cluster and `width` the cells it
    /// occupies. A `ResolvedView` holds no escape sequences, so a symbol
    /// carrying one would reach the backend as ordinary characters.
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

    pub(super) fn space(style: TextStyle) -> Self {
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
    ResolvedView::new(rect.size(), rect.rows)
}
