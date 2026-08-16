//! Ratatui integration for Urushi.
//!
//! This crate provisionally owns full-screen TUI concerns. It currently
//! provides logical-style conversion and stateless widgets that draw a resolved
//! Urushi view into a caller-owned buffer; runtime state, events, frame
//! scheduling, and terminal lifecycle remain future work.
//!
//! The widgets compute no geometry. The box model lives in `urushi`'s layout
//! pass, and this crate only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.

mod style;
mod widget;

pub use style::RatatuiStyle;
pub use widget::{RatatuiStyleExt, RatatuiWidget, ViewWidget};
