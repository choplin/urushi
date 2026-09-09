//! Cell-aware word and hard wrapping.

use super::{Grapheme, PrintableLines, PrintableText, StyledTextGrapheme};

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
        out.extend(
            wrap_line(line.graphemes().collect(), width)
                .into_iter()
                .map(|row| row.into_iter().map(Grapheme::as_str).collect()),
        );
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

/// Counts the rows [`wrap_text`] would produce without allocating them.
pub(crate) fn wrapped_line_count(text: &PrintableLines, width: usize) -> usize {
    let width = width.max(1);
    text.lines()
        .into_iter()
        .map(|line| {
            if line.width() <= width {
                return 1;
            }
            let mut rows = 0usize;
            let mut current_width = 0usize;
            for word in line.as_str().split(' ') {
                let word = PrintableText::new(word);
                let word_width = word.width();
                if current_width > 0 {
                    if current_width + 1 + word_width <= width {
                        current_width += 1 + word_width;
                        continue;
                    }
                    rows += 1;
                    current_width = 0;
                }
                if word_width <= width {
                    current_width = word_width;
                } else {
                    for grapheme in word.graphemes() {
                        let grapheme_width = grapheme.width();
                        if current_width + grapheme_width > width && current_width > 0 {
                            rows += 1;
                            current_width = 0;
                        }
                        current_width += grapheme_width;
                    }
                }
            }
            rows + 1
        })
        .sum()
}

/// Wraps styled graphemes without treating style boundaries as text
/// boundaries.
pub(crate) fn wrap_styled_lines<'a>(
    lines: Vec<Vec<StyledTextGrapheme<'a>>>,
    width: usize,
) -> Vec<Vec<StyledTextGrapheme<'a>>> {
    let width = width.max(1);
    lines
        .into_iter()
        .flat_map(|line| wrap_line(line, width))
        .collect()
}

trait WrapGrapheme {
    fn symbol(&self) -> &str;
    fn width(&self) -> usize;
}

impl WrapGrapheme for &Grapheme {
    fn symbol(&self) -> &str {
        self.as_str()
    }

    fn width(&self) -> usize {
        Grapheme::width(self)
    }
}

impl WrapGrapheme for StyledTextGrapheme<'_> {
    fn symbol(&self) -> &str {
        self.grapheme.as_str()
    }

    fn width(&self) -> usize {
        (*self).width()
    }
}

fn wrap_line<T: WrapGrapheme>(line: Vec<T>, width: usize) -> Vec<Vec<T>> {
    if line_width(&line) <= width {
        return vec![line];
    }

    let mut words: Vec<(Option<T>, Vec<T>)> = vec![(None, Vec::new())];
    for grapheme in line {
        if grapheme.symbol() == " " {
            words.push((Some(grapheme), Vec::new()));
        } else {
            words.last_mut().expect("one initial word").1.push(grapheme);
        }
    }

    let mut out = Vec::new();
    let mut current = Vec::new();
    let mut current_width = 0;
    for (separator, word) in words {
        let word_width = line_width(&word);
        if current_width > 0 {
            if current_width + 1 + word_width <= width {
                if let Some(separator) = separator {
                    current.push(separator);
                }
                current.extend(word);
                current_width += 1 + word_width;
                continue;
            }
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if word_width <= width {
            current = word;
            current_width = word_width;
        } else {
            for grapheme in word {
                let grapheme_width = grapheme.width();
                if current_width + grapheme_width > width && current_width > 0 {
                    out.push(std::mem::take(&mut current));
                    current_width = 0;
                }
                current.push(grapheme);
                current_width += grapheme_width;
            }
        }
    }
    out.push(current);
    out
}

fn line_width<T: WrapGrapheme>(line: &[T]) -> usize {
    line.iter().map(|grapheme| grapheme.width()).sum()
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

    #[test]
    fn counting_matches_materialized_wrapping() {
        for text in ["", "short", "the quick brown fox", "a  b", "日本語", "a\nb"] {
            for width in 0..8 {
                assert_eq!(
                    wrapped_line_count(PrintableLines::new(text), width),
                    wrap_text(PrintableLines::new(text), width).len(),
                    "{text:?} at width {width}"
                );
            }
        }
    }
}
