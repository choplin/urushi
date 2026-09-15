//! Backend-independent terminal contracts and target-specific inspection for
//! the Urushi ecosystem.
//!
//! This crate owns vocabulary shared by terminal surfaces without depending on
//! another Urushi workspace crate.
//! It does not choose rendering policy, perform layout, or render a view.

use std::io::{self, IsTerminal};

mod terminal;

pub use terminal::{Frame, Position, Rect, Terminal};

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

/// A set of independently selectable SGR text attributes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TextAttributes(u16);

impl TextAttributes {
    pub const BOLD: Self = Self(1 << 0);
    pub const DIM: Self = Self(1 << 1);
    pub const ITALIC: Self = Self(1 << 2);
    pub const SLOW_BLINK: Self = Self(1 << 3);
    pub const REVERSED: Self = Self(1 << 4);
    pub const HIDDEN: Self = Self(1 << 5);
    pub const CROSSED_OUT: Self = Self(1 << 6);

    const ALL_BITS: u16 = Self::BOLD.0
        | Self::DIM.0
        | Self::ITALIC.0
        | Self::SLOW_BLINK.0
        | Self::REVERSED.0
        | Self::HIDDEN.0
        | Self::CROSSED_OUT.0;

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(Self::ALL_BITS)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for TextAttributes {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
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

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
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
    colors: ColorLevel,
    attributes: TextAttributes,
    underline_styles: UnderlineStyles,
    underline_colors: bool,
    hyperlinks: bool,
}

impl TerminalCapabilities {
    /// Returns capabilities with every output feature disabled.
    const fn none() -> Self {
        Self {
            colors: ColorLevel::None,
            attributes: TextAttributes::empty(),
            underline_styles: UnderlineStyles::empty(),
            underline_colors: false,
            hyperlinks: false,
        }
    }

    pub const fn colors(self) -> ColorLevel {
        self.colors
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

    const fn with_colors(mut self, colors: ColorLevel) -> Self {
        self.colors = colors;
        self
    }

    const fn with_attributes(mut self, attributes: TextAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    const fn with_underline_styles(mut self, styles: UnderlineStyles) -> Self {
        self.underline_styles = styles;
        self
    }

    const fn with_underline_colors(mut self, enabled: bool) -> Self {
        self.underline_colors = enabled;
        self
    }

    const fn with_hyperlinks(mut self, enabled: bool) -> Self {
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
    const fn new(size: TerminalSize, capabilities: TerminalCapabilities) -> Self {
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
    let colors = if is_modern_terminal(term)
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
        .with_colors(colors)
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
        TextAttributes::BOLD | TextAttributes::SLOW_BLINK | TextAttributes::REVERSED;
    let xterm_attributes =
        bold_blink_reverse | TextAttributes::DIM | TextAttributes::ITALIC | TextAttributes::HIDDEN;
    if is_modern_terminal(term) {
        TextAttributes::all()
    } else if term == "linux" {
        bold_blink_reverse | TextAttributes::DIM
    } else if term.starts_with("screen") {
        bold_blink_reverse
    } else if term.starts_with("tmux") || term.contains("xterm") {
        xterm_attributes
    } else if term.contains("konsole") {
        bold_blink_reverse | TextAttributes::ITALIC
    } else if term.contains("ansi") {
        bold_blink_reverse | TextAttributes::HIDDEN
    } else if term.contains("cygwin") {
        TextAttributes::BOLD | TextAttributes::REVERSED | TextAttributes::HIDDEN
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
            detect_capabilities(Some("xterm-256color"), None).colors(),
            ColorLevel::Ansi256
        );
        assert_eq!(
            detect_capabilities(Some("xterm-256color"), Some("truecolor")).colors(),
            ColorLevel::TrueColor
        );
        assert_eq!(detect_capabilities(None, None).colors(), ColorLevel::None);
    }

    #[test]
    fn limited_terminals_do_not_advertise_extended_features() {
        let linux = detect_capabilities(Some("linux"), None);

        assert_eq!(linux.colors(), ColorLevel::Ansi16);
        assert_eq!(
            linux.attributes(),
            TextAttributes::BOLD
                | TextAttributes::DIM
                | TextAttributes::SLOW_BLINK
                | TextAttributes::REVERSED
        );
        assert_eq!(linux.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!linux.underline_colors());
        assert!(!linux.hyperlinks());

        let vt100 = detect_capabilities(Some("vt100"), None);
        assert_eq!(vt100.colors(), ColorLevel::None);
        assert_eq!(vt100.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!vt100.underline_colors());
        assert!(!vt100.hyperlinks());
    }

    #[test]
    fn advanced_features_require_a_recognized_terminal() {
        let xterm = detect_capabilities(Some("xterm-256color"), None);
        assert_eq!(
            xterm.attributes(),
            TextAttributes::BOLD
                | TextAttributes::DIM
                | TextAttributes::ITALIC
                | TextAttributes::SLOW_BLINK
                | TextAttributes::REVERSED
                | TextAttributes::HIDDEN
        );
        assert_eq!(xterm.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!xterm.underline_colors());
        assert!(!xterm.hyperlinks());

        let kitty = detect_capabilities(Some("xterm-kitty"), None);
        assert_eq!(kitty.colors(), ColorLevel::TrueColor);
        assert_eq!(kitty.underline_styles(), UnderlineStyles::all());
        assert!(kitty.underline_colors());
        assert!(kitty.hyperlinks());

        let tmux = detect_capabilities(Some("tmux-256color"), None);
        assert_eq!(tmux.attributes(), xterm.attributes());
        assert_eq!(tmux.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!tmux.underline_colors());
        assert!(!tmux.hyperlinks());

        let screen = detect_capabilities(Some("screen.xterm-kitty-256color"), None);
        assert_eq!(screen.colors(), ColorLevel::Ansi256);
        assert_eq!(screen.underline_styles(), UnderlineStyles::SINGLE);
        assert!(!screen.underline_colors());
        assert!(!screen.hyperlinks());
    }

    #[test]
    fn text_attributes_follow_conservative_terminal_families() {
        assert_eq!(
            detect_capabilities(Some("ansi"), None).attributes(),
            TextAttributes::BOLD
                | TextAttributes::SLOW_BLINK
                | TextAttributes::REVERSED
                | TextAttributes::HIDDEN
        );
        assert_eq!(
            detect_capabilities(Some("cygwin"), None).attributes(),
            TextAttributes::BOLD | TextAttributes::REVERSED | TextAttributes::HIDDEN
        );
        assert_eq!(
            detect_capabilities(Some("rxvt"), None).attributes(),
            TextAttributes::BOLD | TextAttributes::SLOW_BLINK | TextAttributes::REVERSED
        );
        let screen = TextAttributes::BOLD | TextAttributes::SLOW_BLINK | TextAttributes::REVERSED;
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
