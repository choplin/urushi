//! The [`BlockStyle`] builder: a rectangle, and the style filling the geometry
//! it creates.

use crate::view::{Limits, RenderedBlock, View, resolve};
use crate::{
    Align, BlockStyleProperty, BlockStylePropertyKey, Border, Color, Modifier, Sides, TextStyle,
    TextStyleProperty, VerticalAlign,
};

/// A rectangle: padding, margin, border, dimensions, alignment, and the
/// [`TextStyle`] that fills the geometry they create.
///
/// A `BlockStyle` is an immutable value, like [`TextStyle`]. Its text properties —
/// colors and modifiers — are the style of the block's own fill: padding rows,
/// alignment gaps, and the content of [`BlockStyle::render`]. A block's style
/// does not flow into a child view; each child carries its own complete value.
///
/// ```
/// use urushi::{Align, BlockStyle, Border, Color};
///
/// let panel = BlockStyle::new()
///     .foreground(Color::CYAN)
///     .border(Border::ROUNDED)
///     .padding((0, 1))
///     .align(Align::Center)
///     .width(20);
///
/// println!("{}", panel.render("こんにちは, urushi!"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStyle {
    text: TextStyle,
    padding: Sides,
    margin: Sides,
    border: Option<Border>,
    border_top: bool,
    border_right: bool,
    border_bottom: bool,
    border_left: bool,
    border_fg: Option<Color>,
    border_bg: Option<Color>,
    width: Option<u16>,
    height: Option<u16>,
    max_width: Option<u16>,
    max_height: Option<u16>,
    align: Align,
    vertical_align: VerticalAlign,
}

impl Default for BlockStyle {
    fn default() -> Self {
        Self {
            text: TextStyle::new(),
            padding: Sides::default(),
            margin: Sides::default(),
            border: None,
            border_top: true,
            border_right: true,
            border_bottom: true,
            border_left: true,
            border_fg: None,
            border_bg: None,
            width: None,
            height: None,
            max_width: None,
            max_height: None,
            align: Align::default(),
            vertical_align: VerticalAlign::default(),
        }
    }
}

