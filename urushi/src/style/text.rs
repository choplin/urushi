//! The [`TextStyle`] builder: everything a terminal can express about a run of
//! text.

use crate::{Color, Modifier, TextStyleProperty, TextStylePropertyKey};

pub(crate) const RESET: &str = "\x1b[0m";

/// A reusable set of text styling rules.
///
/// A `TextStyle` carries no geometry. A position that renders inline text cannot
/// honor padding, a border, or a dimension, so those properties live on
/// [`BlockStyle`](crate::BlockStyle) instead and the illegal combination is
/// unrepresentable rather than merely discouraged.
///
/// A `TextStyle` is an immutable value: builder methods consume and return it, so
/// styles can be stored, cloned, and extended without affecting each other.
///
/// ```
/// use urushi::{Color, TextStyle};
///
/// let base = TextStyle::new().foreground(Color::CYAN);
/// let emphasized = base.clone().bold();
///
/// println!("{}", emphasized.paint("hello"));
/// ```
///
/// Geometry is not merely discouraged here, it is unrepresentable:
///
/// ```compile_fail
/// use urushi::{Border, TextStyle};
///
/// let _ = TextStyle::new().border(Border::ROUNDED);
/// ```
///
/// ```compile_fail
/// use urushi::{TextStyle, TextStyleProperty};
///
/// let _ = TextStyle::new().add(TextStyleProperty::Width(10));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextStyle {
    fg: Option<Color>,
    bg: Option<Color>,
    modifiers: Modifier,
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds or replaces a property in this style.
    // This is the collection operation paired with `remove`, not arithmetic.
    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, property: impl Into<TextStyleProperty>) -> Self {
        match property.into() {
            TextStyleProperty::Foreground(color) => self.fg = Some(color),
            TextStyleProperty::Background(color) => self.bg = Some(color),
            TextStyleProperty::Modifier(modifier) => {
                self.modifiers = self.modifiers.union(modifier);
            }
        }
        self
    }

    /// Removes a property from this style, restoring its default value.
    pub fn remove(mut self, property: impl Into<TextStylePropertyKey>) -> Self {
        match property.into() {
            TextStylePropertyKey::Foreground => self.fg = None,
            TextStylePropertyKey::Background => self.bg = None,
            TextStylePropertyKey::Modifier(modifier) => {
                self.modifiers = self.modifiers.difference(modifier);
            }
        }
        self
    }

    /// Sets the text foreground color.
    pub fn foreground(self, color: impl Into<Color>) -> Self {
        self.add(TextStyleProperty::Foreground(color.into()))
    }

    /// Sets the text background color.
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

    /// Wraps `text` in this style's SGR scope.
    ///
    /// A style that emits no sequence returns `text` unchanged. This produces
    /// no rectangle: padding, borders, and dimensions belong to
    /// [`BlockStyle`](crate::BlockStyle).
    pub fn paint(&self, text: &str) -> String {
        let sgr = self.sgr_prefix();
        if sgr.is_empty() {
            return text.to_owned();
        }
        format!("{sgr}{text}{RESET}")
    }

    /// Replaces every color property while preserving the rest of the style.
    pub(crate) fn map_colors(mut self, map: impl Fn(Color) -> Color) -> Self {
        self.fg = self.fg.map(&map);
        self.bg = self.bg.map(&map);
        self
    }

    /// Removes foreground and background colors while preserving modifiers.
    pub(crate) fn without_colors(mut self) -> Self {
        self.fg = None;
        self.bg = None;
        self
    }

    /// The SGR sequence enabling this style's modifiers and colors, or an
    /// empty string when the style sets none of them.
    pub(crate) fn sgr_prefix(&self) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_and_named_operations_share_value_semantics() {
        let style = TextStyle::new()
            .bold()
            .add(Modifier::ITALIC)
            .add(TextStyleProperty::Foreground(Color::CYAN))
            .remove(Modifier::ITALIC)
            .remove(TextStylePropertyKey::Foreground);

        assert_eq!(style.modifiers(), Modifier::BOLD);
        assert_eq!(style.foreground_color(), None);
    }

    #[test]
    fn removing_a_modifier_removes_it_from_painted_value() {
        let style = TextStyle::new().bold().dim().remove(Modifier::BOLD);

        assert_eq!(style.paint("text"), "\x1b[2mtext\x1b[0m");
        assert_eq!(
            TextStyle::new().remove(Modifier::all()).paint("text"),
            "text"
        );
    }

    #[test]
    fn singleton_properties_replace_and_remove_to_defaults() {
        let style = TextStyle::new()
            .add(TextStyleProperty::Foreground(Color::RED))
            .add(TextStyleProperty::Foreground(Color::BLUE))
            .background(Color::GREEN)
            .remove(TextStylePropertyKey::Background);

        assert_eq!(style.foreground_color(), Some(Color::BLUE));
        assert_eq!(style.background_color(), None);
    }
}
