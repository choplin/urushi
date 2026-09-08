//! The crate-owned field protocol used by form orchestration.

use std::any::Any;

use super::{
    terminal::Event,
    view::{FieldPresentation, PromptStyles},
};

/// A crate-provided prompt field.
///
/// The trait is sealed: applications compose fields supplied by this crate
/// rather than coupling a field implementation to terminal lifecycle details.
#[allow(
    private_bounds,
    reason = "Field stays externally sealed while crate sibling field modules implement its runtime seam."
)]
pub trait Field: private::Sealed + 'static {}

impl<T> Field for T where T: private::Sealed + 'static {}

pub(crate) mod private {
    pub(crate) trait Sealed {
        fn into_entry(self: Box<Self>) -> super::FieldEntry;
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldState {
    Active,
    Invalid { message: String },
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldAction {
    Stay,
    Handled,
    Accept,
    Back,
    Cancel,
}

pub(crate) struct FieldEntry {
    pub(super) name: String,
    pub(super) state: FieldState,
    pub(super) field: Box<dyn RuntimeField>,
}

impl FieldEntry {
    pub(crate) fn new(name: String, field: Box<dyn RuntimeField>) -> Self {
        Self {
            name,
            state: FieldState::Active,
            field,
        }
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn event(&mut self, event: Event) -> FieldAction {
        let action = self.field.event(event);
        self.state = match self.field.validation_error() {
            Some(message) => FieldState::Invalid {
                message: message.to_owned(),
            },
            None => FieldState::Active,
        };
        action
    }

    pub(super) fn activate(&mut self) {
        self.field.focus();
        self.state = FieldState::Active;
    }

    pub(super) fn deactivate(&mut self) {
        self.field.blur();
    }

    pub(super) fn accept(&mut self) {
        self.field.blur();
        self.state = FieldState::Accepted;
    }

    pub(super) fn take_value(&mut self) -> Box<dyn Any> {
        self.field.take_value()
    }

    pub(super) fn view(
        &self,
        styles: &PromptStyles,
        focused: bool,
        width: usize,
    ) -> FieldPresentation {
        self.field.view(styles, focused, width)
    }

    pub(super) fn captures_tab(&self) -> bool {
        self.field.captures_tab()
    }
}

pub(crate) trait RuntimeField {
    fn event(&mut self, event: Event) -> FieldAction;
    fn take_value(&mut self) -> Box<dyn Any>;
    /// Builds this field's view with theme and profile already resolved.
    ///
    /// `focused` is passed rather than post-processed out of the result: once
    /// a span carries a resolved style, an unfocused field's rows cannot be
    /// derived from a focused field's rows without guessing which role a
    /// style came from.
    ///
    /// `width` is the cells this field will be laid out in. A field that
    /// carries a cursor needs it: the visible window of a value that is wider
    /// than the terminal is chosen here, before a `View` exists, so that
    /// resolution can be called with the real available area.
    fn view(&self, styles: &PromptStyles, focused: bool, width: usize) -> FieldPresentation;

    fn validation_error(&self) -> Option<&str> {
        None
    }

    fn focus(&mut self) {}

    fn blur(&mut self) {}

    fn captures_tab(&self) -> bool {
        false
    }
}
