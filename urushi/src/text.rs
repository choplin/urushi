//! ANSI-aware text measurement and wrapping.

use unicode_width::UnicodeWidthChar;

/// Returns the number of terminal cells `s` occupies, ignoring ANSI escape
/// sequences (CSI and OSC).
///
/// East Asian wide characters count as two cells:
///
/// ```
/// use urushi::visible_width;
///
/// assert_eq!(visible_width("hello"), 5);
/// assert_eq!(visible_width("日本語"), 6);
/// assert_eq!(visible_width("\x1b[31mred\x1b[0m"), 3);
/// ```
pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.peek() {
                // CSI: ESC [ <params> <final byte in 0x40-0x7e>
                Some('[') => {
                    chars.next();
                    for c2 in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c2) {
                            break;
                        }
                    }
                }
                // OSC: ESC ] ... terminated by BEL or ST (ESC \)
                Some(']') => {
                    chars.next();
                    while let Some(c2) = chars.next() {
                        if c2 == '\u{07}' {
                            break;
                        }
                        if c2 == '\u{1b}' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            width += UnicodeWidthChar::width(c).unwrap_or(0);
        }
    }
    width
}

/// Greedily word-wraps `text` so no line exceeds `width` cells. Words longer
/// than `width` (including unspaced CJK runs) are broken at character
/// boundaries.
///
/// Wrapping does not account for ANSI sequences spanning line breaks; styled
/// input should be wrapped before styling is applied.
pub(crate) fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in text.lines() {
        if visible_width(line) <= width {
            out.push(line.to_string());
            continue;
        }
        let mut cur = String::new();
        let mut cur_w = 0;
        for word in line.split(' ') {
            let word_w = visible_width(word);
            if cur_w > 0 {
                if cur_w + 1 + word_w <= width {
                    cur.push(' ');
                    cur.push_str(word);
                    cur_w += 1 + word_w;
                    continue;
                }
                out.push(std::mem::take(&mut cur));
                cur_w = 0;
            }
            if word_w <= width {
                cur.push_str(word);
                cur_w = word_w;
            } else {
                hard_break(word, width, &mut cur, &mut cur_w, &mut out);
            }
        }
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Splits a single over-long word at character boundaries, appending full
/// rows to `out` and leaving the remainder in `cur`.
fn hard_break(
    word: &str,
    width: usize,
    cur: &mut String,
    cur_w: &mut usize,
    out: &mut Vec<String>,
) {
    for c in word.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if *cur_w + cw > width && *cur_w > 0 {
            out.push(std::mem::take(cur));
            *cur_w = 0;
        }
        cur.push(c);
        *cur_w += cw;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_ignores_ansi() {
        assert_eq!(visible_width("\x1b[1;38;5;212mabc\x1b[0m"), 3);
        assert_eq!(
            visible_width("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"),
            4
        );
    }

    #[test]
    fn width_counts_wide_chars() {
        assert_eq!(visible_width("日本語"), 6);
        assert_eq!(visible_width("aあ"), 3);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn wrap_by_words() {
        assert_eq!(wrap("the quick brown fox", 10), ["the quick", "brown fox"]);
        assert_eq!(wrap("short", 10), ["short"]);
    }

    #[test]
    fn wrap_breaks_cjk_runs() {
        assert_eq!(wrap("こんにちは", 4), ["こん", "にち", "は"]);
    }

    #[test]
    fn wrap_preserves_existing_newlines() {
        assert_eq!(wrap("a\nb", 10), ["a", "b"]);
    }
}
