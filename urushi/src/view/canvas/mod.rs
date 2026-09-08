//! Free-positioned, renderer-neutral drawing inside a finite [`Canvas`].
//!
//! A Canvas carries exactly one [`CanvasSizing`] value. Ordinary canvases use
//! viewport sizing and consume finite allocation; built-in presentations may
//! internally bind intrinsic requirements without deriving them from items.
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
pub(crate) mod sizing;

use std::any::Any;
use std::fmt;

use super::geometry::Size;

pub use cell::{CanvasCell, CellContribution, Composition, PositionedCell};
pub use context::CanvasContext;
pub use path::Path;
pub(crate) use sizing::CanvasRequirements;
pub use sizing::CanvasSizing;

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
    sizing: CanvasSizing,
    items: Vec<Item>,
}

impl Canvas {
    pub const fn new() -> Self {
        Self {
            sizing: CanvasSizing::viewport(),
            items: Vec::new(),
        }
    }

    /// Replaces this Canvas's complete sizing policy.
    ///
    /// Ordinary callers use viewport sizing. Built-in presentations may bind
    /// an opaque intrinsic value internally; sizing remains independent of the
    /// Canvas's ordered items in either case.
    #[must_use]
    pub fn sizing(mut self, sizing: CanvasSizing) -> Self {
        self.sizing = sizing;
        self
    }

    /// States viewport sizing's extent on an unbounded width axis.
    ///
    /// # Panics
    ///
    /// Panics when a crate-provided intrinsic sizing value is already installed.
    pub const fn width(mut self, width: usize) -> Self {
        self.sizing.set_viewport_width(width);
        self
    }

    /// States viewport sizing's extent on an unbounded height axis.
    ///
    /// # Panics
    ///
    /// Panics when a crate-provided intrinsic sizing value is already installed.
    pub const fn height(mut self, height: usize) -> Self {
        self.sizing.set_viewport_height(height);
        self
    }

    /// States both unbounded-axis extents of viewport sizing.
    ///
    /// # Panics
    ///
    /// Panics when a crate-provided intrinsic sizing value is already installed.
    pub const fn extent(mut self, size: Size) -> Self {
        self.sizing.set_viewport_extent(size);
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
        self.sizing.explicit_width()
    }

    pub(in crate::view) const fn explicit_height(&self) -> Option<usize> {
        self.sizing.explicit_height()
    }

    pub(in crate::view) const fn uses_viewport_sizing(&self) -> bool {
        self.sizing.is_viewport()
    }

    pub(in crate::view) fn width_requirements(&self) -> sizing::Requirements {
        self.sizing.width_requirements()
    }

    pub(in crate::view) fn height_requirements(&self, width: usize) -> sizing::Requirements {
        self.sizing.height_requirements(width)
    }

