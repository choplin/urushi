//! Backend-independent terminal contracts and target-specific inspection for
//! the Urushi ecosystem.
//!
//! This crate owns vocabulary shared by terminal surfaces without depending on
//! another Urushi workspace crate.
//! It does not choose rendering policy, perform layout, or render a view.

use std::io::{self, IsTerminal};

pub mod backend;
mod command;
mod event;
mod query;
mod session;
mod style;
mod terminal;

pub use command::{
    ClearRegion, Command, CommandWriter, ControlString, CursorAppearance, CursorMove,
    HyperlinkParameter, InvalidControlString, InvalidTerminalText, TerminalHyperlink,
    TerminalOutput, TerminalText,
};
pub use event::{
    Event, EventSource, FocusChange, KeyCode, KeyEvent, KeyEventState, KeyKind,
    KeyboardEnhancementFlags, MediaKeyCode, ModifierKeyCode, Modifiers, MouseButton, MouseEvent,
    MouseKind,
};
pub use query::{
    KeyboardEnhancementQuery, PixelSize, TerminalBackground, TerminalQuery, WindowSize,
};
pub use session::{RawModeControl, SessionError, SessionOptions, TerminalSession};
pub use style::{
    Color, TerminalStyle, TextAttribute, TextAttributeIter, TextAttributes, Underline,
    UnderlineStyle,
};
pub use terminal::Position;

/// A complete interactive terminal connection.
///
/// Implementations own the input and output paths, parser state, process-side
/// modes, and terminal queries for one physical terminal connection. The
/// smaller supertraits remain independently useful in tests and adapters.
pub trait TerminalBackend:
    CommandWriter + EventSource + RawModeControl + TerminalQuery + KeyboardEnhancementQuery
{
}

impl<T> TerminalBackend for T where
    T: CommandWriter + EventSource + RawModeControl + TerminalQuery + KeyboardEnhancementQuery
{
}

/// The color fidelity a terminal can display.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ColorLevel {
    /// No terminal color sequences.
    #[default]
    None,
    /// The sixteen ANSI colors.
    Ansi16,
    /// The xterm 256-color palette.
    Ansi256,
    /// 24-bit RGB colors.
    TrueColor,
}

/// A set of underline shapes supported by a terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct UnderlineStyles(u8);

impl UnderlineStyles {
    pub const SINGLE: Self = Self(1 << 0);
    pub const DOUBLE: Self = Self(1 << 1);
    pub const CURLY: Self = Self(1 << 2);
    pub const DOTTED: Self = Self(1 << 3);
    pub const DASHED: Self = Self(1 << 4);

    const ALL_BITS: u8 =
        Self::SINGLE.0 | Self::DOUBLE.0 | Self::CURLY.0 | Self::DOTTED.0 | Self::DASHED.0;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, style: UnderlineStyle) -> bool {
        let selected = match style {
            UnderlineStyle::Single => Self::SINGLE,
            UnderlineStyle::Double => Self::DOUBLE,
            UnderlineStyle::Curly => Self::CURLY,
            UnderlineStyle::Dotted => Self::DOTTED,
            UnderlineStyle::Dashed => Self::DASHED,
        };
        self.0 & selected.0 != 0
    }
}

impl std::ops::BitOr for UnderlineStyles {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// A terminal graphics protocol that can display raster images.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalGraphicsProtocol {
    Kitty,
    Sixel,
}

/// A set of terminal graphics protocols supported by a terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TerminalGraphicsProtocols(u8);

impl TerminalGraphicsProtocols {
    pub const KITTY: Self = Self(1 << 0);
    pub const SIXEL: Self = Self(1 << 1);

    const ALL_BITS: u8 = Self::KITTY.0 | Self::SIXEL.0;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, protocol: TerminalGraphicsProtocol) -> bool {
        let selected = match protocol {
            TerminalGraphicsProtocol::Kitty => Self::KITTY,
            TerminalGraphicsProtocol::Sixel => Self::SIXEL,
        };
        self.0 & selected.0 != 0
    }
}

impl std::ops::BitOr for TerminalGraphicsProtocols {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

/// Rendering features positively confirmed by a terminal query or supplied by
/// an explicitly configured backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalCapabilities {
    color_level: ColorLevel,
    attributes: TextAttributes,
    underline_styles: UnderlineStyles,
    underline_colors: bool,
    hyperlinks: bool,
    graphics_protocols: TerminalGraphicsProtocols,
}

impl TerminalCapabilities {
    /// Returns capabilities with every output feature disabled.
    pub const fn none() -> Self {
        Self {
            color_level: ColorLevel::None,
            attributes: TextAttributes::empty(),
            underline_styles: UnderlineStyles::empty(),
            underline_colors: false,
            hyperlinks: false,
            graphics_protocols: TerminalGraphicsProtocols::empty(),
        }
    }

