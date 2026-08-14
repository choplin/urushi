//! Border character sets for boxed content.

/// The characters used to draw a box border.
///
/// Presets cover the common box-drawing styles; custom borders can be built
/// with struct literal syntax by supplying outer, separator, and junction
/// glyphs. All characters are assumed to be one terminal cell wide.
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
    /// The intersection of a horizontal separator and the left edge.
    pub middle_left: char,
    /// The intersection of a horizontal separator and the right edge.
    pub middle_right: char,
    /// The intersection of horizontal and vertical separators.
    pub middle: char,
    /// The horizontal separator between table rows.
    pub middle_horizontal: char,
    /// The intersection of a vertical separator and the top edge.
    pub middle_top: char,
    /// The intersection of a vertical separator and the bottom edge.
    pub middle_bottom: char,
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
        middle_left: '├',
        middle_right: '┤',
        middle: '┼',
        middle_horizontal: '─',
        middle_top: '┬',
        middle_bottom: '┴',
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
        middle_left: '┣',
        middle_right: '┫',
        middle: '╋',
        middle_horizontal: '━',
        middle_top: '┳',
        middle_bottom: '┻',
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
        middle_left: '╠',
        middle_right: '╣',
        middle: '╬',
        middle_horizontal: '═',
        middle_top: '╦',
        middle_bottom: '╩',
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
        middle_left: '+',
        middle_right: '+',
        middle: '+',
        middle_horizontal: '-',
        middle_top: '+',
        middle_bottom: '+',
    };

    /// Markdown table border: `|---|`
    pub const MARKDOWN: Self = Self {
        top: '-',
        bottom: '-',
        left: '|',
        right: '|',
        top_left: '|',
        top_right: '|',
        bottom_left: '|',
        bottom_right: '|',
        middle_left: '|',
        middle_right: '|',
        middle: '|',
        middle_horizontal: '-',
        middle_top: '|',
        middle_bottom: '|',
    };

    /// Booktabs-style table border with heavy outer rules, a light header rule,
    /// and no vertical rules.
    pub const BOOKTABS: Self = Self {
        top: '━',
        bottom: '━',
        left: ' ',
        right: ' ',
        top_left: '━',
        top_right: '━',
        bottom_left: '━',
        bottom_right: '━',
        middle_left: '─',
        middle_right: '─',
        middle: '─',
        middle_horizontal: '─',
        middle_top: '━',
        middle_bottom: '━',
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
        middle_left: ' ',
        middle_right: ' ',
        middle: ' ',
        middle_horizontal: ' ',
        middle_top: ' ',
        middle_bottom: ' ',
    };
}

#[cfg(test)]
mod tests {
    use super::Border;

    #[test]
    fn presets_define_table_separators_and_junctions() {
        let cases = [
            (Border::NORMAL, ['─', '├', '┤', '┼', '┬', '┴']),
            (Border::ROUNDED, ['─', '├', '┤', '┼', '┬', '┴']),
            (Border::THICK, ['━', '┣', '┫', '╋', '┳', '┻']),
            (Border::DOUBLE, ['═', '╠', '╣', '╬', '╦', '╩']),
            (Border::ASCII, ['-', '+', '+', '+', '+', '+']),
            (Border::MARKDOWN, ['-', '|', '|', '|', '|', '|']),
            (Border::BOOKTABS, ['─', '─', '─', '─', '━', '━']),
            (Border::HIDDEN, [' ', ' ', ' ', ' ', ' ', ' ']),
        ];

        for (border, expected) in cases {
            assert_eq!(
                [
                    border.middle_horizontal,
                    border.middle_left,
                    border.middle_right,
                    border.middle,
                    border.middle_top,
                    border.middle_bottom,
                ],
                expected
            );
        }
    }

    #[test]
    fn booktabs_uses_heavy_outer_rules_and_a_light_middle_rule() {
        assert_eq!(Border::BOOKTABS.top, '━');
        assert_eq!(Border::BOOKTABS.bottom, '━');
        assert_eq!(Border::BOOKTABS.middle_horizontal, '─');
        assert_eq!(Border::BOOKTABS.left, ' ');
        assert_eq!(Border::BOOKTABS.right, ' ');
    }

    #[test]
    fn custom_border_can_define_every_glyph() {
        let border = Border {
            top: 't',
            bottom: 'b',
            left: 'l',
            right: 'r',
            top_left: '1',
            top_right: '2',
            bottom_left: '3',
            bottom_right: '4',
            middle_left: '5',
            middle_right: '6',
            middle: '7',
            middle_horizontal: '8',
            middle_top: '9',
            middle_bottom: '0',
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
                border.middle_left,
                border.middle_right,
                border.middle,
                border.middle_horizontal,
                border.middle_top,
                border.middle_bottom,
            ],
            [
                't', 'b', 'l', 'r', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0'
            ]
        );
    }
}
