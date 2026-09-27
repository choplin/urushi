use std::fmt::{self, Write as _};

use urushi_terminal::Color;

pub(crate) fn write_sgr_params(output: &mut String, color: Color, background: bool) -> fmt::Result {
    match color {
        Color::Ansi(index) if index < 8 => write!(
            output,
            "{}",
            u16::from(index) + if background { 40 } else { 30 }
        ),
        Color::Ansi(index) if index < 16 => write!(
            output,
            "{}",
            u16::from(index - 8) + if background { 100 } else { 90 }
        ),
        Color::Ansi(index) | Color::Ansi256(index) => {
            write!(output, "{};5;{index}", if background { 48 } else { 38 })
        }
        Color::Rgb(red, green, blue) => write!(
            output,
            "{};2;{red};{green};{blue}",
            if background { 48 } else { 38 }
        ),
    }
}

pub(crate) fn write_sgr_underline_params(output: &mut String, color: Color) -> fmt::Result {
    match color {
        Color::Ansi(index) | Color::Ansi256(index) => write!(output, "58;5;{index}"),
        Color::Rgb(red, green, blue) => write!(output, "58;2;{red};{green};{blue}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_each_ansi_color_form() {
        fn encoded(color: Color, background: bool) -> String {
            let mut output = String::new();
            write_sgr_params(&mut output, color, background).expect("String writes cannot fail");
            output
        }

        assert_eq!(encoded(Color::RED, false), "31");
        assert_eq!(encoded(Color::BRIGHT_RED, false), "91");
        assert_eq!(encoded(Color::RED, true), "41");
        assert_eq!(encoded(Color::Ansi256(212), false), "38;5;212");
        assert_eq!(encoded(Color::Rgb(1, 2, 3), true), "48;2;1;2;3");

        let mut underline = String::new();
        write_sgr_underline_params(&mut underline, Color::Ansi(1))
            .expect("String writes cannot fail");
        assert_eq!(underline, "58;5;1");
    }
}
