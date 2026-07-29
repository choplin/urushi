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

mod border;
mod color;
mod style;
mod text;

pub use border::Border;
pub use color::Color;
pub use style::{Align, Sides, Style};
pub use text::visible_width;
