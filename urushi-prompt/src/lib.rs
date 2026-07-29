//! Interactive terminal prompts styled with [`urushi`].
//!
//! Not implemented yet. The planned design follows
//! [huh](https://github.com/charmbracelet/huh): a `Form` → `Group` → field
//! hierarchy (`Input`, `Select`, `MultiSelect`, `Confirm`, …) with per-field
//! validation and a theme built from [`urushi::Style`] values. The public
//! entry point is a blocking `run()`; internally the UI is event-driven.

pub use urushi;
