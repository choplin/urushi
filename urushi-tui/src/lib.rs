//! Ratatui integration for Urushi.
//!
//! This crate provisionally owns full-screen TUI concerns. It provides the
//! [`ratatui`] adapter — logical-style conversion and stateless widgets that
//! draw a resolved Urushi view into a caller-owned buffer — and, behind the
//! `runtime` feature, the full-screen runtime's application value:
//! [`Application`], [`Effect`], and [`Subscription`]. Frame scheduling,
//! delivery, and terminal lifecycle remain future work behind the same
//! feature.
//!
//! The adapter computes no geometry. The box model lives in `urushi`'s layout
//! pass, and the adapter only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.

pub mod ratatui;
#[cfg(feature = "runtime")]
mod runtime;

#[cfg(feature = "runtime")]
pub use runtime::{
    Admission, Application, CellPixels, Effect, FocusChange, Input, KeyCode, KeyEvent, KeyKind,
    Modifiers, SendError, Sender, Signal, Subscription, Surface, SurfaceSize,
};
