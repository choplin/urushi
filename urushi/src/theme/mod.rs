//! Semantic themes for terminal consumers.
//!
//! # Extending a theme
//!
//! [`Theme`] is a concrete type with no application-specific slot. An
//! application extends it by defining its own role type and implementing
//! [`ThemeRole`] for it, which keeps the derived style following theme
//! overrides and light/dark selection instead of freezing it at construction
//! time.
//!
//! 1. A role that derives purely from [`SemanticTokens`] or an existing
//!    built-in role stays parameterless.
//! 2. A role that needs a parameter carries it in the role value, so
//!    `Heading(level)` or `SeriesColor(index)` resolve like any other role.
//! 3. A role that needs data the theme cannot provide — a brand palette, colors
//!    read from a config file — carries a reference to it in the role value, or
//!    the application owns a composite type wrapping [`Theme`] alongside that
//!    data.
//! 4. Terminal-capability downgrading is not part of resolution; it stays in
//!    [`crate::TerminalProfile::resolve_style`].
//!
//! ```
//! use urushi::{Color, ComponentRole, SemanticTokens, Style, Theme, ThemeRole};
//!
//! #[derive(Clone, Copy)]
//! struct ReportTitle;
//!
//! impl ThemeRole for ReportTitle {
//!     fn resolve(self, theme: &Theme) -> Style {
//!         theme
//!             .style(ComponentRole::Body)
//!             .foreground(theme.tokens().accent)
//!             .bold()
//!     }
//! }
//!
//! # let tokens = SemanticTokens {
//! #     text: Color::WHITE,
//! #     text_muted: Color::BRIGHT_BLACK,
//! #     background: Color::BLACK,
//! #     surface: Color::BLACK,
//! #     accent: Color::BLUE,
//! #     accent_text: Color::WHITE,
//! #     success: Color::GREEN,
//! #     warning: Color::YELLOW,
//! #     error: Color::RED,
//! #     border: Color::BRIGHT_BLACK,
//! # };
//! let theme = Theme::from_tokens(tokens);
//! println!("{}", theme.style(ReportTitle).render("report"));
//! ```
//!
//! # Resolving in a draw loop
//!
//! [`Theme::style`] returns an owned [`Style`](crate::Style), because a role
//! that must be re-resolved every frame is one whose style is derived rather
//! than stored. When many roles are resolved per frame, resolve them once into
//! a struct of styles and borrow from that struct while drawing;
//! `urushi-prompt`'s renderer does exactly this. Built-in styles are also
//! reachable as borrows through [`Theme::components`].

mod component_styles;
mod definition;
mod role;
mod tokens;

pub use component_styles::ComponentStyles;
pub use definition::{ColorScheme, Theme, ThemeSet};
pub use role::{ComponentRole, ListRole, TableRole, ThemeRole, TreeRole};
pub use tokens::SemanticTokens;

#[cfg(test)]
mod tests;
