//! Private marker normalization shared by marker-based components.

pub(super) fn normalize_marker(marker: String) -> String {
    let mut output = String::with_capacity(marker.len());
    let mut characters = marker.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\r' => {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                output.push(' ');
            }
            '\n' | '\u{2028}' | '\u{2029}' => output.push(' '),
            other => output.push(other),
        }
    }
    output
}
