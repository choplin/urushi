//! Cell-aware word and hard wrapping.

use super::{
    visible_width,
    width::{ansi_graphemes, ansi_sequence_end},
};

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
        for word in split_ansi_words(line) {
            let word_width = visible_width(&word);
            if current_width > 0 {
                if current_width + 1 + word_width <= width {
                    current.push(' ');
                    current.push_str(&word);
                    current_width += 1 + word_width;
                    continue;
                }
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width <= width {
                current.push_str(&word);
                current_width = word_width;
            } else {
                hard_break(&word, width, &mut current, &mut current_width, &mut out);
            }
        }
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

fn split_ansi_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut offset = 0;

    while offset < line.len() {
        if line.as_bytes()[offset] == 0x1b {
            let end = ansi_sequence_end(line, offset);
            word.push_str(&line[offset..end]);
            offset = end;
            continue;
        }

        let character = line[offset..]
            .chars()
            .next()
            .expect("offset remains on a character boundary");
        offset += character.len_utf8();
        if character == ' ' {
            words.push(std::mem::take(&mut word));
        } else {
            word.push(character);
        }
    }
    words.push(word);
    words
}

fn hard_break(
    word: &str,
    width: usize,
    current: &mut String,
    current_width: &mut usize,
    out: &mut Vec<String>,
) {
    let (graphemes, trailing_controls) = ansi_graphemes(word);
    for grapheme in graphemes {
        if *current_width + grapheme.width > width && *current_width > 0 {
            out.push(std::mem::take(current));
            *current_width = 0;
        }
        current.push_str(&grapheme.rendered);
        *current_width += grapheme.width;
    }
    current.push_str(&trailing_controls);
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

    #[test]
    fn hard_wrap_preserves_ansi_sequences_and_grapheme_clusters() {
        assert_eq!(
            wrap_text("\x1b[31mabcdef\x1b[0m", 4),
            ["\x1b[31mabcd", "ef\x1b[0m"]
        );
        assert_eq!(wrap_text("👩‍💻x", 2), ["👩‍💻", "x"]);
        assert_eq!(
            wrap_text("👩\x1b[31m\u{200d}💻x", 2),
            ["👩\x1b[31m\u{200d}💻", "x"]
        );
        assert_eq!(
            wrap_text("\x1b]8;;https://exa mple.com\x1b\\link\x1b]8;;\x1b\\", 2,),
            ["\x1b]8;;https://exa mple.com\x1b\\li", "nk\x1b]8;;\x1b\\"]
        );
    }
}
