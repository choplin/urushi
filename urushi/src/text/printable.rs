//! Plain text: strings that carry no terminal control information.

use unicode_segmentation::UnicodeSegmentation;

use super::width;

/// Several lines of plain text, separated by `\n`.
///
/// A `&str` cannot say whether it holds plain text or already-rendered ANSI,
/// and a function that guesses has to guess on every call. That guessing is
/// what let escape handling leak into the plain path, so the property is
/// carried by this type instead: every line it holds is a
/// [`PrintableText`], and the `\n` between them is a separator rather than
/// content.
///
/// It has no width. Width is a property of a row of cells, so it belongs to
/// the lines this type splits into. Measuring across a line break would sum
/// cells that never share a row.
///
/// Construction is where the caller declares the domain, mirroring
/// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) on the other
/// side of the boundary. Debug builds check the declaration; release builds
/// take the caller's word for it.
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PrintableLines(str);

impl PrintableLines {
    /// Adopts plain text.
    ///
    /// # Panics
    ///
    /// In debug builds, panics when `text` holds a control character other
    /// than `\n` — an escape sequence, a carriage return, a tab, a backspace.
    /// Handing rendered output to a plain-text argument is a contract
    /// violation, not a supported call with a degraded result: the layout pass
    /// would measure the escapes as ordinary characters, and wrapping or
    /// truncation would split them. Release builds do not check.
    pub fn new(text: &str) -> &Self {
        debug_assert!(
            !text
                .chars()
                .any(|character| character.is_control() && character != '\n'),
            "plain text must not carry terminal control characters: {text:?}"
        );
        Self::adopt(text)
    }

    /// Wraps a string already known to satisfy the invariant.
    fn adopt(text: &str) -> &Self {
        // SAFETY: `PrintableLines` is `repr(transparent)` over `str`, so the
        // two have the same layout and the cast only changes the type.
        unsafe { &*(std::ptr::from_ref::<str>(text) as *const Self) }
    }

    /// Splits into lines, yielding one empty line for empty text.
    ///
    /// A view's text always occupies at least one row, so an empty string
    /// measures as one empty line rather than as no line at all.
    pub fn lines(&self) -> Vec<&PrintableText> {
        // The lines of a value that already satisfies this type's invariant
        // satisfy the line invariant, so re-checking each one would turn one
        // boundary check into a scan per line.
        let lines: Vec<&PrintableText> = self.0.lines().map(PrintableText::adopt).collect();
        if lines.is_empty() {
            vec![PrintableText::adopt("")]
        } else {
            lines
        }
    }
}

/// One line of plain text: a row of cells, every character printable.
///
/// This is the unit measurement is defined on — it holds no control
/// characters at all, not even `\n`. [`PrintableLines`] spans rows and
/// therefore has no width; a `PrintableText` is exactly one row, so its cells
/// come to a single number.
#[derive(Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PrintableText(str);

impl PrintableText {
    /// Adopts one line of plain text.
    ///
    /// # Panics
    ///
    /// In debug builds, panics when `text` holds any control character, `\n`
    /// included — a line is one row of cells, so a line break inside it is as
    /// much a contract violation as an escape sequence. Release builds do not
    /// check.
    pub fn new(text: &str) -> &Self {
        debug_assert!(
            !text.chars().any(char::is_control),
            "a plain-text line must not carry control characters: {text:?}"
        );
        Self::adopt(text)
    }

    /// Wraps a string already known to satisfy the invariant.
    fn adopt(text: &str) -> &Self {
        // SAFETY: `PrintableText` is `repr(transparent)` over `str`, so the
        // two have the same layout and the cast only changes the type.
        unsafe { &*(std::ptr::from_ref::<str>(text) as *const Self) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the terminal cells this line occupies.
    ///
    /// This is the crate's one definition of display width: CJK ideographs
    /// take two cells, an emoji ZWJ sequence takes two, and a combining mark
    /// takes none. Because the receiver is a single line and cannot hold
    /// escapes or cursor movement, nothing has to be scanned for or skipped.
    ///
    /// The cell rules are defined in one place inside the crate, which is
    /// where a measure that depended on the terminal — ambiguous East Asian
    /// width resolved as one cell or two — would take its input.
    pub fn width(&self) -> usize {
        width::text(self)
    }

    /// Cuts to at most `width` cells, on a grapheme boundary.
    ///
    /// A grapheme that would straddle the limit is dropped rather than split,
    /// so the result never exceeds `width` and never leaves half a wide
    /// character behind.
    pub fn truncate(&self, width: usize) -> &Self {
        if self.width() <= width {
            return self;
        }
        let mut end = 0;
        let mut consumed = 0;
        for (offset, grapheme) in self.0.grapheme_indices(true) {
            let grapheme_width = width::grapheme(Self::adopt(grapheme));
            if consumed + grapheme_width > width {
                break;
            }
            consumed += grapheme_width;
            end = offset + grapheme.len();
        }
        Self::adopt(&self.0[..end])
    }

    /// Splits into grapheme clusters, the unit a cell boundary may fall on.
    pub fn graphemes(&self) -> impl Iterator<Item = &Self> {
        self.0.graphemes(true).map(Self::adopt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_wide_characters() {
        assert_eq!(PrintableText::new("日本語").width(), 6);
        assert_eq!(PrintableText::new("aあ").width(), 3);
        assert_eq!(PrintableText::new("👩‍💻").width(), 2);
        assert_eq!(PrintableText::new("e\u{301}").width(), 1);
        assert_eq!(PrintableText::new("").width(), 0);
    }

    #[test]
    fn empty_text_is_one_empty_line() {
        assert_eq!(
            PrintableLines::new("").lines(),
            vec![PrintableText::new("")]
        );
        assert_eq!(
            PrintableLines::new("a\nb").lines(),
            vec![PrintableText::new("a"), PrintableText::new("b")]
        );
    }

    #[test]
    fn graphemes_keep_clusters_whole() {
        let clusters: Vec<&str> = PrintableText::new("👩‍💻x")
            .graphemes()
            .map(PrintableText::as_str)
            .collect();
        assert_eq!(clusters, ["👩‍💻", "x"]);
    }

    #[test]
    #[should_panic(expected = "terminal control characters")]
    fn rejects_rendered_output_handed_to_the_plain_domain() {
        let _ = PrintableLines::new("\x1b[31mred\x1b[0m");
    }

    #[test]
    #[should_panic(expected = "terminal control characters")]
    fn rejects_cursor_movement_in_plain_text() {
        let _ = PrintableLines::new("a\tb");
    }

    #[test]
    #[should_panic(expected = "must not carry control characters")]
    fn rejects_a_line_break_inside_a_line() {
        let _ = PrintableText::new("a\nb");
    }

    #[test]
    fn truncation_drops_a_grapheme_that_would_straddle_the_limit() {
        assert_eq!(PrintableText::new("A日本").truncate(3).as_str(), "A日");
        assert_eq!(PrintableText::new("日本").truncate(1).as_str(), "");
        assert_eq!(
            PrintableText::new("e\u{301}x").truncate(1).as_str(),
            "e\u{301}"
        );
    }
}
