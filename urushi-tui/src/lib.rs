//! Full-screen terminal UI building blocks for Urushi.
//!
//! [`Screen`] is the synchronous presentation engine: it owns Urushi cell
//! buffers and frame history and writes committed changes through a caller's
//! [`urushi_terminal::CommandWriter`]. It does not own input, terminal queries,
//! raw mode, session restoration, an application model, or an async runtime.
//! Callers drive it synchronously from their own loop.
//!
//! [`terminal`] contains the [`Screen`] and [`Frame`] presentation API.
//!
//! # Cargo features
//!
//! - `crossterm` is enabled by default and re-exports the portable Crossterm
//!   backend. Disable default features when the caller supplies another
//!   [`urushi_terminal::CommandWriter`].

mod cell;
pub mod terminal;

pub use terminal::{Frame, Rect, Screen};
pub use urushi_terminal::{Position, TerminalSize};

#[cfg(feature = "crossterm")]
pub use urushi_terminal::backend::crossterm;
