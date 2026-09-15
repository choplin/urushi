//! Explicit feature selection for one render operation.

use crate::{Modifier, TextStyle, Underline, UnderlineStyleSet};
use urushi_terminal::{ColorLevel, TerminalCapabilities, TextAttributes, UnderlineStyles};

use super::palette::{quantize_to_ansi16, quantize_to_ansi256};

/// The output features selected for one render operation.
///
/// The default is deliberately dumb: no escape-sequence-producing feature is
/// selected. A terminal's detected maximum can be adopted with `From` and then
/// narrowed with the `with_*` methods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderSettings {
    colors: ColorLevel,
    modifiers: Modifier,
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
            colors: ColorLevel::TrueColor,
            modifiers: Modifier::all(),
            underline_styles: UnderlineStyleSet::all(),
            underline_colors: true,
            hyperlinks: true,
        }
    }

    pub const fn colors(self) -> ColorLevel {
        self.colors
    }

    pub const fn modifiers(self) -> Modifier {
        self.modifiers
    }

    pub const fn underline_styles(self) -> UnderlineStyleSet {
        self.underline_styles
    }

    pub const fn underline_colors(self) -> bool {
        self.underline_colors
    }

    pub const fn hyperlinks(self) -> bool {
        self.hyperlinks
    }

    pub const fn with_colors(mut self, colors: ColorLevel) -> Self {
        self.colors = colors;
        self
    }

    pub const fn with_modifiers(mut self, modifiers: Modifier) -> Self {
        self.modifiers = modifiers;
        self
    }

    pub const fn with_underline_styles(mut self, styles: UnderlineStyleSet) -> Self {
        self.underline_styles = styles;
        self
    }

    pub const fn with_underline_colors(mut self, enabled: bool) -> Self {
        self.underline_colors = enabled;
        self
    }

    pub const fn with_hyperlinks(mut self, enabled: bool) -> Self {
        self.hyperlinks = enabled;
        self
    }

    /// Applies these selected features to one logical text style.
    ///
    /// This is useful to output backends that serialize runs themselves, such
    /// as an interactive prompt renderer. Ordinary static output should call
    /// [`render`](crate::render) for the whole resolved view.
    pub fn resolve_text_style(self, style: &TextStyle) -> TextStyle {
        let mut resolved = match self.colors {
            ColorLevel::None => style.clone().without_colors(),
            ColorLevel::Ansi16 => style.clone().map_colors(quantize_to_ansi16),
            ColorLevel::Ansi256 => style.clone().map_colors(quantize_to_ansi256),
            ColorLevel::TrueColor => style.clone(),
        };
        resolved.modifiers = resolved.modifiers.intersection(self.modifiers);
        resolved.underline = resolved.underline.and_then(|underline| {
            self.underline_styles
                .contains(underline.style)
                .then_some(Underline {
                    color: self.underline_colors.then_some(underline.color).flatten(),
                    ..underline
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
            .with_colors(capabilities.colors())
            .with_modifiers(modifiers_from(capabilities.attributes()))
            .with_underline_styles(underline_styles_from(capabilities.underline_styles()))
            .with_underline_colors(capabilities.underline_colors())
            .with_hyperlinks(capabilities.hyperlinks())
    }
}

fn modifiers_from(attributes: TextAttributes) -> Modifier {
    let mut modifiers = Modifier::empty();
    for (capability, modifier) in [
        (TextAttributes::BOLD, Modifier::BOLD),
        (TextAttributes::DIM, Modifier::DIM),
        (TextAttributes::ITALIC, Modifier::ITALIC),
        (TextAttributes::SLOW_BLINK, Modifier::SLOW_BLINK),
        (TextAttributes::REVERSED, Modifier::REVERSED),
        (TextAttributes::HIDDEN, Modifier::HIDDEN),
        (TextAttributes::CROSSED_OUT, Modifier::CROSSED_OUT),
    ] {
        if attributes.contains(capability) {
            modifiers = modifiers.union(modifier);
        }
    }
    modifiers
}

fn underline_styles_from(styles: UnderlineStyles) -> UnderlineStyleSet {
    let mut selected = UnderlineStyleSet::empty();
    for (capability, style) in [
        (UnderlineStyles::SINGLE, UnderlineStyleSet::SINGLE),
        (UnderlineStyles::DOUBLE, UnderlineStyleSet::DOUBLE),
        (UnderlineStyles::CURLY, UnderlineStyleSet::CURLY),
        (UnderlineStyles::DOTTED, UnderlineStyleSet::DOTTED),
        (UnderlineStyles::DASHED, UnderlineStyleSet::DASHED),
    ] {
        if styles.contains(capability) {
            selected = selected.union(style);
        }
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Color, Hyperlink, UnderlineStyle};

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
            .with_colors(ColorLevel::Ansi16)
            .with_modifiers(Modifier::ITALIC)
            .with_underline_styles(UnderlineStyleSet::CURLY)
            .with_underline_colors(false)
            .with_hyperlinks(false);
        let resolved = settings.resolve_text_style(&style);

        assert_eq!(resolved.foreground_color(), Some(Color::BRIGHT_RED));
        assert_eq!(resolved.modifiers(), Modifier::ITALIC);
        assert_eq!(
            resolved.underline_value(),
            Some(Underline::new(UnderlineStyle::Curly))
        );
        assert_eq!(resolved.hyperlink_value(), None);
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
}
