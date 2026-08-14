//! ANSI-aware terminal cell measurement.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns the terminal cells occupied by `text`, ignoring ANSI escape and
/// control-string sequences.
pub fn visible_width(text: &str) -> usize {
    if !text.contains('\u{1b}') {
        return UnicodeWidthStr::width(text);
    }

    let mut printable = String::with_capacity(text.len());
    let mut offset = 0;
    while offset < text.len() {
        if text.as_bytes()[offset] == 0x1b {
            offset = ansi_sequence_end(text, offset);
            continue;
        }

        let next_escape = text[offset..]
            .find('\u{1b}')
            .map_or(text.len(), |index| offset + index);
        printable.push_str(&text[offset..next_escape]);
        offset = next_escape;
    }
    UnicodeWidthStr::width(printable.as_str())
}

/// Truncates one rendered row to `max_width` terminal cells.
///
/// ANSI sequences in the retained prefix are copied atomically and do not
/// consume cells. Sequences in discarded text are dropped; active SGR and OSC
/// 8 scopes are closed safely. Printable text is cut only between graphemes.
pub(crate) fn truncate_visible_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if visible_width(text) <= max_width {
        return text.to_string();
    }

    let (graphemes, _) = ansi_graphemes(text);
    let mut output = String::with_capacity(text.len());
    let mut consumed = 0;
    let mut ansi_state = AnsiState::default();

    for grapheme in graphemes {
        if consumed + grapheme.width > max_width {
            break;
        }
        observe_ansi_sequences(&grapheme.rendered, &mut ansi_state);
        output.push_str(&grapheme.rendered);
        consumed += grapheme.width;
        if consumed == max_width {
            break;
        }
    }

    output.push_str(&ansi_state.closures());
    output
}

pub(super) struct AnsiGrapheme {
    pub(super) rendered: String,
    pub(super) width: usize,
}

pub(super) fn ansi_graphemes(text: &str) -> (Vec<AnsiGrapheme>, String) {
    let mut printable = String::with_capacity(text.len());
    let mut controls: Vec<(usize, &str)> = Vec::new();
    let mut offset = 0;
    while offset < text.len() {
        if text.as_bytes()[offset] == 0x1b {
            let end = ansi_sequence_end(text, offset);
            controls.push((printable.len(), &text[offset..end]));
            offset = end;
            continue;
        }

        let next_escape = text[offset..]
            .find('\u{1b}')
            .map_or(text.len(), |index| offset + index);
        printable.push_str(&text[offset..next_escape]);
        offset = next_escape;
    }

    let mut output = Vec::new();
    let mut control_index = 0;
    for (start, grapheme) in printable.grapheme_indices(true) {
        let end = start + grapheme.len();
        let mut rendered = String::with_capacity(grapheme.len());
        for (relative, character) in grapheme.char_indices() {
            let character_offset = start + relative;
            while controls
                .get(control_index)
                .is_some_and(|(offset, _)| *offset == character_offset)
            {
                rendered.push_str(controls[control_index].1);
                control_index += 1;
            }
            rendered.push(character);
        }
        output.push(AnsiGrapheme {
            rendered,
            width: UnicodeWidthStr::width(&printable[start..end]),
        });
    }

    let mut trailing_controls = String::new();
    for (_, sequence) in &controls[control_index..] {
        trailing_controls.push_str(sequence);
    }
    (output, trailing_controls)
}

/// Makes ANSI SGR and OSC 8 scopes self-contained on every rendered row.
pub(crate) fn normalize_ansi_rows(rows: &mut [String]) {
    let mut state = AnsiState::default();
    for row in rows {
        let reopen = state.reopen();
        observe_ansi_sequences(row, &mut state);
        let closures = state.closures();
        if !reopen.is_empty() {
            row.insert_str(0, &reopen);
        }
        row.push_str(&closures);
    }
}

fn observe_ansi_sequences(text: &str, state: &mut AnsiState) {
    let mut offset = 0;
    while let Some(relative) = text[offset..].find('\u{1b}') {
        let start = offset + relative;
        let end = ansi_sequence_end(text, start);
        state.observe(&text[start..end]);
        offset = end;
    }
}

