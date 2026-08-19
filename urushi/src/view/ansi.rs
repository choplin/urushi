//! Cell resolution for rendered output.
//!
//! This module is the crate's only ANSI-aware measurement. Reaching it means
//! going through [`RenderedBlock::from_ansi`](super::RenderedBlock::from_ansi),
//! which is where a caller declares a string to be rendered output; every other
//! path in the crate measures plain text, where escapes cannot occur.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;

use crate::text::{Grapheme, PrintableText, width};

/// The column interval a horizontal tab advances to.
pub(super) const TAB_WIDTH: usize = 8;

/// Splits rendered output into rows.
///
/// `\n` and `\r\n` end a row; the `\r` of a `\r\n` pair is not part of the row
/// it ends. A lone `\r` is cursor movement, not a row boundary, and is left for
/// [`resolve_row`] to interpret. A trailing newline leaves one empty row behind
/// it, and empty text has no rows at all.
pub(super) fn rows(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|row| row.strip_suffix('\r').unwrap_or(row))
        .collect()
}

/// Resolves one row to the cells a terminal would show, and its width.
///
/// A row of rendered output is a sequence of operations on a terminal, not
/// necessarily a sequence of cells: `\r`, `\t` and backspace move the cursor,
/// so later text can land on top of earlier text. Measuring before resolving
/// would count the operations rather than the result. Resolving happens once,
/// here, and everything downstream sees a flat row of cells.
///
/// Rows that carry no control character at all — everything this crate renders
/// itself — are returned borrowed and unchanged.
pub(super) fn resolve_row(row: &str) -> (Cow<'_, str>, usize) {
    if !needs_resolution(row) {
        return (Cow::Borrowed(row), printable_width(row));
    }

    let mut cells: Vec<Cell> = Vec::new();
    let mut column = 0usize;
    let mut scope = Scope::default();
    let mut carried = String::new();
    let mut written: Option<usize> = None;
    let mut offset = 0;

    while offset < row.len() {
        if row.as_bytes()[offset] == 0x1b {
            let end = ansi_sequence_end(row, offset);
            scope.observe(&row[offset..end]);
            offset = end;
            continue;
        }

        let character = row[offset..]
            .chars()
            .next()
            .expect("offset remains on a character boundary");
        match character {
            '\r' => {
                column = 0;
                offset += character.len_utf8();
                continue;
            }
            '\u{8}' => {
                column = column.saturating_sub(1);
                offset += character.len_utf8();
                continue;
            }
            '\t' => {
                // A terminal's tab moves the cursor; it does not erase what it
                // passes over. Columns it skips that were never written stay
                // empty, and an empty column renders as a space.
                column = (column / TAB_WIDTH + 1) * TAB_WIDTH;
                offset += character.len_utf8();
                continue;
            }
            _ => {}
        }

        let grapheme = row[offset..]
            .graphemes(true)
            .next()
            .expect("offset remains on a grapheme boundary");
        offset += grapheme.len();
        if grapheme.chars().all(char::is_control) {
            continue;
        }

        let width = width::grapheme(Grapheme::new(grapheme));
        if width == 0 {
            // A zero-width grapheme claims no column of its own: a lone
            // combining mark, a zero-width space, a byte-order mark. It joins
            // the cell it modifies, or waits for the next one when it has no
            // predecessor. Giving it a column would make the block claim a
            // width its own text does not fill, and a join would then place
            // the next block one column too far left.
            match written.and_then(|index| cells.get_mut(index)) {
                Some(Cell::Head { text, .. }) => text.push_str(grapheme),
                _ => carried.push_str(grapheme),
            }
            continue;
        }
        clear(&mut cells, column, width);
        grow(&mut cells, column + width);
        carried.push_str(grapheme);
        cells[column] = Cell::Head {
            scope: scope.clone(),
            text: std::mem::take(&mut carried),
            width,
        };
        for tail in cells.iter_mut().take(column + width).skip(column + 1) {
            *tail = Cell::Tail;
        }
        written = Some(column);
        column += width;
    }

    let width = cells
        .iter()
        .rposition(|cell| !matches!(cell, Cell::Empty))
        .map_or(0, |last| last + 1);

    // Each cell carries the scope that was open when it was written, and the
    // row re-emits only the transitions between them. Replaying the sequences
    // in stream order instead would move a reset that closed a scope before a
    // carriage return onto the cell that later overwrote its first column,
    // stripping the style from every cell that survived.
    let blank = Scope::default();
    let mut open = Scope::default();
    let mut resolved = String::with_capacity(row.len());
    for cell in &cells[..width] {
        let (text, scope) = match cell {
            Cell::Empty => (" ", &blank),
            Cell::Head { text, scope, .. } => (text.as_str(), scope),
            Cell::Tail => continue,
        };
        if *scope != open {
            resolved.push_str(&open.close());
            resolved.push_str(&scope.open());
            open = scope.clone();
        }
        resolved.push_str(text);
    }
    resolved.push_str(&open.close());
    resolved.push_str(&carried);
    (Cow::Owned(resolved), width)
}

