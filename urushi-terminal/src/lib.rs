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
    ClearRegion, Command, CommandWriter, CursorAppearance, CursorMove, HyperlinkParameter,
    InvalidTerminalText, TerminalHyperlink, TerminalOutput, TerminalText,
};
pub use event::{
    Event, EventSource, FocusChange, KeyCode, KeyEvent, KeyEventState, KeyKind,
    KeyboardEnhancementFlags, MediaKeyCode, ModifierKeyCode, Modifiers, MouseButton, MouseEvent,
    MouseKind,
};
pub use query::{KeyboardEnhancementQuery, PixelSize, TerminalQuery, WindowSize};
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

/// Rendering features conservatively detected for a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalCapabilities {
    color_level: ColorLevel,
    attributes: TextAttributes,
    underline_styles: UnderlineStyles,
    underline_colors: bool,
    hyperlinks: bool,
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
        detect_capabilities(
            std::env::var("TERM").ok().as_deref(),
            std::env::var("COLORTERM").ok().as_deref(),
        ),
    )))
}

fn detect_capabilities(term: Option<&str>, color_term: Option<&str>) -> TerminalCapabilities {
    if term.is_some_and(|value| value.eq_ignore_ascii_case("dumb")) {
        return TerminalCapabilities::none();
    }

    let term = term.map(str::to_ascii_lowercase);
    let term = term.as_deref().unwrap_or_default();
    let color_level = if is_modern_terminal(term)
        || color_term.is_some_and(|value| {
            value.eq_ignore_ascii_case("truecolor") || value.eq_ignore_ascii_case("24bit")
        }) {
        ColorLevel::TrueColor
    } else if term.contains("256color") {
        ColorLevel::Ansi256
    } else if supports_ansi_colors(term) {
        ColorLevel::Ansi16
    } else {
        ColorLevel::None
    };

    let mut capabilities = TerminalCapabilities::none()
        .with_color_level(color_level)
        .with_attributes(basic_attributes(term))
        .with_underline_styles(if supports_basic_sgr(term) {
            UnderlineStyles::SINGLE
        } else {
            UnderlineStyles::empty()
        });

    if supports_extended_sgr(term) {
        capabilities = capabilities
            .with_attributes(TextAttributes::all())
            .with_underline_styles(UnderlineStyles::all())
            .with_underline_colors(true);
    }
    if supports_hyperlinks(term) {
        capabilities = capabilities.with_hyperlinks(true);
    }
    capabilities
}

fn supports_ansi_colors(term: &str) -> bool {
    [
        "ansi", "color", "cygwin", "konsole", "linux", "rxvt", "screen", "tmux", "xterm",
    ]
    .iter()
    .any(|name| term.contains(name))
        || is_modern_terminal(term)
}

fn supports_basic_sgr(term: &str) -> bool {
    [
        "ansi", "color", "cygwin", "konsole", "linux", "rxvt", "screen", "tmux", "vt100", "xterm",
    ]
    .iter()
    .any(|name| term.contains(name))
        || is_modern_terminal(term)
}

fn basic_attributes(term: &str) -> TextAttributes {
    let bold_blink_reverse =
        TextAttribute::Bold | TextAttribute::SlowBlink | TextAttribute::Reversed;
    let xterm_attributes =
        bold_blink_reverse | TextAttribute::Dim | TextAttribute::Italic | TextAttribute::Hidden;
    if is_modern_terminal(term) {
        TextAttributes::all()
    } else if term == "linux" {
        bold_blink_reverse | TextAttribute::Dim
    } else if term.starts_with("screen") {
        bold_blink_reverse
    } else if term.starts_with("tmux") || term.contains("xterm") {
        xterm_attributes
    } else if term.contains("konsole") {
        bold_blink_reverse | TextAttribute::Italic
    } else if term.contains("ansi") {
        bold_blink_reverse | TextAttribute::Hidden
    } else if term.contains("cygwin") {
        TextAttribute::Bold | TextAttribute::Reversed | TextAttribute::Hidden
    } else if term.contains("rxvt") || term.contains("vt100") {
        bold_blink_reverse
    } else {
        TextAttributes::empty()
    }
}

fn is_modern_terminal(term: &str) -> bool {
    !term.starts_with("screen")
        && !term.starts_with("tmux")
        && ["contour", "foot", "ghostty", "kitty", "rio", "wezterm"]
            .iter()
            .any(|name| term.contains(name))
}

