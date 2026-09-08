//! Free-positioned, renderer-neutral drawing inside a finite [`Canvas`].
//!
//! A Canvas owns immutable [`CanvasItem`] values. Resolution first fixes the
//! surface size, then calls each item once with a frame-scoped [`CanvasContext`].
//! Items record `View`, text, [`Path`], or sparse-cell commands in paint order.
//!
//! ```
//! use urushi::{
//!     Available, Canvas, CanvasContext, CanvasItem, Composition, Position,
//!     Size, TextStyle, View, resolve, try_resolve,
//! };
//!
//! #[derive(Debug, Clone, PartialEq)]
//! struct Label(&'static str);
//!
//! impl CanvasItem for Label {
//!     fn draw(&self, canvas: &mut CanvasContext) {
//!         // Responsive items see the final size, not a provisional measure.
//!         let x = canvas.size().width().saturating_sub(self.0.len()) as i64;
//!         canvas.text_with(
//!             Position::new(x, 0),
//!             self.0,
//!             TextStyle::new().bold(),
//!             Composition::Overlay,
//!         );
//!     }
//! }
//!
//! // Explicit extents make an otherwise-unbounded Canvas finite.
//! let view = View::canvas(Canvas::new().extent(Size::new(8, 2)).item(Label("ok")));
//! assert_eq!(resolve(&view, Available::NONE).size(), Size::new(8, 2));
//!
//! // A parent allocation takes precedence over an explicit extent. Omit an
//! // extent only when that axis will always receive a finite allocation.
//! let allocated = View::canvas(Canvas::new().item(Label("ok")));
//! assert_eq!(resolve(&allocated, Available::size(12, 3)).size(), Size::new(12, 3));
//! assert!(try_resolve(&allocated, Available::NONE).is_err());
//! ```

mod assemble;
mod cell;
mod context;
mod path;

use std::any::Any;
use std::fmt;

use super::geometry::Size;

pub use cell::{CanvasCell, CellContribution, Composition, PositionedCell};
pub use context::CanvasContext;
pub use path::Path;

pub(in crate::view) use assemble::canvas_rect;
use context::CanvasCommand;

/// A signed cell position relative to a Canvas's top-left corner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Position {
    pub x: i64,
    pub y: i64,
}

impl Position {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

/// Immutable frame data that records drawing commands after Canvas size is known.
pub trait CanvasItem: fmt::Debug + Send + Sync + 'static {
    /// Records this item's commands after the Canvas size is final.
    fn draw(&self, context: &mut CanvasContext);
}

trait ErasedItem: fmt::Debug + Send + Sync {
    fn draw(&self, context: &mut CanvasContext);
    fn clone_box(&self) -> Box<dyn ErasedItem>;
    fn equals(&self, other: &dyn ErasedItem) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl<T> ErasedItem for T
where
    T: CanvasItem + Clone + PartialEq,
{
    fn draw(&self, context: &mut CanvasContext) {
        CanvasItem::draw(self, context);
    }

    fn clone_box(&self) -> Box<dyn ErasedItem> {
        Box::new(self.clone())
    }

    fn equals(&self, other: &dyn ErasedItem) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Debug)]
struct Item(Box<dyn ErasedItem>);

impl Clone for Item {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.0.equals(&*other.0)
    }
}

/// A finite drawing surface and its ordered, owned frame items.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Canvas {
    width: Option<usize>,
    height: Option<usize>,
    items: Vec<Item>,
}

impl Canvas {
    pub const fn new() -> Self {
        Self {
            width: None,
            height: None,
            items: Vec::new(),
        }
    }

    /// States the extent used when the width axis is unbounded.
    pub const fn width(mut self, width: usize) -> Self {
        self.width = Some(width);
        self
    }

    /// States the extent used when the height axis is unbounded.
    pub const fn height(mut self, height: usize) -> Self {
        self.height = Some(height);
        self
    }

    /// States both extents used when the corresponding axes are unbounded.
    pub const fn extent(mut self, size: Size) -> Self {
        self.width = Some(size.width());
        self.height = Some(size.height());
        self
    }

    /// Appends one owned item to the frame's drawing order.
    pub fn item<T>(mut self, item: T) -> Self
    where
        T: CanvasItem + Clone + PartialEq,
    {
        self.items.push(Item(Box::new(item)));
        self
    }

    pub(in crate::view) const fn explicit_width(&self) -> Option<usize> {
        self.width
    }

    pub(in crate::view) const fn explicit_height(&self) -> Option<usize> {
        self.height
    }

    fn draw(&self, size: Size) -> CanvasContext {
        let mut context = CanvasContext::new(size);
        for item in &self.items {
            item.0.draw(&mut context);
        }
        context
    }
}
