//! Logical styling primitives and box-model rendering.

mod border;
mod color;
mod layout;
mod logical;

pub use border::Border;
pub use color::Color;
pub use layout::{Align, Sides};
pub use logical::Style;

#[cfg(feature = "ratatui")]
pub(crate) use logical::BoxParts;
