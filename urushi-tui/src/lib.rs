//! Full-screen terminal UI building blocks for Urushi.
//!
//! [`Screen`] is the synchronous presentation engine: it owns Urushi cell
//! buffers and frame history and writes committed changes through a caller's
//! [`urushi_terminal::CommandWriter`]. It does not own input, terminal queries,
//! raw mode, session restoration, an application model, or an async runtime.
//! Callers drive it synchronously from their own loop.
//!
//! The [`ratatui`] module is an adapter for logical-style conversion and
//! stateless widgets that draw a resolved Urushi view into a caller-owned
//! Ratatui buffer. Applications that want Urushi's TEA model, effects,
//! subscriptions, frame scheduling, terminal input, and session ownership use
//! the separate `urushi-tui-app` crate above this synchronous engine.
//!
//! The adapter computes no geometry. The box model lives in `urushi`'s layout
//! pass, and the adapter only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.
//!
//! [`terminal`] contains the [`Screen`] and [`Frame`] presentation API.

mod cell;
pub mod ratatui;
pub mod terminal;

pub use terminal::{Frame, Rect, Screen};
pub use urushi_terminal::{Position, TerminalSize};

#[cfg(feature = "crossterm")]
pub use urushi_terminal::backend::crossterm;
