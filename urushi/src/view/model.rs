//! Renderer-neutral terminal output.

use crate::{Align, BlockStyle, TextStyle, VerticalAlign};

/// A fully composed, renderer-neutral terminal view.
///
/// A view is a tree of the four things terminal output does: carry text, put a
/// box around something, place things beside each other, and stack them. Every
/// node resolves to a rectangle, so a bordered block composes inside a row the
/// same way a word does.
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
    /// Plain text and the style applied to it. Never holds escape sequences or
    /// cursor movement: the layout pass measures its graphemes without
    /// scanning for them.
    Text(String, TextStyle),
    /// One [`BlockStyle`] around exactly one child.
    Block(BlockStyle, Box<View>),
    /// Children placed side by side, aligned vertically.
    Row(VerticalAlign, Vec<View>),
    /// Children stacked, aligned horizontally.
    Column(Align, Vec<View>),
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
        Self::Text(text.into(), style)
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

    /// Creates a view that resolves to an empty rectangle.
    pub const fn empty() -> Self {
        Self::Column(Align::Left, Vec::new())
    }
}
