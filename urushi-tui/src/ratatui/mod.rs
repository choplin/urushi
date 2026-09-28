//! The Ratatui backend adapter.
//!
//! The adapter converts Urushi styles and resolved views into Ratatui
//! representations. Stateless widgets write into a buffer the caller owns;
//! [`RatatuiTerminal`] owns a committed buffer and sends its cell diff through
//! a [`crate::terminal::CellWriter`].
//!
//! Two entry points share one cell-writing path. [`ViewWidget`] and
//! [`RatatuiWidget`] serve a plain Ratatui application drawing into a `Rect`
//! it already owns; they resolve the view and discard everything but the
//! cells. A caller that needs the resolution itself — the TUI runtime's
//! renderer resolving once per frame — resolves under [`available`] and writes
//! the result with [`draw_resolved`]. A caller filling a sized anchor can use
//! [`anchor_placement`] to translate its complete logical rectangle and
//! accumulated visible intersection into a destination and source offset.

mod style;
mod terminal;
mod widget;

pub use style::RatatuiStyle;
pub use terminal::{RatatuiFrame, RatatuiTerminal};
pub use widget::{
    CellWriteMode, RatatuiAnchor, RatatuiStyleExt, RatatuiWidget, ViewWidget, anchor_placement,
    available, draw_resolved, draw_resolved_with_mode,
};
