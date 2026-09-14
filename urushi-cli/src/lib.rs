//! Opinionated presentation for human-facing, non-interactive CLI output.
//!
//! `urushi-cli` owns semantic output such as [`Summary`] and [`Warning`]
//! whose rails, glyphs, hierarchy, and role choices form a CLI visual
//! language. Rendering, layout, terminal inspection, logging policy, live
//! progress, prompts, and full-screen UI remain outside this crate.

mod summary;
#[cfg(test)]
mod test_support;
mod theme;
mod warning;

pub use summary::{Summary, SummaryField, SummaryPresentation};
pub use theme::{CliRole, CliTheme};
pub use warning::{Warning, WarningPresentation};
