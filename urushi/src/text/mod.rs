//! ANSI-aware text measurement and wrapping.

mod width;
mod wrap;

pub use width::visible_width;
pub(crate) use width::{normalize_ansi_rows, truncate_visible_width};
pub use wrap::wrap_text;
