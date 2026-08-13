//! Terminal capability resolution and optional stderr lifecycle support.

mod palette;
mod profile;
#[cfg(feature = "terminal")]
mod progress;
#[cfg(feature = "terminal")]
mod stderr;

pub use profile::{AnsiPolicy, ColorProfile, TerminalProfile};
#[cfg(feature = "terminal")]
pub use progress::{ProgressBar, Spinner};
#[cfg(feature = "terminal")]
pub use stderr::{OutputMode, StderrTerminal};
