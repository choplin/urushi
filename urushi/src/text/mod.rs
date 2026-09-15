//! Plain-text measurement and wrapping.
//!
//! Everything here works on [`PrintableText`] and [`PrintableLines`]: text
//! that carries no escape sequences and no cursor movement. Raw ANSI is outside
//! this plain-text model.

mod printable;
mod styled;
mod tab;
pub(crate) mod width;
mod wrap;

pub use printable::{Grapheme, PrintableLines, PrintableText};
pub(crate) use styled::StyledTextGrapheme;
pub use styled::{StyledText, StyledTextError, TextSpan};
pub use tab::{InvalidTabMarker, TabPolicy};
pub(crate) use wrap::{wrap_styled_lines, wrap_text, wrapped_line_count};
