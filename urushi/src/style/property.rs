//! Closed property types used by [`TextStyle`](crate::TextStyle) and
//! [`BlockStyle`](crate::BlockStyle).

use crate::{
    Align, Border, Color, Hyperlink, Length, Modifier, Overflow, Sides, Underline, VerticalAlign,
};

/// A value that can be added to a [`TextStyle`](crate::TextStyle).
///
/// Common properties also have named builder methods on `TextStyle`. This enum is
/// the exhaustive, data-oriented form for code that handles properties
/// generically. It carries text properties only; geometry belongs to
/// [`BlockStyleProperty`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextStyleProperty {
    Foreground(Color),
    Background(Color),
    Underline(Underline),
    Hyperlink(Hyperlink),
    Modifier(Modifier),
}

/// A property that can be removed from a [`TextStyle`](crate::TextStyle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextStylePropertyKey {
    Foreground,
    Background,
    Underline,
    Hyperlink,
    Modifier(Modifier),
}

impl From<Modifier> for TextStyleProperty {
    fn from(value: Modifier) -> Self {
        Self::Modifier(value)
    }
}

impl From<Underline> for TextStyleProperty {
    fn from(value: Underline) -> Self {
        Self::Underline(value)
    }
}

impl From<Hyperlink> for TextStyleProperty {
    fn from(value: Hyperlink) -> Self {
        Self::Hyperlink(value)
    }
}

impl From<Modifier> for TextStylePropertyKey {
    fn from(value: Modifier) -> Self {
        Self::Modifier(value)
    }
}

/// A value that can be added to a [`BlockStyle`](crate::BlockStyle).
///
/// The geometry of a block, plus — through [`BlockStyleProperty::Text`] — every
/// property of the [`TextStyle`](crate::TextStyle) that fills it.
///
/// This is not `Copy`: [`BlockStyleProperty::Overflow`] carries the marker a
/// clipped line ends with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlockStyleProperty {
    Text(TextStyleProperty),
    Padding(Sides),
    Margin(Sides),
    Border(Border),
    BorderTop(bool),
    BorderRight(bool),
    BorderBottom(bool),
    BorderLeft(bool),
    BorderForeground(Color),
    BorderBackground(Color),
    Width(Length),
    Height(Length),
    MinWidth(u16),
    MinHeight(u16),
    MaxWidth(u16),
    MaxHeight(u16),
    Overflow(Overflow),
    Align(Align),
    VerticalAlign(VerticalAlign),
}

/// A property that can be removed from a [`BlockStyle`](crate::BlockStyle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockStylePropertyKey {
    Text(TextStylePropertyKey),
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
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    Overflow,
    Align,
    VerticalAlign,
}

impl From<TextStyleProperty> for BlockStyleProperty {
    fn from(value: TextStyleProperty) -> Self {
        Self::Text(value)
    }
}

impl From<TextStylePropertyKey> for BlockStylePropertyKey {
    fn from(value: TextStylePropertyKey) -> Self {
        Self::Text(value)
    }
}

impl From<Modifier> for BlockStyleProperty {
    fn from(value: Modifier) -> Self {
        Self::Text(TextStyleProperty::Modifier(value))
    }
}

impl From<Underline> for BlockStyleProperty {
    fn from(value: Underline) -> Self {
        Self::Text(TextStyleProperty::Underline(value))
    }
}

impl From<Modifier> for BlockStylePropertyKey {
    fn from(value: Modifier) -> Self {
        Self::Text(TextStylePropertyKey::Modifier(value))
    }
}

/// A value that can be added to a [`GridStyle`](crate::GridStyle).
///
/// The lines a grid draws and the width its columns claim. A grid carries no
/// box geometry and no fill style, so this vocabulary has neither the box
/// properties of [`BlockStyleProperty`] nor a `Text` variant: a grid that needs
/// a border of its own or a stated size is placed inside a block, which has
/// them.
///
/// This is not `Copy`: [`GridStyleProperty::Columns`] carries one entry per
/// column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridStyleProperty {
    Border(Border),
    BorderTop(bool),
    BorderRight(bool),
    BorderBottom(bool),
    BorderLeft(bool),
    BorderColumn(bool),
    BorderRow(bool),
    BorderHeader(bool),
    BorderForeground(Color),
    BorderBackground(Color),
    Columns(Vec<Option<Length>>),
    CellPadding(Sides),
}

/// A property that can be removed from a [`GridStyle`](crate::GridStyle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridStylePropertyKey {
    Border,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    BorderColumn,
    BorderRow,
    BorderHeader,
    BorderForeground,
    BorderBackground,
    Columns,
    CellPadding,
}
