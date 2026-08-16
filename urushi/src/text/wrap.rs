//! Cell-aware word and hard wrapping.

use super::{PrintableLines, PrintableText};

/// Greedily wraps plain text to a terminal-cell width.
///
/// Existing line breaks are kept. A word that does not fit the width alone is
/// broken between grapheme clusters, so a wide character or an emoji sequence
/// is never split down the middle.
pub(crate) fn wrap_text(text: &PrintableLines, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for line in text.lines() {
        if line.width() <= width {
            out.push(line.as_str().to_owned());
            continue;
        }
        let mut current = String::new();
        let mut current_width = 0;
        for word in line.as_str().split(' ') {
            let word = PrintableText::new(word);
            let word_width = word.width();
            if current_width > 0 {
                if current_width + 1 + word_width <= width {
                    current.push(' ');
                    current.push_str(word.as_str());
                    current_width += 1 + word_width;
                    continue;
                }
                out.push(std::mem::take(&mut current));
                current_width = 0;
            }
            if word_width <= width {
                current.push_str(word.as_str());
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
    word: &PrintableText,
    width: usize,
    current: &mut String,
    current_width: &mut usize,
    out: &mut Vec<String>,
) {
    for grapheme in word.graphemes() {
        let grapheme_width = grapheme.width();
        if *current_width + grapheme_width > width && *current_width > 0 {
            out.push(std::mem::take(current));
            *current_width = 0;
        }
        current.push_str(grapheme.as_str());
        *current_width += grapheme_width;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wrap(text: &str, width: usize) -> Vec<String> {
        wrap_text(PrintableLines::new(text), width)
    }

    #[test]
    fn wraps_words() {
        assert_eq!(wrap("the quick brown fox", 10), ["the quick", "brown fox"]);
        assert_eq!(wrap("short", 10), ["short"]);
    }

    #[test]
    fn breaks_cjk_runs() {
        assert_eq!(wrap("こんにちは", 4), ["こん", "にち", "は"]);
    }

    #[test]
    fn preserves_existing_newlines() {
        assert_eq!(wrap("a\nb", 10), ["a", "b"]);
    }

    #[test]
    fn hard_wrap_keeps_grapheme_clusters_whole() {
        assert_eq!(wrap("abcdef", 4), ["abcd", "ef"]);
        assert_eq!(wrap("👩‍💻x", 2), ["👩‍💻", "x"]);
    }
}
