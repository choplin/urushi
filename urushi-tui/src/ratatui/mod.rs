//! The Ratatui backend adapter.
//!
//! The adapter converts Urushi styles and resolved views into Ratatui
//! representations and writes them into a buffer the caller owns. It acquires
//! no application state, no event handling, and no terminal ownership.
//!
//! Two entry points share one cell-writing path. [`ViewWidget`] and
//! [`RatatuiWidget`] serve a plain Ratatui application drawing into a `Rect`
//! it already owns; they resolve the view and discard everything but the
//! cells. A caller that needs the resolution itself — the TUI runtime's
//! renderer resolving once per frame — resolves under [`available`] and writes
//! the result with [`draw_resolved`].

mod style;
mod widget;

pub use style::RatatuiStyle;
pub use widget::{RatatuiStyleExt, RatatuiWidget, ViewWidget, available, draw_resolved};
