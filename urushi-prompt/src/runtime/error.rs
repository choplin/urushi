//! Errors exposed by prompt construction and execution.

use std::{fmt, io};

/// The I/O boundary at which a prompt run failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IoOperation {
    /// Entering raw-mode terminal handling failed.
    EnterTerminal,
    /// Reading the next input event failed.
    ReadEvent,
    /// Drawing or finishing the inline prompt failed.
    Render,
    /// Cleanup after an otherwise successful or cancelled run failed.
    Cleanup,
}

impl fmt::Display for IoOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EnterTerminal => "enter terminal session",
            Self::ReadEvent => "read terminal event",
            Self::Render => "render prompt",
            Self::Cleanup => "clean up terminal session",
        })
    }
}

/// An error that prevents a form from completing normally.
#[derive(Debug)]
#[non_exhaustive]
pub enum RunError {
    /// Standard input and standard error are not both interactive terminals.
    NotInteractive,
    /// The terminal was resized while an inline prompt configured with
    /// [`crate::InlineResizePolicy::ReturnError`] was running.
    Resized {
        /// A terminal restoration failure, if cleanup was attempted and failed.
        cleanup: Option<io::Error>,
    },
    /// A runtime operation failed; cleanup is retained separately when it also failed.
    Io {
        /// The primary operation that failed.
        operation: IoOperation,
        /// The primary I/O failure.
        source: io::Error,
        /// The first cleanup failure, if cleanup was attempted after `source`.
        cleanup: Option<io::Error>,
    },
}

impl fmt::Display for RunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotInteractive => formatter
                .write_str("standard input and standard error must be interactive terminals"),
            Self::Resized {
                cleanup: Some(cleanup),
            } => write!(
                formatter,
                "terminal resized during inline prompt; terminal cleanup also failed: {cleanup}"
            ),
            Self::Resized { cleanup: None } => {
                formatter.write_str("terminal resized during inline prompt")
            }
            Self::Io {
                operation,
                source,
                cleanup: Some(cleanup),
            } => write!(
                formatter,
                "failed to {operation}: {source}; terminal cleanup also failed: {cleanup}"
            ),
            Self::Io {
                operation,
                source,
                cleanup: None,
            } => write!(formatter, "failed to {operation}: {source}"),
        }
    }
}

impl std::error::Error for RunError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NotInteractive => None,
            Self::Resized { cleanup } => cleanup
                .as_ref()
                .map(|error| error as &(dyn std::error::Error + 'static)),
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// An invalid top-level form configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FormBuildError {
    /// A form must contain at least one group.
    EmptyForm,
    /// Field names must be unique across all groups.
    DuplicateFieldName(String),
}

impl fmt::Display for FormBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyForm => formatter.write_str("a form must contain at least one group"),
            Self::DuplicateFieldName(name) => {
                write!(formatter, "field name `{name}` is duplicated in the form")
            }
        }
    }
}

impl std::error::Error for FormBuildError {}

/// An invalid group configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupBuildError {
    /// A group must contain at least one field.
    EmptyGroup,
}

impl fmt::Display for GroupBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a prompt group must contain at least one field")
    }
}

impl std::error::Error for GroupBuildError {}

/// A field-specific configuration error reserved for concrete field controls.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldConfigError {
    /// A field name cannot be empty.
    EmptyName,
    /// A select-like field must contain an option.
    EmptyOptions,
}

impl fmt::Display for FieldConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyName => "a prompt field name cannot be empty",
            Self::EmptyOptions => "a select field must contain at least one option",
        })
    }
}

impl std::error::Error for FieldConfigError {}