fn supports_extended_sgr(term: &str) -> bool {
    is_modern_terminal(term)
}

fn supports_hyperlinks(term: &str) -> bool {
    // Multiplexers require version- and configuration-sensitive passthrough,
    // so TERM alone is not enough to advertise OSC 8 through them.
    !term.starts_with("screen") && !term.starts_with("tmux") && is_modern_terminal(term)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumb_terminal_has_size_but_no_rendering_features() {
        assert_eq!(
            detect_capabilities(Some("dumb"), Some("truecolor")),
            TerminalCapabilities::none()
        );
    }

    #[test]
    fn color_detection_uses_the_highest_advertised_level() {
        assert_eq!(
            detect_capabilities(Some("xterm-256color"), None).color_level(),
            ColorLevel::Ansi256
        );
        assert_eq!(
            detect_capabilities(Some("xterm-256color"), Some("truecolor")).color_level(),
            ColorLevel::TrueColor
        );
        assert_eq!(
            detect_capabilities(None, None).color_level(),
            ColorLevel::None
        );
    }

    #[test]
    fn limited_terminals_do_not_advertise_extended_features() {
        let linux = detect_capabilities(Some("linux"), None);

        assert_eq!(linux.color_level(), ColorLevel::Ansi16);
        assert_eq!(
            linux.attributes(),
            TextAttribute::Bold
                | TextAttribute::Dim
                | TextAttribute::SlowBlink
                | TextAttribute::Reversed
        );
        assert_eq!(linux.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!linux.underline_colors());
        assert!(!linux.hyperlinks());

        let vt100 = detect_capabilities(Some("vt100"), None);
        assert_eq!(vt100.color_level(), ColorLevel::None);
        assert_eq!(vt100.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!vt100.underline_colors());
        assert!(!vt100.hyperlinks());
    }

    #[test]
    fn advanced_features_require_a_recognized_terminal() {
        let xterm = detect_capabilities(Some("xterm-256color"), None);
        assert_eq!(
            xterm.attributes(),
            TextAttribute::Bold
                | TextAttribute::Dim
                | TextAttribute::Italic
                | TextAttribute::SlowBlink
                | TextAttribute::Reversed
                | TextAttribute::Hidden
        );
        assert_eq!(xterm.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!xterm.underline_colors());
        assert!(!xterm.hyperlinks());

        let kitty = detect_capabilities(Some("xterm-kitty"), None);
        assert_eq!(kitty.color_level(), ColorLevel::TrueColor);
        assert_eq!(kitty.underline_styles(), UnderlineStyles::all());
        assert!(kitty.underline_colors());
        assert!(kitty.hyperlinks());

        let tmux = detect_capabilities(Some("tmux-256color"), None);
        assert_eq!(tmux.attributes(), xterm.attributes());
        assert_eq!(tmux.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!tmux.underline_colors());
        assert!(!tmux.hyperlinks());

        let screen = detect_capabilities(Some("screen.xterm-kitty-256color"), None);
        assert_eq!(screen.color_level(), ColorLevel::Ansi256);
        assert_eq!(screen.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!screen.underline_colors());
        assert!(!screen.hyperlinks());
    }

    #[test]
    fn text_attributes_follow_conservative_terminal_families() {
        assert_eq!(
            detect_capabilities(Some("ansi"), None).attributes(),
            TextAttribute::Bold
                | TextAttribute::SlowBlink
                | TextAttribute::Reversed
                | TextAttribute::Hidden
        );
        assert_eq!(
            detect_capabilities(Some("cygwin"), None).attributes(),
            TextAttribute::Bold | TextAttribute::Reversed | TextAttribute::Hidden
        );
        assert_eq!(
            detect_capabilities(Some("rxvt"), None).attributes(),
            TextAttribute::Bold | TextAttribute::SlowBlink | TextAttribute::Reversed
        );
        let screen = TextAttribute::Bold | TextAttribute::SlowBlink | TextAttribute::Reversed;
        assert_eq!(
            detect_capabilities(Some("screen.xterm-256color"), None).attributes(),
            screen
        );
        assert_eq!(
            detect_capabilities(Some("screen.konsole-256color"), None).attributes(),
            screen
        );
    }

    #[test]
    fn non_terminal_output_is_not_a_terminal_with_missing_capabilities() {
        let file = std::fs::File::open("Cargo.toml").unwrap();
        assert_eq!(detect(&file).unwrap(), TerminalDetection::NonTerminal);
    }
}
