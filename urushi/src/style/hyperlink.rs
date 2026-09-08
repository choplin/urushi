//! A safe OSC 8 hyperlink value.

use std::sync::Arc;

/// A terminal hyperlink attached to one run of text.
///
/// The URI and parameters are encoded at construction time so their contents
/// cannot terminate the OSC 8 sequence or introduce another parameter. OSC 8
/// parameter names and values remain intentionally open-ended; terminals
/// currently define `id`, while future terminals may add more keys.
///
/// ```
/// use urushi::{Hyperlink, TextStyle};
///
/// let plain = TextStyle::new().hyperlink("https://example.com");
/// let identified = TextStyle::new().hyperlink(
///     Hyperlink::new("https://example.com").with_parameter("id", "documentation"),
/// );
///
/// assert_eq!(plain.hyperlink_value().unwrap().uri(), "https://example.com");
/// assert_eq!(identified.hyperlink_value().unwrap().parameters().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    uri: Arc<str>,
    parameters: Arc<[(String, String)]>,
}

impl Hyperlink {
    /// Creates a hyperlink to `uri` without parameters.
    ///
    /// Control characters are UTF-8 percent-encoded so the URI cannot close the
    /// OSC string or inject terminal control bytes.
    pub fn new(uri: impl AsRef<str>) -> Self {
        Self {
            uri: encode(uri.as_ref(), false).into(),
            parameters: Arc::from([]),
        }
    }

    /// Adds one OSC 8 parameter.
    ///
    /// OSC 8 separates parameters with `:` and the key from its value with
    /// `=`. Those delimiters, `;`, and control characters are percent-encoded in
    /// both parts, so arbitrary caller input remains one inert parameter.
    pub fn with_parameter(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let mut parameters = self.parameters.to_vec();
        parameters.push((encode(name.as_ref(), true), encode(value.as_ref(), true)));
        self.parameters = parameters.into();
        self
    }

    /// Returns the encoded URI emitted by the ANSI renderer.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the encoded parameters in insertion order.
    pub fn parameters(&self) -> &[(String, String)] {
        &self.parameters
    }

    pub(crate) fn open_sequence(&self) -> String {
        let parameters = self
            .parameters
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join(":");
        format!("\x1b]8;{parameters};{}\x1b\\", self.uri)
    }
}

impl From<&str> for Hyperlink {
    fn from(uri: &str) -> Self {
        Self::new(uri)
    }
}

impl From<String> for Hyperlink {
    fn from(uri: String) -> Self {
        Self::new(uri)
    }
}

fn encode(input: &str, parameter: bool) -> String {
    let mut encoded = String::with_capacity(input.len());
    for character in input.chars() {
        if character.is_control() || (parameter && matches!(character, ':' | '=' | ';')) {
            let mut buffer = [0; 4];
            for byte in character.encode_utf8(&mut buffer).bytes() {
                encoded.push('%');
                encoded.push(hex(byte >> 4));
                encoded.push(hex(byte & 0x0f));
            }
        } else {
            encoded.push(character);
        }
    }
    encoded
}

const fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'A' + nibble - 10) as char,
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_encodes_every_osc_delimiter_the_value_owns() {
        let hyperlink = Hyperlink::new("https://example.com/a\u{1b}\\b\u{7}\u{9c}")
            .with_parameter("i:d=;\u{9d}", "v:a=l;ue\n\u{9b}");

        assert_eq!(hyperlink.uri(), "https://example.com/a%1B\\b%07%C2%9C");
        assert_eq!(
            hyperlink.parameters(),
            &[(
                "i%3Ad%3D%3B%C2%9D".to_owned(),
                "v%3Aa%3Dl%3Bue%0A%C2%9B".to_owned()
            )]
        );
        assert_eq!(
            hyperlink.open_sequence(),
            concat!(
                "\x1b]8;i%3Ad%3D%3B%C2%9D=v%3Aa%3Dl%3Bue%0A%C2%9B;",
                "https://example.com/a%1B\\b%07%C2%9C\x1b\\",
            )
        );
    }
}
