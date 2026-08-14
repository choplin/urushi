//! Composable styling and rendering for terminal applications.
//!
//! `urushi` separates logical styles, renderer-neutral component views, output
//! rendering, and terminal lifecycle. Basic [`Style`] rendering returns a
//! `String`, so it composes with `println!`, logging, or any other place text
//! goes. The optional `terminal` feature adds stderr output and live progress.
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
//! Reusable components return [`View`] values containing logical [`Style`]
//! values. [`AnsiRenderer`] resolves those styles for a [`TerminalProfile`] at
//! the output boundary. Applications retain ownership of workflow-specific
//! composition such as command headers and final outcomes.
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
//!
//! Ratatui conversion and widgets live in the separate `urushi-tui` crate so
//! this core crate remains independent from full-screen TUI backends.

mod component;
mod render;
mod style;
mod terminal;
mod text;
mod theme;
mod view;

pub use component::{
    List, ListEnumerator, ListIndenter, ListItem, ListPosition, ListStyle, SiblingPosition,
    Summary, SummaryField, Table, TableCell, TableStyle, TableStyleFunc, Tree, TreeEnumerator,
    TreeIndenter, TreeNode, TreeStyle, Warning, alphabet_enumerator, arabic_enumerator,
    asterisk_enumerator, bullet_enumerator, dash_enumerator, default_list_indenter,
    default_table_style_func, default_tree_enumerator, default_tree_indenter, roman_enumerator,
};
pub use render::AnsiRenderer;
pub use style::{
    Align, Border, Color, Modifier, Sides, Style, StyleProperty, StylePropertyKey, VerticalAlign,
};
pub use terminal::{AnsiPolicy, ColorProfile, TerminalProfile};
#[cfg(feature = "terminal")]
pub use terminal::{OutputMode, ProgressBar, Spinner, StderrTerminal};
pub use text::{visible_width, wrap_text};
pub use theme::{
    ColorScheme, ComponentRole, ComponentStyles, ListRole, SemanticTokens, TableRole, Theme,
    ThemeRole, ThemeSet, TreeRole,
};
pub use view::{Line, Span, View, join_horizontal, join_vertical};
