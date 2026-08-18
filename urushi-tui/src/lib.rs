//! Ratatui integration for Urushi.
//!
//! This crate provisionally owns full-screen TUI concerns. It currently
//! provides the [`ratatui`] adapter: logical-style conversion and stateless
//! widgets that draw a resolved Urushi view into a caller-owned buffer.
//! Runtime state, events, frame scheduling, and terminal lifecycle remain
//! future work, and land behind the `runtime` feature.
//!
//! The adapter computes no geometry. The box model lives in `urushi`'s layout
//! pass, and the adapter only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.

pub mod ratatui;
