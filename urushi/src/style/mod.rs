//! Logical styling primitives and the block box model.

mod block;
mod border;
mod grid;
mod hyperlink;
mod layout;
mod text;

pub use block::BlockStyle;
pub use border::Border;
pub use grid::GridStyle;
pub use hyperlink::Hyperlink;
pub use layout::{Align, InvalidFillWeight, Length, Overflow, Sides, VerticalAlign};
pub use text::TextStyle;
pub use urushi_terminal::{
    Color, TextAttribute, TextAttributes, Underline, UnderlineStyle,
    UnderlineStyles as UnderlineStyleSet,
};
