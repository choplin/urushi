//! Ratatui integration for Urushi.
//!
//! This crate provisionally owns full-screen TUI concerns. It provides the
//! [`ratatui`] adapter — logical-style conversion and stateless widgets that
//! draw a resolved Urushi view into a caller-owned buffer — and, behind the
//! `runtime` feature, a blocking full-screen runtime with [`Application`],
//! [`Effect`], [`Subscription`], and [`Runtime`]. The runtime owns effect and
//! subscription execution, frame scheduling, terminal input and presentation,
//! and session restoration.
//!
//! The adapter computes no geometry. The box model lives in `urushi`'s layout
//! pass, and the adapter only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.
//!
//! [`terminal`] is the lower-level full-screen presentation layer: frame and
//! commit contracts plus a cell writer that lowers changed cells to
//! `urushi-terminal` commands. The optional TEA runtime is layered above it.

mod cell;
pub mod ratatui;
#[cfg(feature = "runtime")]
mod runtime;
pub mod terminal;

pub use urushi_terminal::{Position, TerminalSize};

#[cfg(feature = "crossterm")]
pub use urushi_terminal::backend::crossterm;

#[cfg(feature = "runtime")]
pub use runtime::{
    Admission, Application, BlockingTask, CellPixels, Clock, CustomTerminal, DefaultPresentation,
    DefaultTerminal, Effect, Error, Execution, Executor, FocusChange, Input, KeyCode, KeyEvent,
    KeyEventState, KeyKind, MediaKeyCode, ModifierKeyCode, Modifiers, MouseButton, MouseEvent,
    MouseKind, Runtime, SendError, Sender, Signal, Subscription, Surface, SurfaceSize, Task,
    TokioClock, TokioExecutor,
};

#[cfg(all(feature = "runtime", feature = "crossterm"))]
pub use runtime::run;
