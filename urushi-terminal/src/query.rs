//! Queries for state owned or reported by a terminal backend.

use std::io;

use crate::{Position, TerminalCapabilities, TerminalSize};

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

    /// Returns the pixel dimensions of one character cell when the reported
    /// window geometry divides into a uniform non-empty cell grid.
    pub const fn cell_pixels(self) -> Option<PixelSize> {
        let Some(pixels) = self.pixels else {
            return None;
        };
        if self.cells.columns() == 0
            || self.cells.rows() == 0
            || pixels.width() % self.cells.columns() != 0
            || pixels.height() % self.cells.rows() != 0
        {
            return None;
        }
        let width = pixels.width() / self.cells.columns();
        let height = pixels.height() / self.cells.rows();
        if width == 0 || height == 0 {
            None
        } else {
            Some(PixelSize::new(width, height))
        }
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

    /// Queries rendering capabilities confirmed by the terminal.
    ///
    /// Backends that cannot exchange capability queries return an empty set
    /// rather than inferring support from environment variables or terminal
    /// names.
    fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
        Ok(TerminalCapabilities::none())
    }
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

    fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
        T::terminal_capabilities(self)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_pixels_require_exact_uniform_geometry() {
        assert_eq!(
            WindowSize::new(TerminalSize::new(80, 24), Some(PixelSize::new(800, 480)),)
                .cell_pixels(),
            Some(PixelSize::new(10, 20))
        );
        assert_eq!(
            WindowSize::new(TerminalSize::new(80, 24), Some(PixelSize::new(801, 480)),)
                .cell_pixels(),
            None
        );
        assert_eq!(WindowSize::default().cell_pixels(), None);
    }
}
