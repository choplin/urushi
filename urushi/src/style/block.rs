//! The [`BlockStyle`] builder: a rectangle, and the style filling the geometry
//! it creates.

use crate::view::Size;
use crate::{
    Align, Border, Color, Hyperlink, Length, Modifier, Overflow, Sides, TextStyle, Underline,
    UnderlineStyle, VerticalAlign,
};

/// A rectangle: padding, margin, border, dimensions, alignment, and the
/// [`TextStyle`] that fills the geometry they create.
///
/// A `BlockStyle` is an immutable value, like [`TextStyle`]. Its text properties —
/// colors and modifiers — are the style of the block's own fill: padding rows,
/// alignment gaps, and text explicitly built from [`BlockStyle::text`]. A block's style
/// does not flow into a child view; each child carries its own complete value.
///
/// ```
/// use urushi::{BlockStyle, Border, TextStyle, View};
///
/// let panel = BlockStyle::new().border(Border::ROUNDED).padding((0, 1));
/// let view = View::block(panel, View::text("こんにちは, urushi!", TextStyle::new()));
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
    border_text: TextStyle,
    border_fg: Option<Color>,
    border_bg: Option<Color>,
    width: Option<Length>,
    height: Option<Length>,
    min_width: Option<u16>,
    min_height: Option<u16>,
    max_width: Option<u16>,
    max_height: Option<u16>,
    overflow: Overflow,
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
            border_text: TextStyle::new(),
            border_fg: None,
            border_bg: None,
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            overflow: Overflow::default(),
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
    pub fn foreground(mut self, color: impl Into<Color>) -> Self {
        self.text = self.text.foreground(color);
        self
    }

    /// Returns the fill foreground color to the terminal default.
    pub fn without_foreground(mut self) -> Self {
        self.text = self.text.without_foreground();
        self
    }

    /// Sets the fill (and text) background color.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.text = self.text.background(color);
        self
    }

    /// Returns the fill background color to the terminal default.
    pub fn without_background(mut self) -> Self {
        self.text = self.text.without_background();
        self
    }

    /// Adds every flag in `modifier` to the active fill text modifiers.
    pub fn add_modifier(mut self, modifier: Modifier) -> Self {
        self.text = self.text.add_modifier(modifier);
        self
    }

    /// Removes every flag in `modifier` from the active fill text modifiers.
    pub fn remove_modifier(mut self, modifier: Modifier) -> Self {
        self.text = self.text.remove_modifier(modifier);
        self
    }

    pub fn bold(self) -> Self {
        self.add_modifier(Modifier::BOLD)
    }

    pub fn dim(self) -> Self {
        self.add_modifier(Modifier::DIM)
    }

    pub fn italic(self) -> Self {
        self.add_modifier(Modifier::ITALIC)
    }

    /// Underlines the fill text with a single line in the foreground color.
    pub fn underline(mut self) -> Self {
        self.text = self.text.underline();
        self
    }

    /// Sets the shape the fill text's underline is drawn with, adding an
    /// underline in the foreground color when the style has none.
    pub fn underline_style(mut self, style: UnderlineStyle) -> Self {
        self.text = self.text.underline_style(style);
        self
    }

    /// Sets the color the fill text's underline is drawn in, adding a single
    /// underline when the style has none.
    pub fn underline_color(mut self, color: impl Into<Color>) -> Self {
        self.text = self.text.underline_color(color);
        self
    }

    /// Replaces the complete fill text underline value.
    pub fn with_underline(mut self, underline: Underline) -> Self {
        self.text = self.text.with_underline(underline);
        self
    }

    /// Removes the fill text underline, including its color.
    pub fn without_underline(mut self) -> Self {
        self.text = self.text.without_underline();
        self
    }

    /// Attaches an OSC 8 hyperlink to the fill text.
    pub fn hyperlink(mut self, hyperlink: impl Into<Hyperlink>) -> Self {
        self.text = self.text.hyperlink(hyperlink);
        self
    }

    /// Removes the OSC 8 hyperlink from the fill text.
    pub fn without_hyperlink(mut self) -> Self {
        self.text = self.text.without_hyperlink();
        self
    }

    pub fn blink(self) -> Self {
        self.add_modifier(Modifier::SLOW_BLINK)
    }

    pub fn reverse(self) -> Self {
        self.add_modifier(Modifier::REVERSED)
    }

    pub fn hide(self) -> Self {
        self.add_modifier(Modifier::HIDDEN)
    }

    pub fn strikethrough(self) -> Self {
        self.add_modifier(Modifier::CROSSED_OUT)
    }

    /// Sets padding between the content and the border.
    pub fn padding(mut self, sides: impl Into<Sides>) -> Self {
        self.padding = sides.into();
        self
    }

    /// Sets unstyled spacing outside the border.
    pub fn margin(mut self, sides: impl Into<Sides>) -> Self {
        self.margin = sides.into();
        self
    }

    /// Sets the border glyphs around the padded content.
    ///
    /// A new style enables all four edges. Use the `border_*` builders to
    /// configure edge visibility independently.
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    /// Removes the border glyph set. Edge configuration is retained.
    pub fn without_border(mut self) -> Self {
        self.border = None;
        self
    }

    /// Enables or disables the top border edge.
    pub fn border_top(mut self, enabled: bool) -> Self {
        self.border_top = enabled;
        self
    }

    /// Enables or disables the right border edge.
    pub fn border_right(mut self, enabled: bool) -> Self {
        self.border_right = enabled;
        self
    }

    /// Enables or disables the bottom border edge.
    pub fn border_bottom(mut self, enabled: bool) -> Self {
        self.border_bottom = enabled;
        self
    }

    /// Enables or disables the left border edge.
    pub fn border_left(mut self, enabled: bool) -> Self {
        self.border_left = enabled;
        self
    }

    /// Replaces the complete logical style used for border glyphs.
    ///
    /// A subsequently applied border foreground or background overrides the
    /// corresponding property in this style.
    pub fn border_text_style(mut self, style: TextStyle) -> Self {
        self.border_text = style;
        self
    }

    /// Sets the border foreground color.
    pub fn border_foreground(mut self, color: impl Into<Color>) -> Self {
        self.border_fg = Some(color.into());
        self
    }

    /// Removes the border foreground override.
    pub fn without_border_foreground(mut self) -> Self {
        self.border_fg = None;
        self
    }

    /// Sets the border background color.
    pub fn border_background(mut self, color: impl Into<Color>) -> Self {
        self.border_bg = Some(color.into());
        self
    }

    /// Removes the border background override.
    pub fn without_border_background(mut self) -> Self {
        self.border_bg = None;
        self
    }

    /// Sets the width of the box: content plus padding plus enabled border
    /// edges, with margin outside it.
    ///
    /// The absence of a width means auto — the content's own width. Content
    /// wider than the resolved box is absorbed by [`BlockStyle::overflow`];
    /// the frame closes at the resolved width either way.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = Some(width.into());
        self
    }

    /// Returns the width to auto sizing.
    pub fn auto_width(mut self) -> Self {
        self.width = None;
        self
    }

    /// Sets the height of the box: content plus padding plus enabled border
    /// edges, with margin outside it.
    ///
    /// This is a size, not a minimum: taller content is clipped inside the
    /// frame rather than growing the box. The absence of a height means auto.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = Some(height.into());
        self
    }

    /// Returns the height to auto sizing.
    pub fn auto_height(mut self) -> Self {
        self.height = None;
        self
    }

    /// Sets the width below which the box does not shrink.
    pub fn min_width(mut self, width: u16) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Removes the minimum width.
    pub fn without_min_width(mut self) -> Self {
        self.min_width = None;
        self
    }

    /// Sets the height below which the box does not shrink.
    pub fn min_height(mut self, height: u16) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Removes the minimum height.
    pub fn without_min_height(mut self) -> Self {
        self.min_height = None;
        self
    }

    /// Bounds the box's width. The box shrinks to fit its content and never
    /// exceeds this bound; the bound never cuts the frame.
    pub fn max_width(mut self, width: u16) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Removes the maximum width.
    pub fn without_max_width(mut self) -> Self {
        self.max_width = None;
        self
    }

    /// Bounds the box's height. The box shrinks to fit its content and never
    /// exceeds this bound; the bound never cuts the frame.
    pub fn max_height(mut self, height: u16) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Removes the maximum height.
    pub fn without_max_height(mut self) -> Self {
        self.max_height = None;
        self
    }

    /// Sets how content wider than the box is absorbed.
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Sets the horizontal alignment of content within the box.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Sets the vertical alignment of content within a fixed-height box.
    pub fn align_vertical(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    /// Returns the fill foreground color instruction, if one is set.
    pub const fn foreground_color(&self) -> Option<Color> {
        self.text.foreground_color()
    }

    /// Returns the fill background color instruction, if one is set.
    pub const fn background_color(&self) -> Option<Color> {
        self.text.background_color()
    }

    /// Returns the fill text's underline instruction, if one is set.
    pub const fn underline_value(&self) -> Option<Underline> {
        self.text.underline_value()
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

    /// Returns the complete logical style used as the border-style base.
    pub const fn border_text(&self) -> &TextStyle {
        &self.border_text
    }

    /// Returns the border foreground color instruction.
    pub const fn border_foreground_color(&self) -> Option<Color> {
        self.border_fg
    }

    /// Returns the border background color instruction.
    pub const fn border_background_color(&self) -> Option<Color> {
        self.border_bg
    }

    /// Returns the box's width, if one is set.
    pub const fn width_length(&self) -> Option<Length> {
        self.width
    }

    /// Returns the box's height, if one is set.
    pub const fn height_length(&self) -> Option<Length> {
        self.height
    }

    /// Returns the box's minimum width, if one is set.
    pub const fn minimum_width(&self) -> Option<u16> {
        self.min_width
    }

    /// Returns the box's minimum height, if one is set.
    pub const fn minimum_height(&self) -> Option<u16> {
        self.min_height
    }

    /// Returns the box's maximum width, if one is set.
    pub const fn maximum_width(&self) -> Option<u16> {
        self.max_width
    }

    /// Returns the box's maximum height, if one is set.
    pub const fn maximum_height(&self) -> Option<u16> {
        self.max_height
    }

    /// Returns how content wider than the box is absorbed.
    pub const fn overflow_policy(&self) -> &Overflow {
        &self.overflow
    }

    /// Returns the per-axis overhead of enabled border edges plus padding.
    ///
    /// This is the conversion between the box a dimension measures and the
    /// content area inside it: outer minus `frame_size()` is the content area.
    /// Margin lies outside the box and keeps [`BlockStyle::margin_sides`].
    pub fn frame_size(&self) -> Size {
        let padding = self.padding;
        let (left, right, top, bottom) = match self.border {
            Some(_) => (
                usize::from(self.border_left),
                usize::from(self.border_right),
                usize::from(self.border_top),
                usize::from(self.border_bottom),
            ),
            None => (0, 0, 0, 0),
        };
        Size::new(
            left + right + usize::from(padding.left) + usize::from(padding.right),
            top + bottom + usize::from(padding.top) + usize::from(padding.bottom),
        )
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
        let mut style = self.border_text.clone();
        if let Some(color) = self.border_fg {
            style = style.foreground(color);
        }
        if let Some(color) = self.border_bg {
            style = style.background(color);
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_operations_remove_optional_values() {
        let style = BlockStyle::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .underline_color(Color::GREEN)
            .hyperlink("https://example.com")
            .border(Border::ROUNDED)
            .border_foreground(Color::CYAN)
            .border_background(Color::BLACK)
            .width(20)
            .height(4)
            .min_width(8)
            .min_height(2)
            .max_width(30)
            .max_height(6)
            .without_foreground()
            .without_background()
            .without_underline()
            .without_hyperlink()
            .without_border()
            .without_border_foreground()
            .without_border_background()
            .auto_width()
            .auto_height()
            .without_min_width()
            .without_min_height()
            .without_max_width()
            .without_max_height();

        assert_eq!(style, BlockStyle::new());
    }

    #[test]
    fn modifier_operations_accept_sets() {
        let modifiers = Modifier::BOLD | Modifier::ITALIC;
        let style = BlockStyle::new()
            .add_modifier(modifiers)
            .remove_modifier(Modifier::ITALIC);

        assert_eq!(style.modifiers(), Modifier::BOLD);
    }
}
