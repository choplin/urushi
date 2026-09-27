//! Explicit feature selection for one render operation.

use crate::{TextAttributes, TextStyle, UnderlineStyleSet};
use urushi_terminal::{ColorLevel, TerminalCapabilities};

use super::palette::{quantize_to_ansi16, quantize_to_ansi256};

/// The output features selected for one render operation.
///
/// The default is deliberately dumb: no escape-sequence-producing feature is
/// selected. A terminal's detected maximum can be adopted with `From` and then
/// narrowed with the named setters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderSettings {
    color_level: ColorLevel,
    attributes: TextAttributes,
    underline_styles: UnderlineStyleSet,
    underline_colors: bool,
    hyperlinks: bool,
}

impl RenderSettings {
    /// Selects every rendering feature without degrading logical styles.
    ///
    /// This is intended for serializers whose styles have already been
    /// narrowed to the output's capabilities. Ordinary terminal output should
    /// detect capabilities and convert them with [`From`].
    pub const fn all() -> Self {
        Self {
            color_level: ColorLevel::TrueColor,
            attributes: TextAttributes::all(),
            underline_styles: UnderlineStyleSet::all(),
            underline_colors: true,
            hyperlinks: true,
        }
    }

    pub const fn get_color_level(self) -> ColorLevel {
        self.color_level
    }

    pub const fn get_attributes(self) -> TextAttributes {
        self.attributes
    }

    pub const fn get_underline_styles(self) -> UnderlineStyleSet {
        self.underline_styles
    }

    pub const fn get_underline_colors(self) -> bool {
        self.underline_colors
    }

    pub const fn get_hyperlinks(self) -> bool {
        self.hyperlinks
    }

    pub const fn color_level(mut self, color_level: ColorLevel) -> Self {
        self.color_level = color_level;
        self
    }

    pub const fn attributes(mut self, attributes: TextAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    pub const fn underline_styles(mut self, styles: UnderlineStyleSet) -> Self {
        self.underline_styles = styles;
        self
    }

    pub const fn underline_colors(mut self, enabled: bool) -> Self {
        self.underline_colors = enabled;
        self
    }

    pub const fn hyperlinks(mut self, enabled: bool) -> Self {
        self.hyperlinks = enabled;
        self
    }

    pub const fn reset_color_level(mut self) -> Self {
        self.color_level = ColorLevel::None;
        self
    }

    pub const fn reset_attributes(mut self) -> Self {
        self.attributes = TextAttributes::empty();
        self
    }

    pub const fn reset_underline_styles(mut self) -> Self {
        self.underline_styles = UnderlineStyleSet::empty();
        self
    }

    pub const fn reset_underline_colors(mut self) -> Self {
        self.underline_colors = false;
        self
    }

    pub const fn reset_hyperlinks(mut self) -> Self {
        self.hyperlinks = false;
        self
    }

    /// Applies these selected features to one logical text style.
    ///
    /// This is useful to output backends that serialize runs themselves, such
    /// as an interactive prompt renderer. Ordinary static output should call
    /// [`render`](crate::render) for the whole resolved view.
    pub fn resolve_text_style(self, style: &TextStyle) -> TextStyle {
        let mut resolved = match self.color_level {
            ColorLevel::None => style.clone().without_colors(),
            ColorLevel::Ansi16 => style.clone().map_colors(quantize_to_ansi16),
            ColorLevel::Ansi256 => style.clone().map_colors(quantize_to_ansi256),
            ColorLevel::TrueColor => style.clone(),
        };
        resolved.attributes = resolved.attributes.intersection(self.attributes);
        resolved.underline = resolved.underline.and_then(|underline| {
            self.underline_styles
                .contains(underline.get_style())
                .then_some(if self.underline_colors {
                    underline
                } else {
                    underline.reset_color()
                })
        });
        if !self.hyperlinks {
            resolved.hyperlink = None;
        }
        resolved.canonical()
    }
}

impl From<TerminalCapabilities> for RenderSettings {
    fn from(capabilities: TerminalCapabilities) -> Self {
        Self::default()
            .color_level(capabilities.color_level())
            .attributes(capabilities.attributes())
            .underline_styles(capabilities.underline_styles())
            .underline_colors(capabilities.underline_colors())
            .hyperlinks(capabilities.hyperlinks())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Hyperlink, TextAttribute, Underline, UnderlineStyle};

    #[test]
    fn default_is_plain_text() {
        let style = TextStyle::new()
            .foreground(Color::RED)
            .bold()
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::BLUE)
            .hyperlink(Hyperlink::new("https://example.com"));

        assert_eq!(
            RenderSettings::default().resolve_text_style(&style),
            TextStyle::new()
        );
    }

    #[test]
    fn settings_select_each_feature_axis_independently() {
        let style = TextStyle::new()
            .foreground(Color::Rgb(255, 0, 0))
            .bold()
            .italic()
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::BLUE)
            .hyperlink("https://example.com");
        let settings = RenderSettings::default()
            .color_level(ColorLevel::Ansi16)
            .attributes(TextAttribute::Italic.into())
            .underline_styles(UnderlineStyleSet::CURLY)
            .underline_colors(false)
            .hyperlinks(false);
        let resolved = settings.resolve_text_style(&style);

        assert_eq!(resolved.get_foreground(), Some(Color::BRIGHT_RED));
        assert_eq!(resolved.get_attributes(), TextAttribute::Italic.into());
        assert_eq!(
            resolved.get_underline(),
            Some(Underline::new(UnderlineStyle::Curly))
        );
        assert_eq!(resolved.get_hyperlink(), None);
    }

    #[test]
    fn all_preserves_every_style_feature() {
        let style = TextStyle::new()
            .foreground(Color::Rgb(1, 2, 3))
            .background(Color::Rgb(4, 5, 6))
            .bold()
            .italic()
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::BLUE)
            .hyperlink("https://example.com");

        assert_eq!(RenderSettings::all().resolve_text_style(&style), style);
    }

    #[test]
    fn reset_builders_restore_every_setting_default() {
        let settings = RenderSettings::default()
            .color_level(ColorLevel::TrueColor)
            .attributes(TextAttributes::all())
            .underline_styles(UnderlineStyleSet::all())
            .underline_colors(true)
            .hyperlinks(true)
            .reset_color_level()
            .reset_attributes()
            .reset_underline_styles()
            .reset_underline_colors()
            .reset_hyperlinks();

        assert_eq!(settings, RenderSettings::default());
    }
}
