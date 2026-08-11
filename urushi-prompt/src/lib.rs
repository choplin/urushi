//! Blocking, inline terminal prompt runtime styled with [`urushi`].
//!
//! [`Form::run`] owns a short-lived terminal session and blocks until the form
//! is submitted, cancelled, or terminal I/O fails. Field controls and the
//! themed inline renderer are added separately; this crate currently exposes
//! the runtime's stable construction and result boundaries.

pub use urushi;

mod input;
pub(crate) mod runtime;

#[cfg(test)]
mod runtime_extension_test;

pub use input::{Input, ValidationError, Validator};
pub use runtime::{
    Field, FieldConfigError, FieldKey, Form, FormBuildError, FormBuilder, FormOutcome, FormValues,
    Group, GroupBuildError, GroupBuilder, IoOperation, RunError,
};