pub(super) fn ansi_sequence_end(text: &str, start: usize) -> usize {
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

#[derive(Debug, Default)]
struct AnsiState {
    sgr_reopen: String,
    hyperlink_reopen: Option<String>,
}

impl AnsiState {
    fn observe(&mut self, sequence: &str) {
        if sequence.starts_with("\x1b[") && sequence.ends_with('m') {
            let parameters = &sequence[2..sequence.len() - 1];
            if parameters.is_empty()
                || parameters
                    .split(';')
                    .all(|parameter| parameter.is_empty() || parameter == "0")
            {
                self.sgr_reopen.clear();
            } else {
                // Retain the sequence so the same state can be restored after
                // a synthetic row-boundary reset. Partial resets are harmless
                // when replayed and avoid guessing extended color parameters.
                self.sgr_reopen.push_str(sequence);
            }
            return;
        }

        if let Some(payload) = sequence.strip_prefix("\x1b]8;") {
            let payload = payload
                .strip_suffix("\x1b\\")
                .or_else(|| payload.strip_suffix('\u{07}'))
                .unwrap_or(payload);
            if let Some((_, uri)) = payload.split_once(';') {
                self.hyperlink_reopen = (!uri.is_empty()).then(|| sequence.to_string());
            }
        }
    }

    fn reopen(&self) -> String {
        let mut output = String::new();
        if let Some(hyperlink) = &self.hyperlink_reopen {
            output.push_str(hyperlink);
        }
        output.push_str(&self.sgr_reopen);
        output
    }

    fn closures(&self) -> String {
        let mut output = String::new();
        if self.hyperlink_reopen.is_some() {
            output.push_str("\x1b]8;;\x1b\\");
        }
        if !self.sgr_reopen.is_empty() {
            output.push_str("\x1b[0m");
        }
        output
    }
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
        assert_eq!(visible_width("e\x1b[31m\u{301}"), 1);
        assert_eq!(visible_width("👩\x1b[31m\u{200d}💻"), 2);
    }

    #[test]
    fn counts_wide_characters() {
        assert_eq!(visible_width("日本語"), 6);
        assert_eq!(visible_width("aあ"), 3);
        assert_eq!(visible_width("👩‍💻"), 2);
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn truncates_without_splitting_graphemes_or_wide_characters() {
        assert_eq!(truncate_visible_width("A日本", 3), "A日");
        assert_eq!(truncate_visible_width("e\u{301}x", 1), "e\u{301}");
        assert_eq!(truncate_visible_width("日本", 1), "");
    }

    #[test]
    fn truncates_ansi_text_and_keeps_control_sequences_atomic() {
        assert_eq!(
            truncate_visible_width("\x1b[31m日本語\x1b[0m", 4),
            "\x1b[31m日本\x1b[0m"
        );
        assert_eq!(
            truncate_visible_width(
                "\x1b]8;;https://example.com\x1b\\link text\x1b]8;;\x1b\\",
                4,
            ),
            "\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"
        );
        assert_eq!(
            truncate_visible_width("e\x1b[31m\u{301}x", 1),
            "e\x1b[31m\u{301}\x1b[0m"
        );
        assert_eq!(
            truncate_visible_width("👩\x1b[31m\u{200d}💻x", 2),
            "👩\x1b[31m\u{200d}💻\x1b[0m"
        );
    }

    #[test]
    fn normalizes_multiline_sgr_and_hyperlink_scopes() {
        let mut rows = vec![
            "\x1b[38;2;255;0;0mred".to_string(),
            "text\x1b[0m".to_string(),
        ];
        normalize_ansi_rows(&mut rows);
        assert_eq!(rows[0], "\x1b[38;2;255;0;0mred\x1b[0m");
        assert_eq!(rows[1], "\x1b[38;2;255;0;0mtext\x1b[0m");

        let mut links = vec![
            "\x1b]8;;https://example.com\x1b\\link".to_string(),
            "text\x1b]8;;\x1b\\".to_string(),
        ];
        normalize_ansi_rows(&mut links);
        assert_eq!(
            links[0],
            "\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"
        );
        assert_eq!(
            links[1],
            "\x1b]8;;https://example.com\x1b\\text\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn discards_controls_after_truncation_and_keeps_escape_sequences_atomic() {
        assert_eq!(truncate_visible_width("abcdef\x1b[2J", 2), "ab");
        assert_eq!(truncate_visible_width("a\x1b7b", 1), "a");
        assert_eq!(truncate_visible_width("\x1b7a", 1), "\x1b7a");
    }
}
