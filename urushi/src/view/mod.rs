//! The view tree, the layout pass that resolves it, and rendered-block
//! composition.

mod ansi;
mod assemble;
mod geometry;
mod height;
mod join;
mod model;
mod rendered;
mod resolve;
mod sizing;
mod width;

pub use geometry::{Available, Size};
pub use join::{join_horizontal, join_vertical};
pub use model::View;
pub use rendered::RenderedBlock;
pub use resolve::{AnchoredRect, ResolvedView, StyledGrapheme, measure, resolve};
