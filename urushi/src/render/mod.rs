//! Adapters from urushi's logical representation to output technologies.

mod ansi;
mod settings;

pub use ansi::render;
pub use settings::RenderSettings;
