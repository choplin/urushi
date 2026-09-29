//! Full-screen terminal UI building blocks for Urushi.
//!
//! [`Screen`] is the synchronous presentation engine: it owns Urushi cell
//! buffers and frame history and writes committed changes through a caller's
//! [`urushi_terminal::CommandWriter`]. It does not own input, terminal queries,
//! raw mode, or session restoration. Callers may drive it from their own loop
//! without enabling the optional runtime or using Tokio.
//!
//! The [`ratatui`] module is an adapter for logical-style conversion and
//! stateless widgets that draw a resolved Urushi view into a caller-owned
//! Ratatui buffer. Behind the `runtime` feature, the crate also provides a
//! blocking full-screen runtime with [`Application`], [`Effect`],
//! [`Subscription`], and [`Runtime`]. The runtime owns effect and subscription
//! execution, frame scheduling, terminal input, presentation, and session
//! restoration.
//!
//! The adapter computes no geometry. The box model lives in `urushi`'s layout
//! pass, and the adapter only translates a Ratatui `Rect` into
//! [`Available`](urushi::Available), calls [`resolve`](urushi::resolve), and
//! converts the resulting graphemes and logical styles into cells.
//!
//! [`terminal`] contains the lower-level [`Screen`] and [`Frame`] presentation
//! API. The optional TEA runtime is layered above it.
//!
//! The runtime does not inspect terminal appearance or replace an
//! application's theme. A caller that adapts to the terminal background opens
//! its terminal backend first, uses [`urushi_terminal::TerminalQuery`] to
//! select a stable [`urushi::Theme`], constructs the application with that
//! theme, and supplies the same backend to [`Runtime::backend`].

mod cell;
pub mod ratatui;
#[cfg(feature = "runtime")]
mod runtime;
pub mod terminal;

pub use terminal::{Frame, Rect, Screen};
pub use urushi_terminal::{Position, TerminalSize};

#[cfg(feature = "crossterm")]
pub use urushi_terminal::backend::crossterm;

#[cfg(feature = "runtime")]
pub use runtime::{
    Admission, Application, BlockingTask, CellPixels, Clock, DefaultTerminal, Effect, Error,
    Execution, Executor, FocusChange, Input, KeyCode, KeyEvent, KeyEventState, KeyKind,
    MediaKeyCode, ModifierKeyCode, Modifiers, MouseButton, MouseEvent, MouseKind, Runtime,
    SendError, Sender, Signal, Subscription, Surface, SurfaceSize, Task, TokioClock, TokioExecutor,
};

#[cfg(all(feature = "runtime", feature = "crossterm"))]
pub use runtime::run;
