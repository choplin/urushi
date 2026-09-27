//! Terminal events normalized independently of an input backend.

use std::{fmt, io, time::Duration};

use crate::{Position, TerminalSize};

/// Extra information requested from terminals implementing an enhanced
/// keyboard protocol.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyboardEnhancementFlags(u8);

impl KeyboardEnhancementFlags {
    pub const DISAMBIGUATE_ESCAPE_CODES: Self = Self(1 << 0);
    pub const REPORT_EVENT_TYPES: Self = Self(1 << 1);
    pub const REPORT_ALTERNATE_KEYS: Self = Self(1 << 2);
    pub const REPORT_ALL_KEYS_AS_ESCAPE_CODES: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The Kitty keyboard protocol bit field written on the wire.
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for KeyboardEnhancementFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// One event read from an interactive terminal.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Event {
    Key(KeyEvent),
    /// Text pasted while bracketed-paste reporting is active.
    Paste(String),
    Focus(FocusChange),
    Mouse(MouseEvent),
    /// A new terminal surface size in cells.
    Resize(TerminalSize),
}

/// One normalized key event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
    pub kind: KeyKind,
    pub state: KeyEventState,
}

impl KeyEvent {
    /// Creates an unmodified key-press event.
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyKind::Press,
            state: KeyEventState::NONE,
        }
    }

    pub const fn with_modifiers(mut self, modifiers: Modifiers) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub const fn with_kind(mut self, kind: KeyKind) -> Self {
        self.kind = kind;
        self
    }

    pub const fn with_state(mut self, state: KeyEventState) -> Self {
        self.state = state;
        self
    }
}

/// Additional state reported with an enhanced keyboard event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KeyEventState(u8);

impl KeyEventState {
    pub const NONE: Self = Self(0);
    pub const KEYPAD: Self = Self(1 << 0);
    pub const CAPS_LOCK: Self = Self(1 << 1);
    pub const NUM_LOCK: Self = Self(1 << 2);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for KeyEventState {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// The transition represented by a key event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyKind {
    #[default]
    Press,
    Repeat,
    Release,
}

/// A backend-independent terminal key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KeyCode {
    Char(char),
    Enter,
    Tab,
    BackTab,
    Backspace,
    Delete,
    Escape,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Up,
    Down,
    Left,
    Right,
    Function(u8),
    Null,
    CapsLock,
    ScrollLock,
    NumLock,
    PrintScreen,
    Pause,
    Menu,
    KeypadBegin,
    Media(MediaKeyCode),
    Modifier(ModifierKeyCode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKeyCode {
    Play,
    Pause,
    PlayPause,
    Reverse,
    Stop,
    FastForward,
    Rewind,
    TrackNext,
    TrackPrevious,
    Record,
    LowerVolume,
    RaiseVolume,
    MuteVolume,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ModifierKeyCode {
    LeftShift,
    LeftControl,
    LeftAlt,
    LeftSuper,
    LeftHyper,
    LeftMeta,
    RightShift,
    RightControl,
    RightAlt,
    RightSuper,
    RightHyper,
    RightMeta,
    IsoLevel3Shift,
    IsoLevel5Shift,
}

/// The keyboard modifiers held for one input event.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers(u8);

impl Modifiers {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CONTROL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    pub const HYPER: Self = Self(1 << 4);
    pub const META: Self = Self(1 << 5);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl fmt::Debug for Modifiers {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        formatter.write_str("Modifiers(")?;
        for (name, held) in [
            ("shift", self.contains(Self::SHIFT)),
            ("control", self.contains(Self::CONTROL)),
            ("alt", self.contains(Self::ALT)),
            ("super", self.contains(Self::SUPER)),
            ("hyper", self.contains(Self::HYPER)),
            ("meta", self.contains(Self::META)),
        ] {
            if held {
                if !first {
                    formatter.write_str("+")?;
                }
                formatter.write_str(name)?;
                first = false;
            }
        }
        if first {
            formatter.write_str("none")?;
        }
        formatter.write_str(")")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FocusChange {
    Gained,
    Lost,
}

/// One normalized mouse event at a zero-based cell position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MouseEvent {
    pub kind: MouseKind,
    pub position: Position,
    pub modifiers: Modifiers,
}

/// A normalized mouse action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MouseKind {
    Down(MouseButton),
    Up(MouseButton),
    Drag(MouseButton),
    Moved,
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Blocking and non-blocking reads from one normalized event source.
pub trait EventSource {
    /// Blocks until one supported terminal event is available.
    fn read_event(&mut self) -> io::Result<Event>;

    /// Returns one already-waiting event without blocking.
    fn poll_event(&mut self) -> io::Result<Option<Event>>;

    /// Waits up to `timeout` for one event.
    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>>;
}

impl<T: EventSource + ?Sized> EventSource for &mut T {
    fn read_event(&mut self) -> io::Result<Event> {
        T::read_event(self)
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        T::poll_event(self)
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        T::poll_event_timeout(self, timeout)
    }
}
