//! Renderer-neutral terminal output.

use crate::Style;

/// One styled text span in a [`Line`].
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    text: String,
    style: Style,
}

impl Span {
    /// Creates a span with logical styling that has not been terminal-resolved.
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Returns the span text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the logical style.
    pub fn style(&self) -> &Style {
        &self.style
    }
}

/// One horizontal row of styled spans.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Line {
    spans: Vec<Span>,
}

impl Line {
    /// Creates an empty line.
    pub const fn new() -> Self {
        Self { spans: Vec::new() }
    }

    /// Creates a line containing one span.
    pub fn styled(text: impl Into<String>, style: Style) -> Self {
        Self::new().span(text, style)
    }

    /// Appends a styled span.
    #[must_use]
    pub fn span(mut self, text: impl Into<String>, style: Style) -> Self {
        self.spans.push(Span::new(text, style));
        self
    }

    /// Returns the spans in display order.
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }
}

/// A fully composed, renderer-neutral terminal view.
///
/// Components return a `View`; output adapters decide how its logical styles
/// map to ANSI sequences, a terminal buffer, or another representation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct View {
    lines: Vec<Line>,
}

impl View {
    /// Creates an empty view.
    pub const fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Creates a one-line view.
    pub fn line(line: Line) -> Self {
        Self { lines: vec![line] }
    }

    /// Appends one line.
    #[must_use]
    pub fn push(mut self, line: Line) -> Self {
        self.lines.push(line);
        self
    }

    /// Appends all lines from another view.
    #[must_use]
    pub fn extend(mut self, other: Self) -> Self {
        self.lines.extend(other.lines);
        self
    }

    /// Returns the lines in display order.
    pub fn lines(&self) -> &[Line] {
        &self.lines
    }

    /// Returns whether the view contains no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_preserves_line_and_span_order() {
        let view = View::line(Line::new().span("a", Style::new()).span("b", Style::new()))
            .push(Line::styled("c", Style::new()));

        assert_eq!(view.lines().len(), 2);
        assert_eq!(view.lines()[0].spans()[0].text(), "a");
        assert_eq!(view.lines()[0].spans()[1].text(), "b");
        assert_eq!(view.lines()[1].spans()[0].text(), "c");
    }
}
