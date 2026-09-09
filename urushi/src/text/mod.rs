//! Plain-text measurement and wrapping.
//!
//! Everything here works on [`PrintableText`] and [`PrintableLines`]: text
//! that carries no escape sequences and no cursor movement. Rendered output is measured by
//! [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead, which
//! is the crate's only ANSI-aware path.

mod printable;
mod styled;
pub(crate) mod width;
mod wrap;

pub use printable::{Grapheme, PrintableLines, PrintableText};
pub(crate) use styled::StyledTextGrapheme;
pub use styled::{StyledText, StyledTextError, TextSpan};
pub(crate) use wrap::{wrap_styled_lines, wrap_text, wrapped_line_count};
