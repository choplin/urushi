//! The view tree, the layout pass that resolves it, and pre-render composition.

mod assemble;
mod canvas;
mod geometry;
mod grid;
mod height;
mod model;
mod resolve;
mod sizing;
mod width;

pub(crate) use canvas::sizing::{CanvasMeasure, CanvasRequirements};
pub use canvas::{
    Canvas, CanvasCell, CanvasContext, CanvasItem, CanvasSizing, CellContribution, Composition,
    LineContinuations, LineGlyphs, LineNetwork, Position, PositionedCell,
};
pub use geometry::{Available, Size};
pub(crate) use height::fit_text_lines;
pub use model::{BlockTitle, Projection, ProjectionBoundary, View, Viewport};
pub use resolve::{
    AnchoredRect, Axis, LayoutError, LayoutErrorKind, ResolvedView, Resolver, StyledGrapheme,
    VisibleRect, measure, resolve, try_measure,
};
pub(crate) use sizing::{Claim, Kind, distribute};