/// The escape-sequence scope in effect at one point of a row.
///
/// SGR is a state machine over the byte stream, not a property of a sequence,
/// so a cell records the state it was written under rather than the sequences
/// that preceded it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Scope {
    sgr: String,
    hyperlink: Option<String>,
}

impl Scope {
    fn observe(&mut self, sequence: &str) {
        if sequence.starts_with("\x1b[") && sequence.ends_with('m') {
            let parameters = &sequence[2..sequence.len() - 1];
            if parameters.is_empty()
                || parameters
                    .split(';')
                    .all(|parameter| parameter.is_empty() || parameter == "0")
            {
                self.sgr.clear();
            } else {
                // The sequence is retained whole rather than parsed: replaying
                // it reproduces the state without having to understand every
                // extended colour form, and a partial reset replays harmlessly.
                self.sgr.push_str(sequence);
            }
            return;
        }

        if let Some(payload) = sequence.strip_prefix("\x1b]8;") {
            let payload = payload
                .strip_suffix("\x1b\\")
                .or_else(|| payload.strip_suffix('\u{07}'))
                .unwrap_or(payload);
            if let Some((_, uri)) = payload.split_once(';') {
                self.hyperlink = (!uri.is_empty()).then(|| sequence.to_owned());
            }
        }
    }

    fn open(&self) -> String {
        let mut output = self.sgr.clone();
        if let Some(hyperlink) = &self.hyperlink {
            output.push_str(hyperlink);
        }
        output
    }

    fn close(&self) -> String {
        let mut output = String::new();
        if self.hyperlink.is_some() {
            output.push_str("\x1b]8;;\x1b\\");
        }
        if !self.sgr.is_empty() {
            output.push_str("\x1b[0m");
        }
        output
    }
}

/// One terminal cell of a resolved row.
enum Cell {
    /// Never written to. Renders as a space.
    Empty,
    /// The first column of a grapheme, with the scope it was written under.
    Head {
        scope: Scope,
        text: String,
        width: usize,
    },
    /// A column covered by the wide grapheme to its left.
    Tail,
}

/// Erases whatever occupies `[column, column + width)`.
///
/// A grapheme is erased whole: overwriting either half of a wide character
/// removes all of it, because a terminal cannot show half a glyph, and the
/// columns it no longer covers fall back to blanks.
fn clear(cells: &mut [Cell], column: usize, width: usize) {
    let mut index = column;
    while index < column + width && index < cells.len() {
        let head = match &cells[index] {
            Cell::Empty => {
                index += 1;
                continue;
            }
            Cell::Head { .. } => index,
            Cell::Tail => (0..index)
                .rev()
                .find(|candidate| matches!(cells[*candidate], Cell::Head { .. }))
                .unwrap_or(index),
        };
        let span = match &cells[head] {
            Cell::Head { width, .. } => *width,
            _ => 1,
        };
        for cleared in head..(head + span).min(cells.len()) {
            cells[cleared] = Cell::Empty;
        }
        index = (head + span).max(index + 1);
    }
}

