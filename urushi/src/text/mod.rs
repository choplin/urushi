//! ANSI-aware text measurement and wrapping.

mod width;
mod wrap;

pub use width::visible_width;
pub use wrap::wrap_text;
