//! Cell-aware word and hard wrapping.

use unicode_width::UnicodeWidthChar;

use super::visible_width;

/// Greedily wraps text to a terminal-cell width.
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in text.lines() {
        if visible_width(line) <= width {
            out.push(line.to_string());
            continue;
        }
        let mut current = String::new();
        let mut current_width = 0;
        for word in line.split(' ') {
            let word_width = visible_width(word);
            if current_width > 0 {
                if current_width + 1 + word_width <= width {
                    current.push(' ');
                    current.push_str(word);
                    current_width += 1 + word_width;
                    continue;
                }
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width <= width {
                current.push_str(word);
                current_width = word_width;
            } else {
                hard_break(word, width, &mut current, &mut current_width, &mut out);
            }
        }
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn hard_break(
    word: &str,
    width: usize,
    current: &mut String,
    current_width: &mut usize,
    out: &mut Vec<String>,
) {
    for character in word.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if *current_width + character_width > width && *current_width > 0 {
            out.push(std::mem::take(current));
            *current_width = 0;
        }
        current.push(character);
        *current_width += character_width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words() {
        assert_eq!(
            wrap_text("the quick brown fox", 10),
            ["the quick", "brown fox"]
        );
        assert_eq!(wrap_text("short", 10), ["short"]);
    }

    #[test]
    fn breaks_cjk_runs() {
        assert_eq!(wrap_text("こんにちは", 4), ["こん", "にち", "は"]);
    }

    #[test]
    fn preserves_existing_newlines() {
        assert_eq!(wrap_text("a\nb", 10), ["a", "b"]);
    }
}
