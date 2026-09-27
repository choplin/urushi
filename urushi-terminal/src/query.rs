//! Queries for state owned or reported by a terminal backend.

use std::io;

use crate::{Position, TerminalSize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PixelSize {
    width: usize,
    height: usize,
}

impl PixelSize {
    pub const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub const fn width(self) -> usize {
        self.width
    }

    pub const fn height(self) -> usize {
        self.height
    }
}

/// Character-cell dimensions and optional pixel geometry for a terminal window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct WindowSize {
    cells: TerminalSize,
    pixels: Option<PixelSize>,
}

impl WindowSize {
    pub const fn new(cells: TerminalSize, pixels: Option<PixelSize>) -> Self {
        Self { cells, pixels }
    }

    pub const fn cells(self) -> TerminalSize {
        self.cells
    }

    /// Returns pixel geometry when the platform reports meaningful values.
    pub const fn pixels(self) -> Option<PixelSize> {
        self.pixels
    }
}

/// Synchronous inspection of terminal state.
///
/// Queries take mutable access because some terminals answer only after the
/// backend writes a protocol request and consumes its response from the input
/// stream. This also prevents an event reader from racing that exchange.
pub trait TerminalQuery {
    /// Queries the current visible size in character cells.
    fn terminal_size(&mut self) -> io::Result<TerminalSize>;

    /// Queries the current zero-based cursor position.
    fn cursor_position(&mut self) -> io::Result<Position>;

    /// Queries both cell and pixel dimensions when the platform exposes them.
    fn window_size(&mut self) -> io::Result<WindowSize>;

    /// Reports whether process terminal input is currently in raw mode.
    fn raw_mode_enabled(&mut self) -> io::Result<bool>;
}

impl<T: TerminalQuery + ?Sized> TerminalQuery for &mut T {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        T::terminal_size(self)
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        T::cursor_position(self)
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        T::window_size(self)
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        T::raw_mode_enabled(self)
    }
}

/// Detects whether enhanced keyboard protocol negotiation is available.
///
/// This is separate from general inspection because session acquisition needs
/// only this capability. Backends may answer from platform knowledge or by
/// exchanging a protocol query with the terminal.
pub trait KeyboardEnhancementQuery {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool>;
}

impl<T: KeyboardEnhancementQuery + ?Sized> KeyboardEnhancementQuery for &mut T {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        T::supports_keyboard_enhancement(self)
    }
}
