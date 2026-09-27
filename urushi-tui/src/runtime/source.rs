//! What the runtime's own sources observe.
//!
//! An application never reads a terminal, a clock, or a signal handler itself.
//! It declares a subscription to one of the runtime's sources and supplies the
//! function that turns the source's own value — an [`Input`], a [`Surface`], a
//! [`Signal`] — into the application's message. These are those values.

pub use urushi_terminal::{
    FocusChange, KeyCode, KeyEvent, KeyEventState, KeyKind, MediaKeyCode, ModifierKeyCode,
    Modifiers, MouseButton, MouseEvent, MouseKind, PixelSize as CellPixels,
    TerminalSize as SurfaceSize,
};

/// Terminal input as it reaches an application.
///
/// The terminal session decides which of these the terminal produces at all:
/// bracketed paste turns a paste into one [`Input::Paste`] instead of a run of
/// keys, focus reporting produces [`Input::Focus`], and keyboard enhancement
/// decides whether a key release can be reported at all. Those are options on
/// the runtime's entry point, not on the subscription.
///
/// Text arrives as [`KeyCode::Char`] keys and as pastes; there is no separate
/// text event. A size change is not input — it is the [`Surface`] source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// A key the terminal reported.
    Key(KeyEvent),
    /// A bracketed paste, delivered whole rather than as its keys.
    Paste(String),
    /// The terminal window gained or lost focus.
    Focus(FocusChange),
    /// A mouse action reported while capture is enabled.
    Mouse(MouseEvent),
}

/// What an application can observe about the surface it draws on.
///
/// An application needs these facts only where they carry application meaning —
/// a viewport measured in cells, a layout that changes with the width. They
/// reach `update` as a message and are read from the model by `view`, which
/// receives no surface input of its own.
///
/// What the terminal can do with graphics is not here. Cell output plus
/// terminal graphics is an extension boundary the cell-only runtime is proven
/// before, and a capability an application cannot yet act on would be a fact
/// with no reader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Surface {
    /// The drawable area, in cells.
    pub size: SurfaceSize,
    /// How large one cell is in pixels, where the terminal reports it.
    pub cell_pixels: Option<CellPixels>,
}

impl Surface {
    /// A surface of `columns` by `rows` cells, whose pixel geometry the
    /// terminal did not report.
    pub const fn new(columns: usize, rows: usize) -> Self {
        Self {
            size: SurfaceSize::new(columns, rows),
            cell_pixels: None,
        }
    }
}

/// A process signal an application can subscribe to.
///
/// A handler is installed only for a signal the application declared, so a
/// signal it did not declare keeps the process's default disposition.
///
/// `SIGWINCH` is not here: a size change is the [`Surface`] source. Nor does
/// [`Signal::Interrupt`] cover `Ctrl-C`, which under raw mode is an ordinary
/// key on the [`Input`] source; it covers the `SIGINT` another process sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signal {
    /// `SIGINT`, sent from outside the terminal.
    Interrupt,
    /// `SIGTERM`, the ordinary request to terminate.
    Terminate,
    /// `SIGHUP`, the terminal that carried this process went away.
    Hangup,
    /// `SIGQUIT`.
    Quit,
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
