//! Renderer-neutral views and rendered-block composition.

mod join;
mod model;

pub use join::{VerticalAlign, join_horizontal, join_vertical};
pub use model::{Line, Span, View};
