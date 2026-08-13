//! Colors named for their display meaning.

use crate::Color;

/// Colors named for their display meaning rather than a rendering technology.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticTokens {
    pub text: Color,
    pub text_muted: Color,
    pub background: Color,
    pub surface: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub border: Color,
}
