//! Styled plain text before it enters layout.

use std::fmt;
use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use super::tab::DEFAULT_TAB_POLICY;
use super::{Grapheme, TabPolicy};
use crate::TextStyle;

/// One caller-authored piece of text carrying one complete style.
///
/// A span is an input segment, not a layout boundary. [`StyledText`] joins all
/// spans before it decides grapheme, wrapping, or clipping boundaries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextSpan {
    text: String,
    style: TextStyle,
}

impl TextSpan {
    /// Creates one caller-authored segment.
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Returns this segment's source text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the complete style assigned to this segment.
    pub const fn style(&self) -> &TextStyle {
        &self.style
    }
}

impl From<String> for TextSpan {
    fn from(text: String) -> Self {
        Self::new(text, TextStyle::new())
    }
}

impl From<&str> for TextSpan {
    fn from(text: &str) -> Self {
        Self::new(text, TextStyle::new())
    }
}

/// A grapheme boundary between two input spans was invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyledTextError {
    span: usize,
    byte_offset: usize,
}

impl StyledTextError {
    /// The zero-based input span whose end split a grapheme cluster.
    pub const fn span(&self) -> usize {
        self.span
    }

    /// The offending byte offset in the concatenated text.
    pub const fn byte_offset(&self) -> usize {
        self.byte_offset
    }
}

impl fmt::Display for StyledTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "text span {} ends at byte {}, inside a grapheme cluster",
            self.span, self.byte_offset
        )
    }
}

impl std::error::Error for StyledTextError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SpanRange {
    range: Range<usize>,
    style: TextStyle,
}

/// One text flow carrying any number of styles.
///
/// The source is stored once. Its private ranges are canonical: they are
/// non-empty, cover the source in order, end only at whole-string grapheme
/// boundaries, and never place equal styles beside each other. Newline and
/// horizontal tab are admitted source controls; layout replaces tabs under
/// this value's policy, while direct text rendering preserves them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledText {
    text: String,
    spans: Vec<SpanRange>,
    tab_policy: Option<TabPolicy>,
}

impl StyledText {
    /// Creates one uniformly styled text flow.
    ///
    /// # Panics
    ///
    /// Panics when `text` contains a control character other than newline or
    /// horizontal tab. Rendered ANSI belongs to the output domain, not this
    /// source-text domain.
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        let text = text.into();
        validate_source(&text);
        let spans = (!text.is_empty())
            .then_some(SpanRange {
                range: 0..text.len(),
                style,
            })
            .into_iter()
            .collect();
        Self {
            text,
            spans,
            tab_policy: None,
        }
    }

    /// Joins input segments into one validated, canonical text flow.
    ///
    /// # Panics
    ///
    /// Panics when any segment contains a control character other than newline
    /// or horizontal tab.
    pub fn try_from_spans<I, S>(spans: I) -> Result<Self, StyledTextError>
    where
        I: IntoIterator<Item = S>,
        S: Into<TextSpan>,
    {
        let spans: Vec<TextSpan> = spans.into_iter().map(Into::into).collect();
        let mut text = String::new();
        let mut boundaries = Vec::new();
        for (index, span) in spans.iter().enumerate() {
            text.push_str(&span.text);
            if !span.text.is_empty() && index + 1 < spans.len() {
                boundaries.push((index, text.len()));
            }
        }
        validate_source(&text);

        let grapheme_boundaries: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
            .collect();
        if let Some((span, byte_offset)) = boundaries
            .into_iter()
            .find(|(_, offset)| grapheme_boundaries.binary_search(offset).is_err())
        {
            return Err(StyledTextError { span, byte_offset });
        }

        let mut ranges: Vec<SpanRange> = Vec::new();
        let mut offset = 0;
        for span in spans {
            let start = offset;
            offset += span.text.len();
            if start == offset {
                continue;
            }
            if let Some(previous) = ranges.last_mut()
                && previous.style == span.style
            {
                previous.range.end = offset;
            } else {
                ranges.push(SpanRange {
                    range: start..offset,
                    style: span.style,
                });
            }
        }
        Ok(Self {
            text,
            spans: ranges,
            tab_policy: None,
        })
    }

    /// Returns the joined plain-text source.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Returns the canonical styled segments in source order.
    pub fn spans(&self) -> impl Iterator<Item = (&str, &TextStyle)> {
        self.spans
            .iter()
            .map(|span| (&self.text[span.range.clone()], &span.style))
    }

    /// Sets the policy used when this text participates in layout.
    ///
    /// Direct text rendering preserves the source tab characters and ignores
    /// this layout-only property.
    pub fn with_tab_policy(mut self, policy: TabPolicy) -> Self {
        self.tab_policy = Some(policy);
        self
    }

    /// Restores the default layout policy of four spaces per tab.
    pub fn without_tab_policy(mut self) -> Self {
        self.tab_policy = None;
        self
    }

    /// Returns the explicitly configured layout policy, if any.
    pub const fn tab_policy(&self) -> Option<&TabPolicy> {
        self.tab_policy.as_ref()
    }

    /// Splits the whole text into rows of styled graphemes.
    pub(crate) fn lines(&self) -> Vec<Vec<StyledTextGrapheme<'_>>> {
        let policy = self.tab_policy.as_ref().unwrap_or(&DEFAULT_TAB_POLICY);
        let mut lines = Vec::new();
        let mut line = Vec::new();
        let mut span = 0;
        for (offset, symbol) in self.text.grapheme_indices(true) {
            if symbol == "\n" {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            while self.spans[span].range.end <= offset {
                span += 1;
            }
            if symbol == "\t" {
                append_tab(&mut line, policy, &self.spans[span].style);
                continue;
            }
            line.push(StyledTextGrapheme {
                grapheme: Grapheme::new(symbol),
                style: &self.spans[span].style,
            });
        }
        if !line.is_empty() || !self.text.ends_with('\n') {
            lines.push(line);
        }
        if lines.is_empty() {
            lines.push(Vec::new());
        }
        lines
    }

    pub(crate) fn uniform_style(&self) -> Option<&TextStyle> {
        (self.spans.len() == 1).then(|| &self.spans[0].style)
    }
}

