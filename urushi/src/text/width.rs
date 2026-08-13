//! ANSI-aware terminal cell measurement.

use unicode_width::UnicodeWidthChar;

/// Returns the terminal cells occupied by `text`, ignoring CSI and OSC sequences.
pub fn visible_width(text: &str) -> usize {
    let mut width = 0;
    let mut chars = text.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            match chars.peek() {
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{07}' {
                            break;
                        }
                        if next == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            width += UnicodeWidthChar::width(character).unwrap_or(0);
        }
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_ansi() {
        assert_eq!(visible_width("\x1b[1;38;5;212mabc\x1b[0m"), 3);
        assert_eq!(
            visible_width("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"),
            4
        );
    }

    #[test]
    fn counts_wide_characters() {
        assert_eq!(visible_width("日本語"), 6);
        assert_eq!(visible_width("aあ"), 3);
        assert_eq!(visible_width(""), 0);
    }
}
