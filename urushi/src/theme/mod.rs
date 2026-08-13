//! Semantic themes for terminal consumers.

mod component_styles;
mod definition;
mod role;
mod tokens;

pub use component_styles::ComponentStyles;
pub use definition::{ColorScheme, Theme, ThemeSet};
pub use role::{ComponentRole, ThemeRole};
pub use tokens::SemanticTokens;

#[cfg(test)]
mod tests;