    pub const fn color_level(self) -> ColorLevel {
        self.color_level
    }

    pub const fn attributes(self) -> TextAttributes {
        self.attributes
    }

    pub const fn underline_styles(self) -> UnderlineStyles {
        self.underline_styles
    }

    pub const fn underline_colors(self) -> bool {
        self.underline_colors
    }

    pub const fn hyperlinks(self) -> bool {
        self.hyperlinks
    }

    pub const fn graphics_protocols(self) -> TerminalGraphicsProtocols {
        self.graphics_protocols
    }

    pub const fn supports_graphics(self, protocol: TerminalGraphicsProtocol) -> bool {
        self.graphics_protocols.contains(protocol)
    }

    pub const fn with_color_level(mut self, color_level: ColorLevel) -> Self {
        self.color_level = color_level;
        self
    }

    pub const fn with_attributes(mut self, attributes: TextAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    pub const fn with_underline_styles(mut self, styles: UnderlineStyles) -> Self {
        self.underline_styles = styles;
        self
    }

    pub const fn with_underline_colors(mut self, enabled: bool) -> Self {
        self.underline_colors = enabled;
        self
    }

    pub const fn with_hyperlinks(mut self, enabled: bool) -> Self {
        self.hyperlinks = enabled;
        self
    }

    pub const fn with_graphics_protocols(mut self, protocols: TerminalGraphicsProtocols) -> Self {
        self.graphics_protocols = protocols;
        self
    }
}

/// The visible dimensions of a terminal in character cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TerminalSize {
    columns: usize,
    rows: usize,
}

impl TerminalSize {
    /// The empty terminal surface.
    pub const ZERO: Self = Self::new(0, 0);

    pub const fn new(columns: usize, rows: usize) -> Self {
        Self { columns, rows }
    }

    pub const fn columns(self) -> usize {
        self.columns
    }

    pub const fn rows(self) -> usize {
        self.rows
    }
}

/// Information observed from one terminal output handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalInfo {
    size: TerminalSize,
    capabilities: TerminalCapabilities,
}

impl TerminalInfo {
    pub const fn new(size: TerminalSize, capabilities: TerminalCapabilities) -> Self {
        Self { size, capabilities }
    }

    pub const fn size(self) -> TerminalSize {
        self.size
    }

    pub const fn capabilities(self) -> TerminalCapabilities {
        self.capabilities
    }
}

/// Whether one output handle is attached to a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalDetection {
    Terminal(TerminalInfo),
    NonTerminal,
}

#[cfg(unix)]
pub fn detect(output: &(impl IsTerminal + std::os::fd::AsFd)) -> io::Result<TerminalDetection> {
    detect_with(output, |output| terminal_size::terminal_size_of(output))
}

#[cfg(windows)]
pub fn detect(
    output: &(impl IsTerminal + std::os::windows::io::AsHandle),
) -> io::Result<TerminalDetection> {
    detect_with(output, |output| terminal_size::terminal_size_of(output))
}

#[cfg(not(any(unix, windows)))]
pub fn detect(output: &impl IsTerminal) -> io::Result<TerminalDetection> {
    if output.is_terminal() {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "terminal size detection is unavailable on this platform",
        ))
    } else {
        Ok(TerminalDetection::NonTerminal)
    }
}

#[cfg(any(unix, windows))]
fn detect_with<T, F>(output: &T, terminal_size: F) -> io::Result<TerminalDetection>
where
    T: IsTerminal,
    F: FnOnce(&T) -> Option<(terminal_size::Width, terminal_size::Height)>,
{
    if !output.is_terminal() {
        return Ok(TerminalDetection::NonTerminal);
    }
    let (terminal_size::Width(columns), terminal_size::Height(rows)) = terminal_size(output)
        .ok_or_else(|| io::Error::other("failed to query the terminal size"))?;
    Ok(TerminalDetection::Terminal(TerminalInfo::new(
        TerminalSize::new(usize::from(columns), usize::from(rows)),
        TerminalCapabilities::none(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_protocol_capabilities_are_independent() {
        let both = TerminalCapabilities::none().with_graphics_protocols(
            TerminalGraphicsProtocols::KITTY | TerminalGraphicsProtocols::SIXEL,
        );
        assert_eq!(both.graphics_protocols(), TerminalGraphicsProtocols::all());
        assert!(both.supports_graphics(TerminalGraphicsProtocol::Kitty));
        assert!(both.supports_graphics(TerminalGraphicsProtocol::Sixel));
    }

    #[test]
    fn non_terminal_output_is_not_a_terminal_with_missing_capabilities() {
        let file = std::fs::File::open("Cargo.toml").unwrap();
        assert_eq!(detect(&file).unwrap(), TerminalDetection::NonTerminal);
    }
}
