//! The full-screen TUI runtime.
//!
//! An application is a value: [`Application`] describes a program, its `Model`
//! is the state the runtime owns, and its `update` returns an [`Effect`] rather
//! than performing one. Everything the program wants to hear from — terminal
//! input included — it declares as a [`Subscription`]. Building those values
//! reads no terminal and starts no work; the runtime's executor machinery
//! interprets them behind boundaries that do not appear in application types.

mod application;
mod delivery;
mod effect;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the runtime core and test harness")
)]
mod executor;
mod source;
mod subscription;
#[cfg(test)]
mod testing;

pub use application::Application;
pub use delivery::{Admission, SendError, Sender};
pub use effect::Effect;
pub use source::{
    CellPixels, FocusChange, Input, KeyCode, KeyEvent, KeyEventState, KeyKind, MediaKeyCode,
    ModifierKeyCode, Modifiers, MouseButton, MouseEvent, MouseKind, Signal, Surface, SurfaceSize,
};
pub use subscription::Subscription;
