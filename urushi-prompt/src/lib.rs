//! Blocking, inline terminal prompt runtime styled with [`urushi`].
//!
//! [`Form::run`] opens and owns a short-lived default terminal session, while
//! [`Form::run_with_terminal`] runs on a caller-owned connection after an
//! explicit theme has been selected. Both block until the form is submitted,
//! cancelled, or terminal I/O fails. Neither queries the terminal background
//! or changes the supplied theme. The crate exposes the form runtime ([`Form`],
//! [`FormBuilder`], [`PromptStart`], [`InlineResizePolicy`], [`Group`],
//! [`FormOutcome`], [`RunError`]) together with the [`Input`], [`Select`], and
//! [`Confirm`] field controls, and re-exports [`urushi`] for styling.

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
    Group, GroupBuildError, GroupBuilder, InlineResizePolicy, IoOperation, PromptStart, RunError,
};
pub use select::{Select, SelectOption};
