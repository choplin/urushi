//! Adapters from urushi's logical representation to output technologies.

mod ansi;
mod palette;
mod settings;

pub use ansi::{render, render_text};
pub use settings::RenderSettings;
