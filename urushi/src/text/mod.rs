//! ANSI-aware text measurement and wrapping.

mod width;
mod wrap;

pub(crate) use width::truncate_visible_width;
pub use width::visible_width;
pub use wrap::wrap_text;