    fn draw(&self, size: Size) -> CanvasContext {
        let mut context = CanvasContext::new(size);
        for item in &self.items {
            item.0.draw(&mut context);
        }
        context
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::sizing::{CanvasMeasure, CanvasRequirements};
    use super::*;
    use crate::{Align, Available, BlockStyle, Grapheme, View, measure, resolve};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Event {
        Width,
        Height(usize),
        Draw(Size),
    }

    #[derive(Debug, Clone)]
    struct StagedMeasure {
        width: CanvasRequirements,
        height: CanvasRequirements,
        events: Arc<Mutex<Vec<Event>>>,
    }

    impl PartialEq for StagedMeasure {
        fn eq(&self, other: &Self) -> bool {
            self.width == other.width && self.height == other.height
        }
    }

    impl CanvasMeasure for StagedMeasure {
        fn width_requirements(&self) -> CanvasRequirements {
            self.events.lock().unwrap().push(Event::Width);
            self.width
        }

        fn height_requirements(&self, width: usize) -> CanvasRequirements {
            self.events.lock().unwrap().push(Event::Height(width));
            self.height
        }
    }

    #[derive(Debug, Clone)]
    struct DrawPastHeight(Arc<Mutex<Vec<Event>>>);

    impl PartialEq for DrawPastHeight {
        fn eq(&self, _other: &Self) -> bool {
            true
        }
    }

    impl CanvasItem for DrawPastHeight {
        fn draw(&self, context: &mut CanvasContext) {
            self.0.lock().unwrap().push(Event::Draw(context.size()));
            context.cells([PositionedCell::new(
                Position::new(0, 2),
                CellContribution::new().symbol(Grapheme::new("x")),
            )]);
        }
    }

    #[test]
    fn intrinsic_measurement_is_staged_once_before_drawing() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sizing = CanvasSizing::intrinsic(StagedMeasure {
            width: CanvasRequirements::new(8, 3),
            height: CanvasRequirements::new(4, 2),
            events: Arc::clone(&events),
        });
        let view = View::canvas(
            Canvas::new()
                .sizing(sizing)
                .item(DrawPastHeight(Arc::clone(&events))),
        );

        assert_eq!(measure(&view), Size::new(8, 4));
        assert_eq!(
            *events.lock().unwrap(),
            [Event::Width, Event::Height(8)],
            "measurement never invokes Canvas items"
        );
        events.lock().unwrap().clear();

        let resolved = resolve(&view, Available::size(5, 2));

        assert_eq!(resolved.size(), Size::new(5, 2));
        assert_eq!(
            *events.lock().unwrap(),
            [Event::Width, Event::Height(5), Event::Draw(Size::new(5, 2))]
        );
        assert!(
            resolved
                .rows()
                .iter()
                .flatten()
                .all(|cell| cell.symbol() != "x"),
            "drawing beyond the selected height is cropped without reflow"
        );
    }

    #[test]
    fn intrinsic_equality_ignores_allocation_identity() {
        let first = CanvasSizing::intrinsic(StagedMeasure {
            width: CanvasRequirements::new(8, 3),
            height: CanvasRequirements::new(4, 2),
            events: Arc::new(Mutex::new(Vec::new())),
        });
        let same_value_in_another_allocation = CanvasSizing::intrinsic(StagedMeasure {
            width: CanvasRequirements::new(8, 3),
            height: CanvasRequirements::new(4, 2),
            events: Arc::new(Mutex::new(Vec::new())),
        });

        assert_eq!(first, same_value_in_another_allocation);
    }

    #[test]
    fn intrinsic_floors_win_before_the_final_safety_crop() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sizing = CanvasSizing::intrinsic(StagedMeasure {
            width: CanvasRequirements::new(8, 3),
            height: CanvasRequirements::new(4, 3),
            events: Arc::clone(&events),
        });
        let view = View::canvas(
            Canvas::new()
                .sizing(sizing)
                .item(DrawPastHeight(Arc::clone(&events))),
        );

        let resolved = resolve(&view, Available::size(1, 1));

        assert_eq!(resolved.size(), Size::new(1, 1));
        assert_eq!(
            *events.lock().unwrap(),
            [Event::Width, Event::Height(3), Event::Draw(Size::new(3, 3))]
        );
    }

    #[test]
    fn selected_width_height_floors_propagate_through_ancestor_claims() {
        let first_events = Arc::new(Mutex::new(Vec::new()));
        let second_events = Arc::new(Mutex::new(Vec::new()));
        let intrinsic = |height, floor, events: &Arc<Mutex<Vec<Event>>>| {
            View::canvas(
                Canvas::new()
                    .sizing(CanvasSizing::intrinsic(StagedMeasure {
                        width: CanvasRequirements::new(1, 0),
                        height: CanvasRequirements::new(height, floor),
                        events: Arc::clone(events),
                    }))
                    .item(DrawPastHeight(Arc::clone(events))),
            )
        };
        let view = View::column(
            Align::Left,
            [
                View::block(BlockStyle::new(), intrinsic(10, 8, &first_events)),
                intrinsic(10, 0, &second_events),
            ],
        );

        let resolved = resolve(&view, Available::size(3, 10));

        assert_eq!(resolved.size(), Size::new(3, 10));
        assert_eq!(
            first_events.lock().unwrap().last(),
            Some(&Event::Draw(Size::new(1, 8)))
        );
        assert_eq!(
            second_events.lock().unwrap().last(),
            Some(&Event::Draw(Size::new(3, 2)))
        );
    }
}
