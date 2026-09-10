use std::fmt;

use unicode_segmentation::UnicodeSegmentation;

use crate::{Grapheme, TextStyle};

use super::Position;

/// How one command combines its contributions with preceding commands.
#[derive(Clone, Copy)]
pub enum Composition {
    Replace,
    Overlay,
    Custom(fn(&CanvasCell, &CellContribution) -> CanvasCell),
}

impl fmt::Debug for Composition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replace => f.write_str("Replace"),
            Self::Overlay => f.write_str("Overlay"),
            Self::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

/// Complete content of one Canvas cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanvasCell {
    symbol: String,
    style: TextStyle,
}

impl CanvasCell {
    /// Creates complete cell content.
    ///
    /// The symbol is validated at this boundary even in release builds.
    pub fn new(symbol: &Grapheme, style: TextStyle) -> Self {
        validate_symbol(symbol.as_str());
        Self {
            symbol: symbol.as_str().to_owned(),
            style,
        }
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn style(&self) -> &TextStyle {
        &self.style
    }

    /// Replaces the symbol, validating it even in release builds.
    pub fn with_symbol(mut self, symbol: &Grapheme) -> Self {
        validate_symbol(symbol.as_str());
        self.symbol = symbol.as_str().to_owned();
        self
    }

    pub fn with_style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }
}

/// A sparse contribution; absent fields are transparent under [`Composition::Overlay`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CellContribution {
    symbol: Option<String>,
    style: Option<TextStyle>,
}

impl CellContribution {
    pub fn new() -> Self {
        Self::default()
    }

    /// Contributes a symbol, validating it even in release builds.
    pub fn symbol(mut self, symbol: &Grapheme) -> Self {
        validate_symbol(symbol.as_str());
        self.symbol = Some(symbol.as_str().to_owned());
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn symbol_value(&self) -> Option<&str> {
        self.symbol.as_deref()
    }

    pub fn style_value(&self) -> Option<&TextStyle> {
        self.style.as_ref()
    }
}

/// Sparse positioned cell input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionedCell {
    pub position: Position,
    pub contribution: CellContribution,
}

impl PositionedCell {
    pub const fn new(position: Position, contribution: CellContribution) -> Self {
        Self {
            position,
            contribution,
        }
    }
}

pub(super) fn validate_symbol(symbol: &str) {
    assert!(
        !symbol.chars().any(char::is_control) && symbol.graphemes(true).count() == 1,
        "a Canvas cell symbol must be exactly one printable grapheme: {symbol:?}"
    );
}

pub(super) fn validate_cell_glyph(symbol: &str) {
    validate_symbol(symbol);
    assert_eq!(
        Grapheme::new(symbol).width(),
        1,
        "a rasterized cell glyph must occupy exactly one terminal cell: {symbol:?}"
    );
}
