//! A logical terminal hyperlink value.

use std::sync::Arc;

/// A terminal hyperlink attached to one run of text.
///
/// The URI and parameters remain logical data. The selected terminal output
/// boundary escapes and frames them when it encodes OSC 8. Parameter names and
/// values remain intentionally open-ended; terminals currently define `id`,
/// while future terminals may add more keys.
///
/// ```
/// use urushi::{Hyperlink, TextStyle};
///
/// let plain = TextStyle::new().hyperlink("https://example.com");
/// let identified = TextStyle::new().hyperlink(
///     Hyperlink::new("https://example.com").parameter("id", "documentation"),
/// );
///
/// assert_eq!(plain.get_hyperlink().unwrap().uri(), "https://example.com");
/// assert_eq!(identified.get_hyperlink().unwrap().parameters().len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hyperlink {
    uri: Arc<str>,
    parameters: Arc<[(String, String)]>,
}

impl Hyperlink {
    /// Creates a hyperlink to `uri` without parameters.
    ///
    pub fn new(uri: impl AsRef<str>) -> Self {
        Self {
            uri: uri.as_ref().into(),
            parameters: Arc::from([]),
        }
    }

    /// Adds one OSC 8 parameter.
    ///
    pub fn parameter(mut self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let mut parameters = self.parameters.to_vec();
        parameters.push((name.as_ref().to_owned(), value.as_ref().to_owned()));
        self.parameters = parameters.into();
        self
    }

    /// Returns the logical URI supplied by the caller.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the logical parameters in insertion order.
    pub fn parameters(&self) -> &[(String, String)] {
        &self.parameters
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_preserves_logical_hyperlink_data() {
        let hyperlink = Hyperlink::new("https://example.com/a\u{1b}\\b\u{7}\u{9c}")
            .parameter("i:d=;\u{9d}", "v:a=l;ue\n\u{9b}");

        assert_eq!(hyperlink.uri(), "https://example.com/a\u{1b}\\b\u{7}\u{9c}");
        assert_eq!(
            hyperlink.parameters(),
            &[("i:d=;\u{9d}".to_owned(), "v:a=l;ue\n\u{9b}".to_owned())]
        );
    }
}