fn validate_source(text: &str) {
    assert!(
        !text
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t'),
        "styled text must not carry terminal control characters other than newline and tab: {text:?}"
    );
}

fn append_tab<'a>(
    line: &mut Vec<StyledTextGrapheme<'a>>,
    policy: &'a TabPolicy,
    style: &'a TextStyle,
) {
    let mut occupied = 0;
    if let Some(marker) = policy.marker() {
        for symbol in marker.graphemes(true) {
            let grapheme = Grapheme::new(symbol);
            occupied += grapheme.width();
            line.push(StyledTextGrapheme { grapheme, style });
        }
    }
    for _ in occupied..usize::from(policy.width()) {
        line.push(StyledTextGrapheme {
            grapheme: Grapheme::space(),
            style,
        });
    }
}

impl From<String> for StyledText {
    fn from(text: String) -> Self {
        Self::new(text, TextStyle::new())
    }
}

impl From<&str> for StyledText {
    fn from(text: &str) -> Self {
        Self::new(text, TextStyle::new())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct StyledTextGrapheme<'a> {
    pub grapheme: &'a Grapheme,
    pub style: &'a TextStyle,
}

impl StyledTextGrapheme<'_> {
    pub fn width(self) -> usize {
        self.grapheme.width()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;

    #[test]
    fn strings_become_default_spans() {
        let owned = TextSpan::from(String::from("owned"));
        let borrowed = TextSpan::from("borrowed");

        assert_eq!(owned, TextSpan::new("owned", TextStyle::new()));
        assert_eq!(borrowed, TextSpan::new("borrowed", TextStyle::new()));
    }

    #[test]
    fn canonicalizes_empty_and_adjacent_equal_spans() {
        let red = TextStyle::new().foreground(Color::RED);
        let text = StyledText::try_from_spans([
            TextSpan::new("a", red.clone()),
            TextSpan::new("", TextStyle::new()),
            TextSpan::new("b", red.clone()),
        ])
        .unwrap();

        assert_eq!(text.as_str(), "ab");
        assert_eq!(text.spans().collect::<Vec<_>>(), [("ab", &red)]);
    }

    #[test]
    fn rejects_a_span_boundary_inside_a_whole_text_grapheme() {
        let error = StyledText::try_from_spans([
            TextSpan::new("e", TextStyle::new()),
            TextSpan::new("\u{301}", TextStyle::new().bold()),
        ])
        .unwrap_err();

        assert_eq!(error.span(), 0);
        assert_eq!(error.byte_offset(), 1);
    }

    #[test]
    fn assigns_styles_after_segmenting_the_whole_text() {
        let red = TextStyle::new().foreground(Color::RED);
        let text = StyledText::try_from_spans([
            TextSpan::new("日", red.clone()),
            TextSpan::new("👩‍💻", TextStyle::new()),
        ])
        .unwrap();
        let lines = text.lines();

        assert_eq!(lines[0].len(), 2);
        assert_eq!(lines[0][0].grapheme.as_str(), "日");
        assert_eq!(lines[0][0].style, &red);
        assert_eq!(lines[0][1].grapheme.as_str(), "👩‍💻");
    }
}
