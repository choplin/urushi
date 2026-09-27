//! The [`BlockStyle`] builder: a rectangle, and the style filling the geometry
//! it creates.

use crate::view::Size;
use crate::{
    Align, Border, Color, Hyperlink, Length, Overflow, Sides, TextAttribute, TextAttributes,
    TextStyle, Underline, UnderlineStyle, VerticalAlign,
};

/// A rectangle: padding, margin, border, dimensions, alignment, and the
/// [`TextStyle`] that fills the geometry they create.
///
/// A `BlockStyle` is an immutable value, like [`TextStyle`]. Its text properties —
/// colors and attributes — are the style of the block's own fill: padding rows,
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
    text_style: TextStyle,
    padding: Sides,
    margin: Sides,
    border: Option<Border>,
    border_top: bool,
    border_right: bool,
    border_bottom: bool,
    border_left: bool,
    border_text_style: TextStyle,
    border_foreground: Option<Color>,
    border_background: Option<Color>,
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
            text_style: TextStyle::new(),
            padding: Sides::default(),
            margin: Sides::default(),
            border: None,
            border_top: true,
            border_right: true,
            border_bottom: true,
            border_left: true,
            border_text_style: TextStyle::new(),
            border_foreground: None,
            border_background: None,
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
    pub fn from_text_style(text_style: TextStyle) -> Self {
        Self {
            text_style,
            ..Self::default()
        }
    }

    /// Returns the style filling this block.
    pub const fn text_style(&self) -> &TextStyle {
        &self.text_style
    }

    /// Sets the fill (and text) foreground color.
    pub fn foreground(mut self, color: impl Into<Color>) -> Self {
        self.text_style = self.text_style.foreground(color);
        self
    }

    /// Returns the fill foreground color to the terminal default.
    pub fn reset_foreground(mut self) -> Self {
        self.text_style = self.text_style.reset_foreground();
        self
    }

    /// Sets the fill (and text) background color.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.text_style = self.text_style.background(color);
        self
    }

    /// Returns the fill background color to the terminal default.
    pub fn reset_background(mut self) -> Self {
        self.text_style = self.text_style.reset_background();
        self
    }

    /// Adds one active fill text attribute.
    pub fn add_attribute(mut self, attribute: TextAttribute) -> Self {
        self.text_style = self.text_style.add_attribute(attribute);
        self
    }

    /// Adds a set of active fill text attributes.
    pub fn add_attributes(mut self, attributes: TextAttributes) -> Self {
        self.text_style = self.text_style.add_attributes(attributes);
        self
    }

    /// Removes one active fill text attribute.
    pub fn remove_attribute(mut self, attribute: TextAttribute) -> Self {
        self.text_style = self.text_style.remove_attribute(attribute);
        self
    }

    /// Removes a set of active fill text attributes.
    pub fn remove_attributes(mut self, attributes: TextAttributes) -> Self {
        self.text_style = self.text_style.remove_attributes(attributes);
        self
    }

    /// Restores the active fill text attributes to their default value.
    pub fn reset_attributes(mut self) -> Self {
        self.text_style = self.text_style.reset_attributes();
        self
    }

    pub fn bold(self) -> Self {
        self.add_attribute(TextAttribute::Bold)
    }

    pub fn dim(self) -> Self {
        self.add_attribute(TextAttribute::Dim)
    }

    pub fn italic(self) -> Self {
        self.add_attribute(TextAttribute::Italic)
    }

    /// Underlines the fill text with a single line in the foreground color.
    pub fn underlined(mut self) -> Self {
        self.text_style = self.text_style.underlined();
        self
    }

    /// Sets the shape the fill text's underline is drawn with, adding an
    /// underline in the foreground color when the style has none.
    pub fn underline_style(mut self, style: UnderlineStyle) -> Self {
        self.text_style = self.text_style.underline_style(style);
        self
    }

    /// Sets the color the fill text's underline is drawn in, adding a single
    /// underline when the style has none.
    pub fn underline_color(mut self, color: impl Into<Color>) -> Self {
        self.text_style = self.text_style.underline_color(color);
        self
    }

    /// Replaces the complete fill text underline value.
    pub fn underline(mut self, underline: Underline) -> Self {
        self.text_style = self.text_style.underline(underline);
        self
    }

    /// Removes the fill text underline, including its color.
    pub fn reset_underline(mut self) -> Self {
        self.text_style = self.text_style.reset_underline();
        self
    }

    /// Attaches an OSC 8 hyperlink to the fill text.
    pub fn hyperlink(mut self, hyperlink: impl Into<Hyperlink>) -> Self {
        self.text_style = self.text_style.hyperlink(hyperlink);
        self
    }

    /// Removes the OSC 8 hyperlink from the fill text.
    pub fn reset_hyperlink(mut self) -> Self {
        self.text_style = self.text_style.reset_hyperlink();
        self
    }

    pub fn blink(self) -> Self {
        self.add_attribute(TextAttribute::SlowBlink)
    }

    pub fn reverse(self) -> Self {
        self.add_attribute(TextAttribute::Reversed)
    }

    pub fn hide(self) -> Self {
        self.add_attribute(TextAttribute::Hidden)
    }

    pub fn strikethrough(self) -> Self {
        self.add_attribute(TextAttribute::CrossedOut)
    }

    /// Sets padding between the content and the border.
    pub fn padding(mut self, sides: impl Into<Sides>) -> Self {
        self.padding = sides.into();
        self
    }

    /// Restores the block padding to its default value.
    pub fn reset_padding(mut self) -> Self {
        self.padding = Sides::default();
        self
    }

    /// Sets unstyled spacing outside the border.
    pub fn margin(mut self, sides: impl Into<Sides>) -> Self {
        self.margin = sides.into();
        self
    }

    /// Restores the block margin to its default value.
    pub fn reset_margin(mut self) -> Self {
        self.margin = Sides::default();
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
    pub fn reset_border(mut self) -> Self {
        self.border = None;
        self
    }

    /// Enables or disables the top border edge.
    pub fn border_top(mut self, enabled: bool) -> Self {
        self.border_top = enabled;
        self
    }

    /// Restores top-edge visibility to its default value.
    pub fn reset_border_top(mut self) -> Self {
        self.border_top = true;
        self
    }

    /// Enables or disables the right border edge.
    pub fn border_right(mut self, enabled: bool) -> Self {
        self.border_right = enabled;
        self
    }

    /// Restores right-edge visibility to its default value.
    pub fn reset_border_right(mut self) -> Self {
        self.border_right = true;
        self
    }

    /// Enables or disables the bottom border edge.
    pub fn border_bottom(mut self, enabled: bool) -> Self {
        self.border_bottom = enabled;
        self
    }

    /// Restores bottom-edge visibility to its default value.
    pub fn reset_border_bottom(mut self) -> Self {
        self.border_bottom = true;
        self
    }

    /// Enables or disables the left border edge.
    pub fn border_left(mut self, enabled: bool) -> Self {
        self.border_left = enabled;
        self
    }

    /// Restores left-edge visibility to its default value.
    pub fn reset_border_left(mut self) -> Self {
        self.border_left = true;
        self
    }

    /// Replaces the complete logical style used for border glyphs.
    ///
    /// A subsequently applied border foreground or background overrides the
    /// corresponding property in this style.
    pub fn border_text_style(mut self, style: TextStyle) -> Self {
        self.border_text_style = style;
        self
    }

    /// Restores the complete logical border-glyph style to its default value.
    pub fn reset_border_text_style(mut self) -> Self {
        self.border_text_style = TextStyle::default();
        self
    }

    /// Sets the border foreground color.
    pub fn border_foreground(mut self, color: impl Into<Color>) -> Self {
        self.border_foreground = Some(color.into());
        self
    }

    /// Removes the border foreground override.
    pub fn reset_border_foreground(mut self) -> Self {
        self.border_foreground = None;
        self
    }

    /// Sets the border background color.
    pub fn border_background(mut self, color: impl Into<Color>) -> Self {
        self.border_background = Some(color.into());
        self
    }

    /// Removes the border background override.
    pub fn reset_border_background(mut self) -> Self {
        self.border_background = None;
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
    pub fn reset_width(mut self) -> Self {
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
    pub fn reset_height(mut self) -> Self {
        self.height = None;
        self
    }

    /// Sets the width below which the box does not shrink.
    pub fn min_width(mut self, width: u16) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Removes the minimum width.
    pub fn reset_min_width(mut self) -> Self {
        self.min_width = None;
        self
    }

    /// Sets the height below which the box does not shrink.
    pub fn min_height(mut self, height: u16) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Removes the minimum height.
    pub fn reset_min_height(mut self) -> Self {
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
    pub fn reset_max_width(mut self) -> Self {
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
    pub fn reset_max_height(mut self) -> Self {
        self.max_height = None;
        self
    }

    /// Sets how content wider than the box is absorbed.
    pub fn overflow(mut self, overflow: Overflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Restores the overflow policy to its default value.
    pub fn reset_overflow(mut self) -> Self {
        self.overflow = Overflow::default();
        self
    }

    /// Sets the horizontal alignment of content within the box.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    /// Restores horizontal alignment to its default value.
    pub fn reset_align(mut self) -> Self {
        self.align = Align::default();
        self
    }

    /// Sets the vertical alignment of content within a fixed-height box.
    pub fn vertical_align(mut self, align: VerticalAlign) -> Self {
        self.vertical_align = align;
        self
    }

    /// Restores vertical alignment to its default value.
    pub fn reset_vertical_align(mut self) -> Self {
        self.vertical_align = VerticalAlign::default();
        self
    }

    /// Returns the fill foreground color instruction, if one is set.
    pub const fn get_foreground(&self) -> Option<Color> {
        self.text_style.get_foreground()
    }

    /// Returns the fill background color instruction, if one is set.
    pub const fn get_background(&self) -> Option<Color> {
        self.text_style.get_background()
    }

    /// Returns the fill text's underline instruction, if one is set.
    pub const fn get_underline(&self) -> Option<Underline> {
        self.text_style.get_underline()
    }

    /// Returns the active fill text attributes.
    pub const fn get_attributes(&self) -> TextAttributes {
        self.text_style.get_attributes()
    }

    /// Returns the padding applied inside the border.
    pub const fn get_padding(&self) -> Sides {
        self.padding
    }

    /// Returns the unstyled margin applied outside the border.
    pub const fn get_margin(&self) -> Sides {
        self.margin
    }

    /// Returns the border glyph set, if a border is enabled.
    pub const fn get_border(&self) -> Option<Border> {
        self.border
    }

    /// Returns whether the top border edge is enabled.
    pub const fn get_border_top(&self) -> bool {
        self.border_top
    }

    /// Returns whether the right border edge is enabled.
    pub const fn get_border_right(&self) -> bool {
        self.border_right
    }

    /// Returns whether the bottom border edge is enabled.
    pub const fn get_border_bottom(&self) -> bool {
        self.border_bottom
    }

    /// Returns whether the left border edge is enabled.
    pub const fn get_border_left(&self) -> bool {
        self.border_left
    }

    /// Returns the complete logical style used as the border-style base.
    pub const fn get_border_text_style(&self) -> &TextStyle {
        &self.border_text_style
    }

    /// Returns the border foreground color instruction.
    pub const fn get_border_foreground(&self) -> Option<Color> {
        self.border_foreground
    }

    /// Returns the border background color instruction.
    pub const fn get_border_background(&self) -> Option<Color> {
        self.border_background
    }

    /// Returns the box's width, if one is set.
    pub const fn get_width(&self) -> Option<Length> {
        self.width
    }

    /// Returns the box's height, if one is set.
    pub const fn get_height(&self) -> Option<Length> {
        self.height
    }

    /// Returns the box's minimum width, if one is set.
    pub const fn get_min_width(&self) -> Option<u16> {
        self.min_width
    }

    /// Returns the box's minimum height, if one is set.
    pub const fn get_min_height(&self) -> Option<u16> {
        self.min_height
    }

    /// Returns the box's maximum width, if one is set.
    pub const fn get_max_width(&self) -> Option<u16> {
        self.max_width
    }

    /// Returns the box's maximum height, if one is set.
    pub const fn get_max_height(&self) -> Option<u16> {
        self.max_height
    }

    /// Returns how content wider than the box is absorbed.
    pub const fn get_overflow(&self) -> &Overflow {
        &self.overflow
    }

    /// Returns the per-axis overhead of enabled border edges plus padding.
    ///
    /// This is the conversion between the box a dimension measures and the
    /// content area inside it: outer minus `frame_size()` is the content area.
    /// Margin lies outside the box and keeps [`BlockStyle::get_margin`].
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
    pub const fn get_align(&self) -> Align {
        self.align
    }

    /// Returns the vertical alignment within the content box.
    pub const fn get_vertical_align(&self) -> VerticalAlign {
        self.vertical_align
    }

    /// The style drawn on this block's border glyphs.
    pub(crate) fn border_style(&self) -> TextStyle {
        let mut style = self.border_text_style.clone();
        if let Some(color) = self.border_foreground {
            style = style.foreground(color);
        }
        if let Some(color) = self.border_background {
            style = style.background(color);
        }
        style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_builders_restore_every_property_default() {
        let style = BlockStyle::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .add_attribute(TextAttribute::Italic)
            .underline_color(Color::GREEN)
            .hyperlink("https://example.com")
            .padding(1)
            .margin(2)
            .border(Border::ROUNDED)
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_left(false)
            .border_text_style(TextStyle::new().bold())
            .border_foreground(Color::CYAN)
            .border_background(Color::BLACK)
            .width(20)
            .height(4)
            .min_width(8)
            .min_height(2)
            .max_width(30)
            .max_height(6)
            .overflow(Overflow::ellipsis())
            .align(Align::Center)
            .vertical_align(VerticalAlign::Bottom)
            .reset_foreground()
            .reset_background()
            .reset_attributes()
            .reset_underline()
            .reset_hyperlink()
            .reset_padding()
            .reset_margin()
            .reset_border()
            .reset_border_top()
            .reset_border_right()
            .reset_border_bottom()
            .reset_border_left()
            .reset_border_text_style()
            .reset_border_foreground()
            .reset_border_background()
            .reset_width()
            .reset_height()
            .reset_min_width()
            .reset_min_height()
            .reset_max_width()
            .reset_max_height()
            .reset_overflow()
            .reset_align()
            .reset_vertical_align();

        assert_eq!(style, BlockStyle::new());
    }

    #[test]
    fn attribute_operations_accept_sets() {
        let attributes = TextAttribute::Bold | TextAttribute::Italic;
        let style = BlockStyle::new()
            .add_attributes(attributes)
            .remove_attribute(TextAttribute::Italic);

        assert_eq!(style.get_attributes(), TextAttribute::Bold.into());
    }
}
