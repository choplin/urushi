//! The full-screen TUI runtime.
//!
//! An application is a value: [`Application`] describes a program, its `Model`
//! is the state the runtime owns, and its `update` returns an [`Effect`] rather
//! than performing one. Everything the program wants to hear from — terminal
//! input included — it declares as a [`Subscription`]. Building those values
//! reads no terminal and starts no work; the runtime's executor machinery
//! interprets them behind boundaries that do not appear in application types.

mod application;
mod core;
mod delivery;
mod effect;
mod entry;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the public runtime entry point")
)]
pub(crate) mod evaluator;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the runtime core and test harness")
)]
mod executor;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the public runtime entry point")
)]
mod presentation;
pub(crate) mod renderer;
mod scheduler;
mod source;
mod sources;
mod subscription;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "wired by the public runtime entry point")
)]
mod terminal_source;
#[cfg(test)]
mod testing;

pub use application::Application;
pub use delivery::{Admission, SendError, Sender};
pub use effect::Effect;
#[cfg(feature = "crossterm")]
pub use entry::run;
pub use entry::{CustomTerminal, DefaultPresentation, DefaultTerminal, Error, Runtime};
pub use executor::{BlockingTask, Clock, Execution, Executor, Task, TokioClock, TokioExecutor};
pub use source::{
    CellPixels, FocusChange, Input, KeyCode, KeyEvent, KeyEventState, KeyKind, MediaKeyCode,
    ModifierKeyCode, Modifiers, MouseButton, MouseEvent, MouseKind, Signal, Surface, SurfaceSize,
};
pub use subscription::Subscription;
