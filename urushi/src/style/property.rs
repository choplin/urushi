//! Closed property types used by [`Style`](crate::Style).

use crate::{Align, Border, Color, Modifier, Sides, VerticalAlign};

/// A value that can be added to a [`Style`](crate::Style).
///
/// Common properties also have named builder methods on `Style`. This enum is
/// the exhaustive, data-oriented form for code that handles properties
/// generically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleProperty {
    Foreground(Color),
    Background(Color),
    Modifier(Modifier),
    Padding(Sides),
    Margin(Sides),
    Border(Border),
    BorderTop(bool),
    BorderRight(bool),
    BorderBottom(bool),
    BorderLeft(bool),
    BorderForeground(Color),
    BorderBackground(Color),
    Width(u16),
    Height(u16),
    MaxWidth(u16),
    MaxHeight(u16),
    Align(Align),
    VerticalAlign(VerticalAlign),
}

/// A property that can be removed from a [`Style`](crate::Style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StylePropertyKey {
    Foreground,
    Background,
    Modifier(Modifier),
    Padding,
    Margin,
    Border,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    BorderForeground,
    BorderBackground,
    Width,
    Height,
    MaxWidth,
    MaxHeight,
    Align,
    VerticalAlign,
}

impl From<Modifier> for StyleProperty {
    fn from(value: Modifier) -> Self {
        Self::Modifier(value)
    }
}

impl From<Modifier> for StylePropertyKey {
    fn from(value: Modifier) -> Self {
        Self::Modifier(value)
    }
}
