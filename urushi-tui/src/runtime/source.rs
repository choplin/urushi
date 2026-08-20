//! What the runtime's own sources observe.
//!
//! An application never reads a terminal, a clock, or a signal handler itself.
//! It declares a subscription to one of the runtime's sources and supplies the
//! function that turns the source's own value — an [`Input`], a [`Surface`], a
//! [`Signal`] — into the application's message. These are those values.

use std::fmt;

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
}

/// One key event: the key, the modifiers held with it, and whether it was
/// pressed, repeated, or released.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Which key.
    pub code: KeyCode,
    /// The modifiers held with it.
    pub modifiers: Modifiers,
    /// Press, repeat, or release.
    pub kind: KeyKind,
}

impl KeyEvent {
    /// A press of `code` with no modifier held.
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyKind::Press,
        }
    }

    /// A press of `code` with `modifiers` held.
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyKind::Press,
        }
    }
}

/// Whether a [`KeyEvent`] is a press, the terminal's repeat of a held key, or a
/// release.
///
/// Each repeat is its own event: the runtime delivers every one, so a model
/// that counts key presses counts what the user did. A release is reported only
/// where the terminal supports keyboard enhancement and the session asked for
/// releases; without both, only [`KeyKind::Press`] and [`KeyKind::Repeat`]
/// occur.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyKind {
    /// The key went down.
    #[default]
    Press,
    /// The terminal repeated a held key.
    Repeat,
    /// The key came up.
    Release,
}

/// Which key a [`KeyEvent`] is.
///
/// These are the keys that carry text or move a cursor. A key that carries
/// neither — a lock key, a media key, a modifier reported on its own — is not
/// delivered: an application has nothing to do with it, and a terminal reports
/// it only under keyboard enhancement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key, as the character it produces.
    Char(char),
    /// Enter or Return.
    Enter,
    /// Tab.
    Tab,
    /// Shift-Tab, which terminals report as a key of its own.
    BackTab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Escape.
    Escape,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Arrow up.
    Up,
    /// Arrow down.
    Down,
    /// Arrow left.
    Left,
    /// Arrow right.
    Right,
    /// A function key, as its number.
    Function(u8),
}

/// The modifier keys held during a [`KeyEvent`].
///
/// These three are what a terminal reports without keyboard enhancement, and
/// they are what a terminal application binds. Super and Meta are left out: a
/// terminal that cannot report them would make any binding on them
/// unreachable.
///
/// ```
/// use urushi_tui::Modifiers;
///
/// let control_alt = Modifiers { control: true, alt: true, ..Modifiers::NONE };
///
/// assert_eq!(control_alt, Modifiers::CONTROL.union(Modifiers::ALT));
/// ```
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Shift was held.
    pub shift: bool,
    /// Control was held.
    pub control: bool,
    /// Alt was held.
    pub alt: bool,
}

impl Modifiers {
    /// No modifier held.
    pub const NONE: Self = Self {
        shift: false,
        control: false,
        alt: false,
    };
    /// Shift alone.
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    /// Control alone.
    pub const CONTROL: Self = Self {
        control: true,
        ..Self::NONE
    };
    /// Alt alone.
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };

    /// The modifiers of `self` and `other` together.
    pub const fn union(self, other: Self) -> Self {
        Self {
            shift: self.shift || other.shift,
            control: self.control || other.control,
            alt: self.alt || other.alt,
        }
    }
}

impl fmt::Debug for Modifiers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let held: Vec<&str> = [
            ("shift", self.shift),
            ("control", self.control),
            ("alt", self.alt),
        ]
        .into_iter()
        .filter_map(|(name, is_held)| is_held.then_some(name))
        .collect();
        if held.is_empty() {
            return formatter.write_str("Modifiers(none)");
        }
        write!(formatter, "Modifiers({})", held.join("+"))
    }
}

/// Whether the terminal window gained or lost focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusChange {
    /// The window gained focus.
    Gained,
    /// The window lost focus.
    Lost,
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
    pub const fn new(columns: u16, rows: u16) -> Self {
        Self {
            size: SurfaceSize { columns, rows },
            cell_pixels: None,
        }
    }
}

/// The drawable area in cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SurfaceSize {
    /// Columns.
    pub columns: u16,
    /// Rows.
    pub rows: u16,
}

/// How large one cell is in pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellPixels {
    /// Width in pixels.
    pub width: u16,
    /// Height in pixels.
    pub height: u16,
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
