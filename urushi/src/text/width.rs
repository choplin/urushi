//! The one definition of display width.
//!
//! Both domains ask this module how many cells a run of printable text
//! occupies: the plain path through [`PrintableText`](super::PrintableText),
//! and the rendered path once [`view::ansi`](crate::view) has stripped the
//! escape sequences. Neither knows the cell rules; they only know which text
//! to hand over.
//!
//! Anything that would make the measure depend on the terminal rather than on
//! the text — ambiguous East Asian width resolved as one cell or two, a
//! ligature shaped into fewer cells than its clusters occupy — becomes a
//! parameter of these two functions, and of nothing else.

use unicode_width::UnicodeWidthStr;

use super::{Grapheme, PrintableText};

/// The cells one grapheme cluster occupies.
///
/// CJK ideographs take two, an emoji ZWJ sequence takes two, and a combining
/// mark takes none.
pub(crate) fn grapheme(cluster: &Grapheme) -> usize {
    UnicodeWidthStr::width(cluster.as_str())
}

/// The cells a run of printable text occupies.
///
/// The sum over grapheme clusters, not `unicode-width`'s measure of the whole
/// string. The two differ where a ligature spans a cluster boundary, and the
/// sum is the one the layout pass needs: a resolved row is a sequence of
/// per-grapheme tokens whose widths add to the rectangle's width, and a box
/// fixes its width before its content is wrapped into it.
///
/// Printable ASCII is one cell per byte, so a run that is entirely ASCII is
/// its own length and needs no segmentation. That is exact rather than an
/// approximation: every ASCII character is either printable and one cell wide,
/// or a control character, which `unicode-width` also counts as one and which
/// callers are contracted not to pass.
pub(crate) fn text(text: &PrintableText) -> usize {
    let text = text.as_str();
    if text.is_ascii() {
        return text.len();
    }
    PrintableText::new(text).graphemes().map(grapheme).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn width(line: &str) -> usize {
        text(PrintableText::new(line))
    }

    #[test]
    fn ascii_is_one_cell_per_byte() {
        assert_eq!(width("hello, world!"), 13);
        assert_eq!(width(""), 0);
        // The fast path and the general path agree.
        assert_eq!(
            width("abc"),
            PrintableText::new("abc")
                .graphemes()
                .map(grapheme)
                .sum::<usize>()
        );
    }

    #[test]
    fn wide_and_zero_width_clusters_keep_their_cells() {
        assert_eq!(width("日本語"), 6);
        assert_eq!(width("aあ"), 3);
        assert_eq!(width("👩‍💻"), 2);
        assert_eq!(width("e\u{301}"), 1);
    }

    #[test]
    fn a_ligature_spanning_clusters_is_measured_by_its_clusters() {
        // LAM + ALEF render as one cell, but the layout pass carries them as
        // two tokens, so the model measures two.
        assert_eq!(UnicodeWidthStr::width("\u{644}\u{627}"), 1);
        assert_eq!(width("\u{644}\u{627}"), 2);
    }
}
