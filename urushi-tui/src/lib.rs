//! Ratatui integration for Urushi.
//!
//! This crate provisionally owns full-screen TUI concerns. It currently
//! provides logical-style conversion and a stateless box-model widget; runtime
//! state, events, frame scheduling, and terminal lifecycle remain future work.

mod style;
mod widget;

pub use style::RatatuiStyle;
pub use widget::{RatatuiStyleExt, RatatuiWidget};
