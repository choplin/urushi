//! Palette conversion and optional private progress-output support.

pub(crate) mod palette;
#[cfg(feature = "terminal")]
mod progress;
#[cfg(feature = "terminal")]
mod stderr;

#[cfg(feature = "terminal")]
pub use progress::{ProgressBar, Spinner};