fn grow(cells: &mut Vec<Cell>, len: usize) {
    while cells.len() < len {
        cells.push(Cell::Empty);
    }
}

/// Whether a row has to be resolved to cells before it can be measured.
///
/// Any C0 control or DEL qualifies, not only the three that move the cursor.
/// `unicode-width` counts every C0 control as one cell, so a row carrying a
/// bell or a form feed would otherwise be measured a cell too wide and would
/// keep the control character in a block that promises to hold none. Escape is
/// excluded because the sequence it opens is handled on both paths.
fn needs_resolution(row: &str) -> bool {
    row.bytes()
        .any(|byte| (byte < 0x20 && byte != 0x1b) || byte == 0x7f)
}

/// Returns the cells a row occupies, skipping escape sequences.
///
/// Only valid for a row that holds no control character besides escape;
/// [`resolve_row`] handles the rest.
fn printable_width(row: &str) -> usize {
    if !row.contains('\u{1b}') {
        return width::text(PrintableText::new(row));
    }

    let mut printable = String::with_capacity(row.len());
    let mut offset = 0;
    while offset < row.len() {
        if row.as_bytes()[offset] == 0x1b {
            offset = ansi_sequence_end(row, offset);
            continue;
        }
        let next = row[offset..]
            .find('\u{1b}')
            .map_or(row.len(), |index| offset + index);
        printable.push_str(&row[offset..next]);
        offset = next;
    }
    width::text(PrintableText::new(printable.as_str()))
}

/// Returns the byte offset just past the escape sequence starting at `start`.
fn ansi_sequence_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let Some(kind) = bytes.get(start + 1).copied() else {
        return text.len();
    };

    match kind {
        b'[' => bytes[start + 2..]
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
            .map_or(text.len(), |index| start + 3 + index),
        b']' => control_string_end(bytes, start + 2, true),
        b'P' | b'X' | b'^' | b'_' => control_string_end(bytes, start + 2, false),
        _ => {
            let mut index = start + 1;
            while bytes
                .get(index)
                .is_some_and(|byte| (0x20..=0x2f).contains(byte))
            {
                index += 1;
            }
            bytes
                .get(index)
                .filter(|byte| (0x30..=0x7e).contains(*byte))
                .map_or((start + 1).min(text.len()), |_| index + 1)
        }
    }
}

