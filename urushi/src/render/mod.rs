//! Adapters from urushi's logical representation to output technologies.

mod ansi;
#[cfg(feature = "ratatui")]
mod ratatui;

pub use ansi::AnsiRenderer;
#[cfg(feature = "ratatui")]
pub use ratatui::{RatatuiStyle, RatatuiWidget};
