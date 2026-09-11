//! Border character sets for boxed content.

/// The characters used to draw a box border.
///
/// Presets cover the common box-drawing styles; custom borders can be built
/// with struct literal syntax by supplying the four edges and four corners.
/// All characters are assumed to be one terminal cell wide.
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

#[cfg(test)]
mod tests {
    use super::Border;

    #[test]
    fn presets_define_box_edges_and_corners() {
        let cases = [
            (Border::NORMAL, ['─', '─', '│', '│', '┌', '┘']),
            (Border::ROUNDED, ['─', '─', '│', '│', '╭', '╯']),
            (Border::THICK, ['━', '━', '┃', '┃', '┏', '┛']),
            (Border::DOUBLE, ['═', '═', '║', '║', '╔', '╝']),
            (Border::ASCII, ['-', '-', '|', '|', '+', '+']),
            (Border::HIDDEN, [' ', ' ', ' ', ' ', ' ', ' ']),
        ];

        for (border, expected) in cases {
            assert_eq!(
                [
                    border.top,
                    border.bottom,
                    border.left,
                    border.right,
                    border.top_left,
                    border.bottom_right,
                ],
                expected
            );
        }
    }

    #[test]
    fn custom_border_can_define_every_box_glyph() {
        let border = Border {
            top: 't',
            bottom: 'b',
            left: 'l',
            right: 'r',
            top_left: '1',
            top_right: '2',
            bottom_left: '3',
            bottom_right: '4',
        };

        assert_eq!(
            [
                border.top,
                border.bottom,
                border.left,
                border.right,
                border.top_left,
                border.top_right,
                border.bottom_left,
                border.bottom_right,
            ],
            ['t', 'b', 'l', 'r', '1', '2', '3', '4']
        );
    }
}
