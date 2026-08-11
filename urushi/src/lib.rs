//! Box-model styling for terminal output.
//!
//! `urushi` provides reusable style definitions — colors, text modifiers,
//! padding, margins, borders, and alignment — that render text into plain
//! ANSI strings. The output is a `String`, so it composes with `println!`,
//! logging, or any other place terminal text goes. No terminal setup, raw
//! mode, or event loop is involved.
//!
//! The API is modeled after Go's [lipgloss](https://github.com/charmbracelet/lipgloss).
//!
//! # Example
//!
//! ```
//! use urushi::{Align, Border, Color, Style};
//!
//! let style = Style::new()
//!     .foreground(Color::Ansi256(212))
//!     .border(Border::ROUNDED)
//!     .padding((0, 1))
//!     .align(Align::Center)
//!     .width(20);
//!
//! println!("{}", style.render("こんにちは, urushi!"));
//! ```
//!
//! Width calculations are aware of East Asian wide characters, so padding,
//! borders, and alignment stay correct for CJK text.
//!
//! # Theme-aware CLI output
//!
//! Define light and dark themes once, choose the color scheme explicitly, and
//! detect capabilities for the writer that will receive the rendered string:
//!
//! ```
//! use std::io::stdout;
//!
//! use urushi::{
//!     Color, ColorScheme, ComponentRole, SemanticTokens, TerminalProfile, Theme, ThemeSet,
//! };
//!
//! let light = SemanticTokens {
//!     text: Color::BLACK,
//!     text_muted: Color::BRIGHT_BLACK,
//!     background: Color::WHITE,
//!     surface: Color::BRIGHT_WHITE,
//!     accent: Color::BLUE,
//!     accent_text: Color::WHITE,
//!     success: Color::GREEN,
//!     warning: Color::YELLOW,
//!     error: Color::RED,
//!     border: Color::BRIGHT_BLACK,
//! };
//! let dark = SemanticTokens { surface: Color::BLACK, ..light };
//! let themes = ThemeSet::new(Theme::from_tokens(light), Theme::from_tokens(dark));
//!
//! let theme = themes.select(ColorScheme::Dark);
//! let output = stdout();
//! let profile = TerminalProfile::detect_for(&output);
//! let panel = profile.resolve_style(theme.style(ComponentRole::PanelFocused));
//! assert!(!panel.render("保存しました").is_empty());
//! ```
//!
//! Call [`TerminalProfile::detect_for`] for each output writer rather than
//! reusing a profile from another stream. A non-TTY writer (such as a file or
//! pipe) disables ANSI entirely; a non-empty `NO_COLOR` removes colors but
//! keeps text modifiers. Use [`TerminalProfile::new`] when an application
//! needs a deterministic explicit override instead of detection.

mod border;
mod color;
mod join;
#[cfg(feature = "ratatui")]
mod ratatui;
mod style;
mod terminal;
mod text;
mod theme;

#[cfg(feature = "ratatui")]
pub use self::ratatui::RatatuiStyle;
pub use border::Border;
pub use color::Color;
pub use join::{VerticalAlign, join_horizontal, join_vertical};
pub use style::{Align, Sides, Style};
pub use terminal::{AnsiPolicy, ColorProfile, TerminalProfile};
pub use text::visible_width;
pub use theme::{
    ColorScheme, ComponentRole, ComponentStyles, SemanticTokens, Theme, ThemeRole, ThemeSet,
};
