//! The full-screen TUI runtime.
//!
//! An application is a value: [`Application`] describes a program, its `Model`
//! is the state the runtime owns, and its `update` returns an [`Effect`] rather
//! than performing one. Everything the program wants to hear from — terminal
//! input included — it declares as a [`Subscription`]. Nothing in this module
//! reads a terminal, runs a closure, polls a future, or draws; those belong to
//! the runtime that interprets these values.

mod application;
mod delivery;
mod effect;
mod source;
mod subscription;
#[cfg(test)]
mod testing;

pub use application::Application;
pub use delivery::{Admission, SendError, Sender};
pub use effect::Effect;
pub use source::{
    CellPixels, FocusChange, Input, KeyCode, KeyEvent, KeyKind, Modifiers, Signal, Surface,
    SurfaceSize,
};
pub use subscription::Subscription;
