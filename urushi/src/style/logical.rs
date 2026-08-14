//! The [`Style`] builder and its string renderer.

use crate::text::{visible_width, wrap_text};
use crate::{Align, Border, Color, Modifier, Sides, StyleProperty, StylePropertyKey};

const RESET: &str = "\x1b[0m";

/// A reusable set of styling rules that renders text into an ANSI string.
///
/// A `Style` is an immutable value: builder methods consume and return it, so
/// styles can be stored, cloned, and extended without affecting each other.
///
/// ```
/// use urushi::{Border, Color, Style};
///
/// let base = Style::new().foreground(Color::CYAN);
/// let boxed = base.clone().border(Border::ROUNDED).padding((0, 1));
///
/// println!("{}", boxed.render("hello"));
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    modifiers: Modifier,
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
    align: Align,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            fg: None,
            bg: None,
            modifiers: Modifier::empty(),
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
            align: Align::default(),
        }
    }
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a property in this style.
    // This is the collection operation paired with `remove`, not arithmetic.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, property: impl Into<StyleProperty>) -> Self {
        match property.into() {
            StyleProperty::Foreground(color) => self.fg = Some(color),
            StyleProperty::Background(color) => self.bg = Some(color),
            StyleProperty::Modifier(modifier) => {
                self.modifiers = self.modifiers.union(modifier);
            }
            StyleProperty::Padding(sides) => self.padding = sides,
            StyleProperty::Margin(sides) => self.margin = sides,
            StyleProperty::Border(border) => self.border = Some(border),
            StyleProperty::BorderTop(enabled) => self.border_top = enabled,
            StyleProperty::BorderRight(enabled) => self.border_right = enabled,
            StyleProperty::BorderBottom(enabled) => self.border_bottom = enabled,
            StyleProperty::BorderLeft(enabled) => self.border_left = enabled,
            StyleProperty::BorderForeground(color) => self.border_fg = Some(color),
            StyleProperty::BorderBackground(color) => self.border_bg = Some(color),
            StyleProperty::Width(width) => self.width = Some(width),
            StyleProperty::Height(height) => self.height = Some(height),
            StyleProperty::Align(align) => self.align = align,
        }
        self
    }

    /// Removes a property from this style, restoring its default value.
    pub fn remove(mut self, property: impl Into<StylePropertyKey>) -> Self {
        match property.into() {
            StylePropertyKey::Foreground => self.fg = None,
            StylePropertyKey::Background => self.bg = None,
            StylePropertyKey::Modifier(modifier) => {
                self.modifiers = self.modifiers.difference(modifier);
            }
            StylePropertyKey::Padding => self.padding = Sides::default(),
            StylePropertyKey::Margin => self.margin = Sides::default(),
            StylePropertyKey::Border => self.border = None,
            StylePropertyKey::BorderTop => self.border_top = true,
            StylePropertyKey::BorderRight => self.border_right = true,
            StylePropertyKey::BorderBottom => self.border_bottom = true,
            StylePropertyKey::BorderLeft => self.border_left = true,
            StylePropertyKey::BorderForeground => self.border_fg = None,
            StylePropertyKey::BorderBackground => self.border_bg = None,
            StylePropertyKey::Width => self.width = None,
            StylePropertyKey::Height => self.height = None,
            StylePropertyKey::Align => self.align = Align::default(),
        }
        self
    }

    /// Sets the text (and padding) foreground color.
    pub fn foreground(self, color: impl Into<Color>) -> Self {
        self.add(StyleProperty::Foreground(color.into()))
    }

    /// Sets the text (and padding) background color.
    pub fn background(self, color: impl Into<Color>) -> Self {
        self.add(StyleProperty::Background(color.into()))
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
        self.add(StyleProperty::Padding(sides.into()))
    }

    /// Sets unstyled spacing outside the border.
    pub fn margin(self, sides: impl Into<Sides>) -> Self {
        self.add(StyleProperty::Margin(sides.into()))
    }

    /// Sets the border glyphs around the padded content.
    ///
    /// A new style enables all four edges. Use the `border_*` builders to
    /// configure edge visibility independently.
    pub fn border(self, border: Border) -> Self {
        self.add(StyleProperty::Border(border))
    }

    /// Enables or disables the top border edge.
    pub fn border_top(self, enabled: bool) -> Self {
        self.add(StyleProperty::BorderTop(enabled))
    }

    /// Enables or disables the right border edge.
    pub fn border_right(self, enabled: bool) -> Self {
        self.add(StyleProperty::BorderRight(enabled))
    }

    /// Enables or disables the bottom border edge.
    pub fn border_bottom(self, enabled: bool) -> Self {
        self.add(StyleProperty::BorderBottom(enabled))
    }

    /// Enables or disables the left border edge.
    pub fn border_left(self, enabled: bool) -> Self {
        self.add(StyleProperty::BorderLeft(enabled))
    }

    /// Sets the border foreground color.
    pub fn border_foreground(self, color: impl Into<Color>) -> Self {
        self.add(StyleProperty::BorderForeground(color.into()))
    }

    /// Sets the border background color.
    pub fn border_background(self, color: impl Into<Color>) -> Self {
        self.add(StyleProperty::BorderBackground(color.into()))
    }

    /// Fixes the width of the padded content box (excluding border and
    /// margin). Content is word-wrapped to fit.
    pub fn width(self, width: u16) -> Self {
        self.add(StyleProperty::Width(width))
    }

    /// Sets the minimum height of the padded content box (excluding border and
    /// margin). Taller content expands the box instead of being truncated.
    pub fn height(self, height: u16) -> Self {
        self.add(StyleProperty::Height(height))
    }

    /// Sets the horizontal alignment of content within the box.
    pub fn align(self, align: Align) -> Self {
        self.add(StyleProperty::Align(align))
    }

    /// Returns the foreground color instruction, if this style sets one.
    pub const fn foreground_color(&self) -> Option<Color> {
        self.fg
    }

    /// Returns the background color instruction, if this style sets one.
    pub const fn background_color(&self) -> Option<Color> {
        self.bg
    }

    /// Returns the active text modifiers.
    pub const fn modifiers(&self) -> Modifier {
        self.modifiers
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

    /// Returns the horizontal alignment within the content box.
    pub const fn horizontal_alignment(&self) -> Align {
        self.align
    }

    /// Replaces every color property while preserving the rest of the style.
    pub(crate) fn map_colors(mut self, map: impl Fn(Color) -> Color) -> Self {
        self.fg = self.fg.map(&map);
        self.bg = self.bg.map(&map);
        self.border_fg = self.border_fg.map(&map);
        self.border_bg = self.border_bg.map(&map);
        self
    }

    /// Removes foreground, background, and border colors while preserving the
    /// box model and text modifiers.
    pub(crate) fn without_colors(mut self) -> Self {
        self.fg = None;
        self.bg = None;
        self.border_fg = None;
        self.border_bg = None;
        self
    }

    /// Removes every property that can cause this style to emit an SGR
    /// sequence while preserving its layout properties.
    pub(crate) fn without_ansi(mut self) -> Self {
        self = self.without_colors();
        self.modifiers = Modifier::empty();
        self
    }

    /// Renders `content` with this style, returning an ANSI string.
    ///
    /// The returned string contains no trailing newline; multi-line output is
    /// joined with `\n`.
    pub fn render(&self, content: &str) -> String {
        let pad = self.padding;
        let (pl, pr, pt, pb) = (
            pad.left as usize,
            pad.right as usize,
            pad.top as usize,
            pad.bottom as usize,
        );

        // 1. Wrap when an explicit width constrains the content box.
        let inner_target = self
            .width
            .map(|w| (w as usize).saturating_sub(pl + pr).max(1));
        let mut lines: Vec<String> = match inner_target {
            Some(w) => wrap_text(content, w),
            None => content.lines().map(str::to_string).collect(),
        };
        if lines.is_empty() {
            lines.push(String::new());
        }

        // 2. The content width. A wide character that cannot fit the target
        // width may still overflow it; `max` keeps the box consistent.
        let natural = lines.iter().map(|l| visible_width(l)).max().unwrap_or(0);
        let inner = inner_target.map_or(natural, |w| w.max(natural));
        let total = pl + inner + pr;

        // 3. Pad and align each line, applying the text style across the
        // full padded width so backgrounds cover the padding as well.
        let sgr = self.sgr_prefix();
        let reset = if sgr.is_empty() { "" } else { RESET };
        let blank_row = format!("{sgr}{}{reset}", " ".repeat(total));
        let natural_height = pt + lines.len() + pb;
        let target_height = self.height.map_or(natural_height, |height| {
            usize::from(height).max(natural_height)
        });
        let mut rows: Vec<String> = Vec::with_capacity(target_height);
        for _ in 0..pt {
            rows.push(blank_row.clone());
        }
        for line in &lines {
            let gap = inner.saturating_sub(visible_width(line));
            let (left, right) = match self.align {
                Align::Left => (0, gap),
                Align::Center => (gap / 2, gap - gap / 2),
                Align::Right => (gap, 0),
            };
            let line = reapply_after_reset(line, &sgr);
            rows.push(format!(
                "{sgr}{}{line}{}{reset}",
                " ".repeat(pl + left),
                " ".repeat(right + pr),
            ));
        }
        for _ in 0..pb {
            rows.push(blank_row.clone());
        }
        for _ in natural_height..target_height {
            rows.push(blank_row.clone());
        }

        // 4. Border.
        if let Some(b) = self.border {
            let bsgr = self.border_sgr_prefix();
            let breset = if bsgr.is_empty() { "" } else { RESET };
            let top: String = std::iter::repeat_n(b.top, total).collect();
            let bottom: String = std::iter::repeat_n(b.bottom, total).collect();
            let mut bordered = Vec::with_capacity(
                rows.len() + usize::from(self.border_top) + usize::from(self.border_bottom),
            );
            if self.border_top {
                let top_left = optional_border_char(self.border_left, b.top_left);
                let top_right = optional_border_char(self.border_right, b.top_right);
                let edge = format!("{top_left}{top}{top_right}");
                bordered.push(if edge.is_empty() {
                    edge
                } else {
                    format!("{bsgr}{edge}{breset}")
                });
            }
            for row in rows {
                let left = styled_border_char(self.border_left, b.left, &bsgr, breset);
                let right = styled_border_char(self.border_right, b.right, &bsgr, breset);
                bordered.push(format!("{left}{row}{right}"));
            }
            if self.border_bottom {
                let bottom_left = optional_border_char(self.border_left, b.bottom_left);
                let bottom_right = optional_border_char(self.border_right, b.bottom_right);
                let edge = format!("{bottom_left}{bottom}{bottom_right}");
                bordered.push(if edge.is_empty() {
                    edge
                } else {
                    format!("{bsgr}{edge}{breset}")
                });
            }
            rows = bordered;
        }

        // 5. Margin: plain, unstyled space outside the border.
        let m = self.margin;
        if m == Sides::default() {
            return rows.join("\n");
        }
        let outer = total
            + usize::from(self.border.is_some() && self.border_left)
            + usize::from(self.border.is_some() && self.border_right);
        let (ml, mr) = (m.left as usize, m.right as usize);
        let blank_margin = " ".repeat(ml + outer + mr);
        let mut out: Vec<String> = Vec::with_capacity(rows.len() + (m.top + m.bottom) as usize);
        for _ in 0..m.top {
            out.push(blank_margin.clone());
        }
        for row in rows {
            out.push(format!("{}{row}{}", " ".repeat(ml), " ".repeat(mr)));
        }
        for _ in 0..m.bottom {
            out.push(blank_margin.clone());
        }
        out.join("\n")
    }

    /// The SGR sequence enabling this style's modifiers and colors, or an
    /// empty string when the style sets none of them.
    fn sgr_prefix(&self) -> String {
        let mut params: Vec<String> = Vec::new();
        for (added, code) in [
            (self.modifiers.contains(Modifier::BOLD), "1"),
            (self.modifiers.contains(Modifier::DIM), "2"),
            (self.modifiers.contains(Modifier::ITALIC), "3"),
            (self.modifiers.contains(Modifier::UNDERLINED), "4"),
            (self.modifiers.contains(Modifier::SLOW_BLINK), "5"),
            (self.modifiers.contains(Modifier::REVERSED), "7"),
            (self.modifiers.contains(Modifier::CROSSED_OUT), "9"),
        ] {
            if added {
                params.push(code.to_string());
            }
        }
        if let Some(c) = self.fg {
            params.push(c.sgr_params(false));
        }
        if let Some(c) = self.bg {
            params.push(c.sgr_params(true));
        }
        if params.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", params.join(";"))
        }
    }

    fn border_sgr_prefix(&self) -> String {
        let mut params: Vec<String> = Vec::new();
        if let Some(c) = self.border_fg {
            params.push(c.sgr_params(false));
        }
        if let Some(c) = self.border_bg {
            params.push(c.sgr_params(true));
        }
        if params.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", params.join(";"))
        }
    }
}

