//! Logical styling primitives and the block box model.

mod block;
mod border;
mod color;
mod layout;
mod modifier;
mod property;
mod text;
mod underline;

pub use block::BlockStyle;
pub use border::Border;
pub use color::Color;
pub use layout::{Align, Length, Overflow, Sides, VerticalAlign};
pub use modifier::Modifier;
pub use property::{
    BlockStyleProperty, BlockStylePropertyKey, TextStyleProperty, TextStylePropertyKey,
};
pub use text::TextStyle;
pub use underline::{Underline, UnderlineStyle};