impl BlockStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a block style filled with `text`.
    pub fn from_text_style(text: TextStyle) -> Self {
        Self {
            text,
            ..Self::default()
        }
    }

    /// Adds or replaces a property in this style.
    // This is the collection operation paired with `remove`, not arithmetic.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, property: impl Into<BlockStyleProperty>) -> Self {
        match property.into() {
            BlockStyleProperty::Text(property) => self.text = self.text.add(property),
            BlockStyleProperty::Padding(sides) => self.padding = sides,
            BlockStyleProperty::Margin(sides) => self.margin = sides,
            BlockStyleProperty::Border(border) => self.border = Some(border),
            BlockStyleProperty::BorderTop(enabled) => self.border_top = enabled,
            BlockStyleProperty::BorderRight(enabled) => self.border_right = enabled,
            BlockStyleProperty::BorderBottom(enabled) => self.border_bottom = enabled,
            BlockStyleProperty::BorderLeft(enabled) => self.border_left = enabled,
            BlockStyleProperty::BorderForeground(color) => self.border_fg = Some(color),
            BlockStyleProperty::BorderBackground(color) => self.border_bg = Some(color),
            BlockStyleProperty::Width(width) => self.width = Some(width),
            BlockStyleProperty::Height(height) => self.height = Some(height),
            BlockStyleProperty::MaxWidth(width) => self.max_width = Some(width),
            BlockStyleProperty::MaxHeight(height) => self.max_height = Some(height),
            BlockStyleProperty::Align(align) => self.align = align,
            BlockStyleProperty::VerticalAlign(align) => self.vertical_align = align,
        }
        self
    }

    /// Removes a property from this style, restoring its default value.
    pub fn remove(mut self, property: impl Into<BlockStylePropertyKey>) -> Self {
        match property.into() {
            BlockStylePropertyKey::Text(property) => self.text = self.text.remove(property),
            BlockStylePropertyKey::Padding => self.padding = Sides::default(),
            BlockStylePropertyKey::Margin => self.margin = Sides::default(),
            BlockStylePropertyKey::Border => self.border = None,
            BlockStylePropertyKey::BorderTop => self.border_top = true,
            BlockStylePropertyKey::BorderRight => self.border_right = true,
            BlockStylePropertyKey::BorderBottom => self.border_bottom = true,
            BlockStylePropertyKey::BorderLeft => self.border_left = true,
            BlockStylePropertyKey::BorderForeground => self.border_fg = None,
            BlockStylePropertyKey::BorderBackground => self.border_bg = None,
            BlockStylePropertyKey::Width => self.width = None,
            BlockStylePropertyKey::Height => self.height = None,
            BlockStylePropertyKey::MaxWidth => self.max_width = None,
            BlockStylePropertyKey::MaxHeight => self.max_height = None,
            BlockStylePropertyKey::Align => self.align = Align::default(),
            BlockStylePropertyKey::VerticalAlign => self.vertical_align = VerticalAlign::default(),
        }
        self
    }

    /// Replaces the style filling this block.
    pub fn text_style(mut self, text: TextStyle) -> Self {
        self.text = text;
        self
    }

    /// Returns the style filling this block.
    pub const fn text(&self) -> &TextStyle {
        &self.text
    }

    /// Sets the fill (and text) foreground color.
    pub fn foreground(self, color: impl Into<Color>) -> Self {
        self.add(TextStyleProperty::Foreground(color.into()))
    }

    /// Sets the fill (and text) background color.
    pub fn background(self, color: impl Into<Color>) -> Self {
        self.add(TextStyleProperty::Background(color.into()))
    }

    pub fn bold(self) -> Self {
        self.add(Modifier::BOLD)
    }

    pub fn dim(self) -> Self {
        self.add(Modifier::DIM)
    }

    pub fn italic(self) -> Self {
        self.add(Modifier::ITALIC)
    }

    pub fn underline(self) -> Self {
        self.add(Modifier::UNDERLINED)
    }

    pub fn blink(self) -> Self {
        self.add(Modifier::SLOW_BLINK)
    }

    pub fn reverse(self) -> Self {
        self.add(Modifier::REVERSED)
    }

    pub fn strikethrough(self) -> Self {
        self.add(Modifier::CROSSED_OUT)
    }

    /// Sets padding between the content and the border.
    pub fn padding(self, sides: impl Into<Sides>) -> Self {
        self.add(BlockStyleProperty::Padding(sides.into()))
    }

    /// Sets unstyled spacing outside the border.
    pub fn margin(self, sides: impl Into<Sides>) -> Self {
        self.add(BlockStyleProperty::Margin(sides.into()))
    }

    /// Sets the border glyphs around the padded content.
    ///
    /// A new style enables all four edges. Use the `border_*` builders to
    /// configure edge visibility independently.
    pub fn border(self, border: Border) -> Self {
        self.add(BlockStyleProperty::Border(border))
    }

    /// Enables or disables the top border edge.
    pub fn border_top(self, enabled: bool) -> Self {
        self.add(BlockStyleProperty::BorderTop(enabled))
    }

    /// Enables or disables the right border edge.
    pub fn border_right(self, enabled: bool) -> Self {
        self.add(BlockStyleProperty::BorderRight(enabled))
    }

    /// Enables or disables the bottom border edge.
    pub fn border_bottom(self, enabled: bool) -> Self {
        self.add(BlockStyleProperty::BorderBottom(enabled))
    }

    /// Enables or disables the left border edge.
    pub fn border_left(self, enabled: bool) -> Self {
        self.add(BlockStyleProperty::BorderLeft(enabled))
    }

    /// Sets the border foreground color.
    pub fn border_foreground(self, color: impl Into<Color>) -> Self {
        self.add(BlockStyleProperty::BorderForeground(color.into()))
    }

    /// Sets the border background color.
    pub fn border_background(self, color: impl Into<Color>) -> Self {
        self.add(BlockStyleProperty::BorderBackground(color.into()))
    }

    /// Fixes the width of the padded content box (excluding border and
    /// margin). Text content is word-wrapped to fit.
    pub fn width(self, width: u16) -> Self {
        self.add(BlockStyleProperty::Width(width))
    }

    /// Sets the minimum height of the padded content box (excluding border and
    /// margin). Taller content expands the box instead of being truncated.
    pub fn height(self, height: u16) -> Self {
        self.add(BlockStyleProperty::Height(height))
    }

    /// Limits the final block width, including padding, border, and margin.
    /// Positive values crop without rewrapping; zero disables the constraint.
    pub fn max_width(self, width: u16) -> Self {
        self.add(BlockStyleProperty::MaxWidth(width))
    }

    /// Limits the final block height, including padding, border, and margin.
    /// Positive values remove rows from the bottom; zero disables the
    /// constraint.
    pub fn max_height(self, height: u16) -> Self {
        self.add(BlockStyleProperty::MaxHeight(height))
    }

    /// Sets the horizontal alignment of content within the box.
    pub fn align(self, align: Align) -> Self {
        self.add(BlockStyleProperty::Align(align))
    }

    /// Sets the vertical alignment of content within a fixed-height box.
    pub fn align_vertical(self, align: VerticalAlign) -> Self {
        self.add(BlockStyleProperty::VerticalAlign(align))
    }

    /// Returns the fill foreground color instruction, if one is set.
    pub const fn foreground_color(&self) -> Option<Color> {
        self.text.foreground_color()
    }

    /// Returns the fill background color instruction, if one is set.
    pub const fn background_color(&self) -> Option<Color> {
        self.text.background_color()
    }

    /// Returns the active fill text modifiers.
    pub const fn modifiers(&self) -> Modifier {
        self.text.modifiers()
    }

    /// Returns the padding applied inside the border.
    pub const fn padding_sides(&self) -> Sides {
        self.padding
    }

    /// Returns the unstyled margin applied outside the border.
    pub const fn margin_sides(&self) -> Sides {
        self.margin
    }

    /// Returns the border glyph set, if a border is enabled.
    pub const fn border_kind(&self) -> Option<Border> {
        self.border
    }

    /// Returns whether the top border edge is enabled.
    pub const fn is_border_top_enabled(&self) -> bool {
        self.border_top
    }

    /// Returns whether the right border edge is enabled.
    pub const fn is_border_right_enabled(&self) -> bool {
        self.border_right
    }

    /// Returns whether the bottom border edge is enabled.
    pub const fn is_border_bottom_enabled(&self) -> bool {
        self.border_bottom
    }

    /// Returns whether the left border edge is enabled.
    pub const fn is_border_left_enabled(&self) -> bool {
        self.border_left
    }

    /// Returns the border foreground color instruction.
    pub const fn border_foreground_color(&self) -> Option<Color> {
        self.border_fg
    }

    /// Returns the border background color instruction.
    pub const fn border_background_color(&self) -> Option<Color> {
        self.border_bg
    }

    /// Returns the fixed content-box width, if one is set.
    pub const fn fixed_width(&self) -> Option<u16> {
        self.width
    }

    /// Returns the minimum padded content-box height, if one is set.
    pub const fn fixed_height(&self) -> Option<u16> {
        self.height
    }

    /// Returns the maximum final block width, if one is set.
    pub const fn maximum_width(&self) -> Option<u16> {
        self.max_width
    }

    /// Returns the maximum final block height, if one is set.
    pub const fn maximum_height(&self) -> Option<u16> {
        self.max_height
    }

    /// Returns the horizontal alignment within the content box.
    pub const fn horizontal_alignment(&self) -> Align {
        self.align
    }

    /// Returns the vertical alignment within the content box.
    pub const fn vertical_alignment(&self) -> VerticalAlign {
        self.vertical_align
    }

    /// The style drawn on this block's border glyphs.
    pub(crate) fn border_style(&self) -> TextStyle {
        let mut style = TextStyle::new();
        if let Some(color) = self.border_fg {
            style = style.foreground(color);
        }
        if let Some(color) = self.border_bg {
            style = style.background(color);
        }
        style
    }

    /// Replaces every color property while preserving the rest of the style.
    pub(crate) fn map_colors(mut self, map: impl Fn(Color) -> Color) -> Self {
        self.text = self.text.map_colors(&map);
        self.border_fg = self.border_fg.map(&map);
        self.border_bg = self.border_bg.map(&map);
        self
    }

    /// Removes fill and border colors while preserving the box model and text
    /// modifiers.
    pub(crate) fn without_colors(mut self) -> Self {
        self.text = self.text.without_colors();
        self.border_fg = None;
        self.border_bg = None;
        self
    }

    /// Removes every property that can emit an SGR sequence while preserving
    /// the box model.
    pub(crate) fn without_ansi(mut self) -> Self {
        self = self.without_colors();
        self.text = TextStyle::new();
        self
    }

    /// Renders plain-text `content` as a rectangle.
    ///
    /// This is the single-block case of the one layout pass: it resolves
    /// `Block(self, Text(content, self.text))` with unbounded limits. The
    /// returned block contains no trailing newline; rows are joined with `\n`.
    ///
    /// `content` is plain text. Escape sequences in it are measured as ordinary
    /// graphemes, so a block containing them comes out deterministically too
    /// wide; adopt already-rendered output with
    /// [`RenderedBlock::from_ansi`](crate::RenderedBlock::from_ansi) instead.
    pub fn render(&self, content: &str) -> RenderedBlock {
        let view = View::block(self.clone(), View::text(content, self.text.clone()));
        resolve(&view, Limits::NONE).into_rendered_block()
    }
}
