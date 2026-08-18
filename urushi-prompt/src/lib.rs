//! Blocking, inline terminal prompt runtime styled with [`urushi`].
//!
//! [`Form::run`] owns a short-lived terminal session and blocks until the form
//! is submitted, cancelled, or terminal I/O fails. The crate exposes the form
//! runtime ([`Form`], [`FormBuilder`], [`Group`], [`FormOutcome`], [`RunError`])
//! together with the [`Input`], [`Select`], and [`Confirm`] field controls, and
//! re-exports [`urushi`] for styling.

pub use urushi;

mod confirm;
mod input;
pub(crate) mod runtime;
mod select;

#[cfg(test)]
mod runtime_extension_test;

pub use confirm::{Confirm, ConfirmAnswer, ConfirmSource};
pub use input::{Input, ValidationError, Validator};
pub use runtime::{
    Field, FieldConfigError, FieldKey, Form, FormBuildError, FormBuilder, FormOutcome, FormValues,
    Group, GroupBuildError, GroupBuilder, IoOperation, RunError,
};
pub use select::{Select, SelectOption};