fn optional_border_char(enabled: bool, value: char) -> String {
    if enabled {
        value.to_string()
    } else {
        String::new()
    }
}

fn styled_border_char(enabled: bool, value: char, sgr: &str, reset: &str) -> String {
    if enabled {
        format!("{sgr}{value}{reset}")
    } else {
        String::new()
    }
}

fn reapply_after_reset(content: &str, sgr: &str) -> String {
    if sgr.is_empty() || !content.contains(RESET) {
        return content.to_string();
    }

    content.replace(RESET, &format!("{RESET}{sgr}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_and_named_operations_share_value_semantics() {
        let style = Style::new()
            .bold()
            .add(Modifier::ITALIC)
            .add(StyleProperty::Foreground(Color::CYAN))
            .remove(Modifier::ITALIC)
            .remove(StylePropertyKey::Foreground);

        assert_eq!(style.modifiers(), Modifier::BOLD);
        assert_eq!(style.foreground_color(), None);
    }

    #[test]
    fn removing_a_modifier_removes_it_from_rendered_value() {
        let style = Style::new().bold().dim().remove(Modifier::BOLD);

        assert_eq!(style.render("text"), "\x1b[2mtext\x1b[0m");
        assert_eq!(Style::new().remove(Modifier::all()).render("text"), "text");
    }

    #[test]
    fn singleton_properties_replace_and_remove_to_defaults() {
        let style = Style::new()
            .add(StyleProperty::Foreground(Color::RED))
            .add(StyleProperty::Foreground(Color::BLUE))
            .background(Color::GREEN)
            .padding(1)
            .margin(2)
            .border(Border::ROUNDED)
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_left(false)
            .border_foreground(Color::CYAN)
            .border_background(Color::BLACK)
            .width(20)
            .height(10)
            .align(Align::Right)
            .remove(StylePropertyKey::Background)
            .remove(StylePropertyKey::Padding)
            .remove(StylePropertyKey::Margin)
            .remove(StylePropertyKey::Border)
            .remove(StylePropertyKey::BorderTop)
            .remove(StylePropertyKey::BorderRight)
            .remove(StylePropertyKey::BorderBottom)
            .remove(StylePropertyKey::BorderLeft)
            .remove(StylePropertyKey::BorderForeground)
            .remove(StylePropertyKey::BorderBackground)
            .remove(StylePropertyKey::Width)
            .remove(StylePropertyKey::Height)
            .remove(StylePropertyKey::Align);

        assert_eq!(style.foreground_color(), Some(Color::BLUE));
        assert_eq!(style.background_color(), None);
        assert_eq!(style.padding_sides(), Sides::default());
        assert_eq!(style.margin_sides(), Sides::default());
        assert_eq!(style.border_kind(), None);
        assert!(style.is_border_top_enabled());
        assert!(style.is_border_right_enabled());
        assert!(style.is_border_bottom_enabled());
        assert!(style.is_border_left_enabled());
        assert_eq!(style.border_foreground_color(), None);
        assert_eq!(style.border_background_color(), None);
        assert_eq!(style.fixed_width(), None);
        assert_eq!(style.fixed_height(), None);
        assert_eq!(style.horizontal_alignment(), Align::Left);
    }

    #[test]
    fn height_builder_and_generic_property_share_value_semantics() {
        let named = Style::new().height(4);
        let generic = Style::new().add(StyleProperty::Height(4));

        assert_eq!(named, generic);
        assert_eq!(named.fixed_height(), Some(4));
        assert_eq!(
            generic.remove(StylePropertyKey::Height).fixed_height(),
            None
        );
    }

    #[test]
    fn border_side_builders_and_generic_properties_share_value_semantics() {
        let named = Style::new()
            .border_top(false)
            .border_right(false)
            .border_bottom(false)
            .border_left(false);
        let generic = Style::new()
            .add(StyleProperty::BorderTop(false))
            .add(StyleProperty::BorderRight(false))
            .add(StyleProperty::BorderBottom(false))
            .add(StyleProperty::BorderLeft(false));

        assert_eq!(named, generic);
        assert!(!named.is_border_top_enabled());
        assert!(!named.is_border_right_enabled());
        assert!(!named.is_border_bottom_enabled());
        assert!(!named.is_border_left_enabled());

        let restored_top = generic
            .border(Border::ASCII)
            .remove(StylePropertyKey::BorderTop);
        assert_eq!(restored_top.render("x"), "-\nx");
    }
}
