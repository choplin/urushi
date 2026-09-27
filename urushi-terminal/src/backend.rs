//! Physical implementations of the Urushi terminal contracts.

/// ANSI/ECMA-48 output implemented without a terminal framework dependency.
pub mod ansi;

/// Native Unix terminal connection owned entirely by Urushi.
#[cfg(unix)]
pub mod native;

/// Crossterm implementation of the Urushi backend contracts.
#[cfg(feature = "crossterm")]
pub mod crossterm;
