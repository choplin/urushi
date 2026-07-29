//! Border character sets for boxed content.

/// The characters used to draw a box border.
///
/// Presets cover the common box-drawing styles; custom borders can be built
/// with struct literal syntax. All characters are assumed to be one terminal
/// cell wide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Border {
    pub top: char,
    pub bottom: char,
    pub left: char,
    pub right: char,
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
}

impl Border {
    /// Single-line border: `┌─┐`
    pub const NORMAL: Self = Self {
        top: '─',
        bottom: '─',
        left: '│',
        right: '│',
        top_left: '┌',
        top_right: '┐',
        bottom_left: '└',
        bottom_right: '┘',
    };

    /// Rounded-corner border: `╭─╮`
    pub const ROUNDED: Self = Self {
        top_left: '╭',
        top_right: '╮',
        bottom_left: '╰',
        bottom_right: '╯',
        ..Self::NORMAL
    };

    /// Heavy-line border: `┏━┓`
    pub const THICK: Self = Self {
        top: '━',
        bottom: '━',
        left: '┃',
        right: '┃',
        top_left: '┏',
        top_right: '┓',
        bottom_left: '┗',
        bottom_right: '┛',
    };

    /// Double-line border: `╔═╗`
    pub const DOUBLE: Self = Self {
        top: '═',
        bottom: '═',
        left: '║',
        right: '║',
        top_left: '╔',
        top_right: '╗',
        bottom_left: '╚',
        bottom_right: '╝',
    };

    /// ASCII-only border for terminals without box-drawing glyphs: `+-+`
    pub const ASCII: Self = Self {
        top: '-',
        bottom: '-',
        left: '|',
        right: '|',
        top_left: '+',
        top_right: '+',
        bottom_left: '+',
        bottom_right: '+',
    };

    /// Invisible border that still occupies one cell on each side.
    pub const HIDDEN: Self = Self {
        top: ' ',
        bottom: ' ',
        left: ' ',
        right: ' ',
        top_left: ' ',
        top_right: ' ',
        bottom_left: ' ',
        bottom_right: ' ',
    };
}