fn control_string_end(bytes: &[u8], mut index: usize, allows_bell: bool) -> usize {
    while index < bytes.len() {
        if allows_bell && bytes[index] == 0x07 {
            return index + 1;
        }
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(row: &str) -> (String, usize) {
        let (text, width) = resolve_row(row);
        (text.into_owned(), width)
    }

    #[test]
    fn splits_rows_on_newlines_and_crlf() {
        assert_eq!(rows(""), Vec::<&str>::new());
        assert_eq!(rows("a"), ["a"]);
        assert_eq!(rows("a\nb"), ["a", "b"]);
        assert_eq!(rows("a\r\nb"), ["a", "b"]);
        assert_eq!(rows("a\n"), ["a", ""]);
        assert_eq!(rows("a\rb"), ["a\rb"]);
    }

    #[test]
    fn leaves_rows_without_cursor_movement_untouched() {
        let (text, width) = resolve_row("\x1b[31mabc\x1b[0m");
        assert!(matches!(text, Cow::Borrowed(_)));
        assert_eq!(width, 3);
        assert_eq!(printable_width("日本語"), 6);
        assert_eq!(
            printable_width("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"),
            4
        );
    }

    #[test]
    fn carriage_return_overwrites_from_the_first_column() {
        assert_eq!(resolved("ab\rc"), ("cb".to_owned(), 2));
        assert_eq!(resolved("\rProgress"), ("Progress".to_owned(), 8));
    }

    #[test]
    fn backspace_steps_back_one_column_and_stops_at_zero() {
        assert_eq!(resolved("abc\u{8}\u{8}X"), ("aXc".to_owned(), 3));
        assert_eq!(resolved("\u{8}\u{8}a"), ("a".to_owned(), 1));
    }

    #[test]
    fn tab_advances_to_the_next_tab_stop() {
        assert_eq!(resolved("a\tb"), ("a       b".to_owned(), 9));
        assert_eq!(
            resolved("12345678\tx"),
            ("12345678        x".to_owned(), 17)
        );
        assert_eq!(resolved("a\t"), ("a".to_owned(), 1));
    }

    #[test]
    fn a_zero_width_grapheme_takes_no_cell() {
        // The same text measures the same whether or not an unrelated cursor
        // movement sends the row down the resolving path.
        assert_eq!(resolve_row("a\u{200b}b").1, 2);
        assert_eq!(resolved("a\u{200b}b\u{8}b"), ("a\u{200b}b".to_owned(), 2));
        assert_eq!(resolved("\r\u{301}a"), ("\u{301}a".to_owned(), 1));
        assert_eq!(resolved("\r\u{301}"), ("\u{301}".to_owned(), 0));
    }

    #[test]
    fn other_control_characters_are_dropped_rather_than_measured() {
        for row in ["a\u{7}b", "a\0b", "a\u{7f}b", "a\u{b}b", "a\u{c}b"] {
            assert_eq!(resolved(row), ("ab".to_owned(), 2), "{row:?}");
        }
    }

    #[test]
    fn overwriting_half_a_wide_grapheme_erases_all_of_it() {
        assert_eq!(resolved("日\rx"), ("x".to_owned(), 1));
        assert_eq!(resolved("日本\r\u{8}ab"), ("ab本".to_owned(), 4));
        // The column the erased character no longer covers becomes a blank,
        // and is only invisible when it falls at the end of the row.
        assert_eq!(resolved("日本\ra"), ("a 本".to_owned(), 4));
    }

    #[test]
    fn a_cell_keeps_the_scope_it_was_written_under() {
        // The style in effect when `b` was written is the one it keeps, and
        // the row closes the scope it opened rather than leaking it.
        assert_eq!(
            resolved("\x1b[31ma\r\x1b[1mb"),
            ("\x1b[31m\x1b[1mb\x1b[0m".to_owned(), 1)
        );
        assert_eq!(
            resolved("\x1b[31mab\rc\x1b[0m"),
            ("\x1b[31mcb\x1b[0m".to_owned(), 2)
        );
    }

    #[test]
    fn a_scope_closed_before_a_carriage_return_still_covers_the_cells_it_wrapped() {
        // `a`, `b` and `c` are red, the scope closes, then `X` overwrites `a`
        // unstyled. Replaying the stream in order would move the reset onto
        // `X` and strip the colour from the cells that survived.
        assert_eq!(
            resolved("\x1b[31mabc\x1b[0m\rX"),
            ("X\x1b[31mbc\x1b[0m".to_owned(), 3)
        );
        assert_eq!(
            resolved("\x1b[31mab\x1b[0m\x1b[32mcd\x1b[0m\rZ"),
            ("Z\x1b[31mb\x1b[0m\x1b[32mcd\x1b[0m".to_owned(), 4)
        );
    }

    #[test]
    fn a_hyperlink_closed_before_a_carriage_return_keeps_the_text_it_wrapped() {
        assert_eq!(
            resolved("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\\rZ"),
            (
                "Z\x1b]8;;https://example.com\x1b\\ink\x1b]8;;\x1b\\".to_owned(),
                4
            )
        );
    }
}
