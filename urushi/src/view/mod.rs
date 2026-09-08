//! The view tree, the layout pass that resolves it, and rendered-block
//! composition.

mod ansi;
mod assemble;
mod canvas;
mod geometry;
mod grid;
mod height;
mod join;
mod model;
mod rendered;
mod resolve;
mod sizing;
mod width;

pub use canvas::{
    Canvas, CanvasCell, CanvasContext, CanvasItem, CanvasSizing, CellContribution, Composition,
    Path, Position, PositionedCell,
};
pub use geometry::{Available, Size};
pub use join::{join_horizontal, join_vertical};
pub use model::View;
pub use rendered::RenderedBlock;
pub use resolve::{
    AnchoredRect, Axis, LayoutError, LayoutErrorKind, ResolvedView, StyledGrapheme, measure,
    resolve, try_measure, try_resolve,
};
