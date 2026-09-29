//! Built-in terminal theme palettes mapped to Urushi semantic roles.

use std::fmt;

use crate::Color;

use super::{ColorScheme, SemanticTokens, Theme};

/// One named built-in terminal theme.
///
/// The catalog is generated from the terminal palette files in the pinned
/// iTerm2-Color-Schemes snapshot documented in
/// `urushi/THIRD_PARTY_NOTICES.md`. Every source theme is present; Urushi does
/// not select a preferred subset.
///
/// A terminal palette describes indexed colors, while an Urushi theme
/// describes semantic UI roles. The conversion uses terminal
/// foreground/background for `text`/`background`, ANSI red/green/yellow/blue
/// for the corresponding semantic colors, neutral foreground/background
/// blends for `surface`, `text_muted`, and `border`, and the background for
/// `accent_text`.
///
/// ```
/// use urushi::{ColorScheme, ThemePreset, ThemeSet};
///
/// let light = ThemePreset::get("Catppuccin Latte").unwrap();
/// let dark = ThemePreset::get("Catppuccin Mocha").unwrap();
/// assert_eq!(light.scheme(), ColorScheme::Light);
/// assert_eq!(dark.scheme(), ColorScheme::Dark);
/// let themes = ThemeSet::new(light.theme(), dark.theme());
/// assert_ne!(themes.light().tokens().background, themes.dark().tokens().background);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemePreset(usize);

impl ThemePreset {
    /// Returns every built-in theme in name order.
    pub const fn all() -> &'static [Self] {
        &generated::THEME_PRESETS
    }

    /// Finds a built-in theme by its exact name.
    pub fn get(name: &str) -> Option<Self> {
        generated::THEME_DATA
            .binary_search_by(|data| data.name.cmp(name))
            .ok()
            .map(Self)
    }

    /// Returns the source theme's name.
    pub const fn name(self) -> &'static str {
        generated::THEME_DATA[self.0].name
    }

    /// Returns whether this preset has a light or dark background.
    pub const fn scheme(self) -> ColorScheme {
        generated::THEME_DATA[self.0].scheme
    }

    /// Builds an Urushi theme from this terminal palette.
    pub fn theme(self) -> Theme {
        generated::THEME_DATA[self.0].palette.theme()
    }
}

impl fmt::Debug for ThemePreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ThemePreset")
            .field("name", &self.name())
            .field("scheme", &self.scheme())
            .finish()
    }
}

impl fmt::Display for ThemePreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy)]
struct ThemeData {
    name: &'static str,
    scheme: ColorScheme,
    palette: PaletteTheme,
}

#[derive(Clone, Copy)]
struct PaletteTheme {
    foreground: Rgb,
    background: Rgb,
    red: Rgb,
    green: Rgb,
    yellow: Rgb,
    blue: Rgb,
}

impl PaletteTheme {
    fn theme(self) -> Theme {
        Theme::from_tokens(SemanticTokens {
            text: self.foreground.color(),
            text_muted: mix(self.background, self.foreground, 55).color(),
            background: self.background.color(),
            surface: mix(self.background, self.foreground, 8).color(),
            accent: self.blue.color(),
            accent_text: self.background.color(),
            success: self.green.color(),
            warning: self.yellow.color(),
            error: self.red.color(),
            border: mix(self.background, self.foreground, 25).color(),
        })
    }
}

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

impl Rgb {
    const fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }
}

fn mix(background: Rgb, foreground: Rgb, foreground_percent: u16) -> Rgb {
    fn channel(background: u8, foreground: u8, foreground_percent: u16) -> u8 {
        let background_percent = 100 - foreground_percent;
        let mixed =
            u16::from(background) * background_percent + u16::from(foreground) * foreground_percent;
        ((mixed + 50) / 100) as u8
    }

    Rgb(
        channel(background.0, foreground.0, foreground_percent),
        channel(background.1, foreground.1, foreground_percent),
        channel(background.2, foreground.2, foreground_percent),
    )
}

mod generated {
    use super::{ColorScheme, PaletteTheme, Rgb, ThemeData, ThemePreset};

    include!("generated_presets.rs");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_the_complete_pinned_snapshot() {
        assert_eq!(ThemePreset::all().len(), 463);
        assert_eq!(
            ThemePreset::all()
                .iter()
                .filter(|preset| preset.scheme() == ColorScheme::Light)
                .count(),
            78
        );
        assert_eq!(
            ThemePreset::all()
                .iter()
                .filter(|preset| preset.scheme() == ColorScheme::Dark)
                .count(),
            385
        );
    }

    #[test]
    fn catalog_names_are_sorted_and_unique() {
        for presets in ThemePreset::all().windows(2) {
            assert!(presets[0].name() < presets[1].name());
        }
    }

    #[test]
    fn exact_name_lookup_returns_the_catalog_value() {
        let preset = ThemePreset::get("Catppuccin Mocha").expect("preset exists");
        assert_eq!(preset.name(), "Catppuccin Mocha");
        assert_eq!(preset.scheme(), ColorScheme::Dark);
        assert_eq!(
            preset.theme().tokens().background,
            Color::Rgb(0x1e, 0x1e, 0x2e)
        );
        assert!(ThemePreset::get("catppuccin mocha").is_none());
    }

    #[test]
    fn every_preset_builds_a_semantic_theme() {
        for preset in ThemePreset::all() {
            let tokens = preset.theme().tokens().to_owned();
            assert_ne!(tokens.text, tokens.background, "{preset}");
            assert_ne!(tokens.surface, tokens.background, "{preset}");
        }
    }
}
