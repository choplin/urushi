//! Blocking prompt runtime facade.

mod crossterm;
mod crossterm_executor;
mod error;
mod field;
mod form;
pub(crate) mod frame;
mod inline_plan;
mod presentation;
pub(crate) mod resolve;
mod terminal;
mod view;

pub use error::{FieldConfigError, FormBuildError, GroupBuildError, IoOperation, RunError};
pub use field::Field;
pub(crate) use field::{FieldAction, FieldEntry, RuntimeField, private};
pub use form::{
    FieldKey, Form, FormBuilder, FormOutcome, FormValues, Group, GroupBuilder, InlineResizePolicy,
    PromptStart,
};
#[cfg(test)]
pub(crate) use form::{FormState, ReducerResult};
pub(crate) use terminal::{Event, KeyCode, RenderFinish};
#[cfg(test)]
pub(crate) use terminal::{EventSource, KeyEvent, KeyModifiers, Renderer, TerminalControl};
pub(crate) use urushi::TextSpan;
pub(crate) use view::{
    FieldPresentation, FieldRegionKind, LineKind, PromptStyles, PromptView, ViewCursor,
    clipped_line_view, field_line_view, fixed_view, line_view, line_view_with_cursor, region,
    window_spans,
};
#[cfg(test)]
pub(crate) use view::{PromptLine, test_styles};
