//! The [`TextStyle`] builder: everything a terminal can express about a run of
//! text.

use crate::{Color, Hyperlink, Modifier, Underline, UnderlineStyle};

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
/// let text = urushi::StyledText::new("hello", emphasized);
/// let _ = text;
/// ```
///
/// Geometry is not merely discouraged here, it is unrepresentable:
///
/// ```compile_fail
/// use urushi::{Border, TextStyle};
///
/// let _ = TextStyle::new().border(Border::ROUNDED);
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub(crate) foreground: Option<Color>,
    pub(crate) background: Option<Color>,
    pub(crate) underline: Option<Underline>,
    pub(crate) hyperlink: Option<Hyperlink>,
    pub(crate) modifiers: Modifier,
}

impl TextStyle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the text foreground color.
    pub fn foreground(mut self, color: impl Into<Color>) -> Self {
        self.foreground = Some(color.into());
        self
    }

    /// Returns the text foreground color to the terminal default.
    pub fn reset_foreground(mut self) -> Self {
        self.foreground = None;
        self
    }

    /// Sets the text background color.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.background = Some(color.into());
        self
    }

    /// Returns the text background color to the terminal default.
    pub fn reset_background(mut self) -> Self {
        self.background = None;
        self
    }

    /// Adds every flag in `modifier` to the active text modifiers.
    pub fn add_modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers = self.modifiers.union(modifier);
        self
    }

    /// Removes every flag in `modifier` from the active text modifiers.
    pub fn remove_modifier(mut self, modifier: Modifier) -> Self {
        self.modifiers = self.modifiers.difference(modifier);
        self
    }

    /// Removes every active text modifier.
    pub fn reset_modifiers(mut self) -> Self {
        self.modifiers = Modifier::empty();
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

    /// Underlines the text with a single line in the foreground color.
    ///
    /// This sets the complete default underline value; the other two builders
    /// below refine an underline that may already be set.
    pub fn underlined(self) -> Self {
        self.underline(Underline::default())
    }

    /// Sets the shape the underline is drawn with, adding an underline in the
    /// foreground color when the style has none.
    pub fn underline_style(self, style: UnderlineStyle) -> Self {
        let underline = Underline {
            style,
            color: self.underline.and_then(|underline| underline.color),
        };
        self.underline(underline)
    }

    /// Sets the color the underline is drawn in, adding a single underline when
    /// the style has none.
    ///
    /// A color is only reachable through an underline, so a style cannot carry
    /// an underline color that nothing draws.
    pub fn underline_color(self, color: impl Into<Color>) -> Self {
        let underline = self.underline.unwrap_or_default().color(color.into());
        self.underline(underline)
    }

    /// Replaces the complete underline value.
    pub fn underline(mut self, underline: Underline) -> Self {
        self.underline = Some(underline);
        self
    }

    /// Removes the underline, including its color.
    pub fn reset_underline(mut self) -> Self {
        self.underline = None;
        self
    }

    /// Attaches an OSC 8 hyperlink to this text.
    ///
    /// A URI converts directly for the ordinary case. Use [`Hyperlink`] when
    /// the link needs parameters such as `id`.
    pub fn hyperlink(mut self, hyperlink: impl Into<Hyperlink>) -> Self {
        self.hyperlink = Some(hyperlink.into());
        self
    }

    /// Removes the OSC 8 hyperlink from this text.
    pub fn reset_hyperlink(mut self) -> Self {
        self.hyperlink = None;
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

    /// Returns the foreground color instruction, if this style sets one.
    pub const fn get_foreground(&self) -> Option<Color> {
        self.foreground
    }

    /// Returns the background color instruction, if this style sets one.
    pub const fn get_background(&self) -> Option<Color> {
        self.background
    }

    /// Returns the underline instruction, if this style sets one.
    pub const fn get_underline(&self) -> Option<Underline> {
        self.underline
    }

    /// Returns the hyperlink attached to this text, if any.
    pub fn get_hyperlink(&self) -> Option<&Hyperlink> {
        self.hyperlink.as_ref()
    }

    /// Returns the active text modifiers.
    pub const fn get_modifiers(&self) -> Modifier {
        self.modifiers
    }

    pub(crate) fn overlay(mut self, contribution: &Self) -> Self {
        let Self {
            foreground,
            background,
            underline,
            hyperlink,
            modifiers,
        } = contribution;
        if let Some(color) = foreground {
            self.foreground = Some(*color);
        }
        if let Some(color) = background {
            self.background = Some(*color);
        }
        if let Some(underline) = underline {
            self.underline = Some(*underline);
        }
        if let Some(hyperlink) = hyperlink {
            self.hyperlink = Some(hyperlink.clone());
        }
        self.modifiers = self.modifiers.union(*modifiers);
        self
    }

    /// Folds values that cannot reach the output, so that two styles with the
    /// same appearance are the same value.
    ///
    /// One fold exists: an underline color equal to the foreground draws
    /// exactly what an absent one draws, since an absent one means "the
    /// foreground color". Nothing else is folded — a value is dropped only when
    /// doing so cannot change the output whatever the terminal does, which is
    /// why reversed video, whose equivalence assumes how a terminal implements
    /// `dim`, stays as written.
    ///
    /// Applied once the style is final: [`RenderSettings`](crate::RenderSettings)
    /// calls it as its last step, after degradation, because degradation is what
    /// makes two logical colors equal. A `TextStyle` is an immutable value built
    /// by consuming builders, so any earlier fold is undone by the next call that
    /// changes the foreground, which is why this is not part of the public
    /// builder surface.
    ///
    /// One residue is not closable: when the foreground is absent its concrete
    /// color is the terminal's default and unknown here, so an underline color
    /// equal to it cannot be recognized.
    pub(crate) fn canonical(mut self) -> Self {
        if let Some(underline) = self.underline
            && underline.color.is_some()
            && underline.color == self.foreground
        {
            self.underline = Some(Underline {
                color: None,
                ..underline
            });
        }
        self
    }

    /// Replaces every color property while preserving the rest of the style.
    pub(crate) fn map_colors(mut self, map: impl Fn(Color) -> Color) -> Self {
        self.foreground = self.foreground.map(&map);
        self.background = self.background.map(&map);
        self.underline = self.underline.map(|underline| Underline {
            color: underline.color.map(&map),
            ..underline
        });
        self
    }

    /// Removes every color while preserving modifiers and the underline shape.
    ///
    /// An underline survives colorless render settings — it is a shape, not a
    /// color — but its color does not, exactly as a foreground does not.
    pub(crate) fn without_colors(mut self) -> Self {
        self.foreground = None;
        self.background = None;
        self.underline = self.underline.map(|underline| Underline {
            color: None,
            ..underline
        });
        self
    }

    /// The SGR sequence enabling this style's modifiers and colors, or an
    /// empty string when the style sets none of them.
    pub(crate) fn sgr_prefix(&self) -> String {
        let mut params: Vec<String> = Vec::new();
        // Attribute parameters are emitted in SGR order, the underline in the
        // slot its code occupies, so one style always spells one sequence.
        for (added, code) in [
            (self.modifiers.contains(Modifier::BOLD), "1"),
            (self.modifiers.contains(Modifier::DIM), "2"),
            (self.modifiers.contains(Modifier::ITALIC), "3"),
            (
                self.underline.is_some(),
                self.underline.unwrap_or_default().style.sgr_params(),
            ),
            (self.modifiers.contains(Modifier::SLOW_BLINK), "5"),
            (self.modifiers.contains(Modifier::REVERSED), "7"),
            (self.modifiers.contains(Modifier::HIDDEN), "8"),
            (self.modifiers.contains(Modifier::CROSSED_OUT), "9"),
        ] {
            if added {
                params.push(code.to_string());
            }
        }
        if let Some(c) = self.foreground {
            params.push(c.sgr_params(false));
        }
        if let Some(c) = self.background {
            params.push(c.sgr_params(true));
        }
        // An absent underline color is the terminal's default, which a style of
        // effective values expresses by emitting nothing: the reset that closes
        // every painted scope already restores it, so there is no SGR 59 here.
        if let Some(c) = self.underline.and_then(|underline| underline.color) {
            params.push(c.sgr_underline_params());
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
    use crate::test_support::render_style;

    #[test]
    fn named_operations_preserve_effective_value_semantics() {
        let style = TextStyle::new()
            .bold()
            .add_modifier(Modifier::ITALIC)
            .foreground(Color::CYAN)
            .remove_modifier(Modifier::ITALIC)
            .reset_foreground();

        assert_eq!(style.get_modifiers(), Modifier::BOLD);
        assert_eq!(style.get_foreground(), None);
    }

    #[test]
    fn reset_builders_restore_every_property_default() {
        let style = TextStyle::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .add_modifier(Modifier::BOLD | Modifier::ITALIC)
            .underline_color(Color::GREEN)
            .hyperlink("https://example.com")
            .reset_foreground()
            .reset_background()
            .reset_modifiers()
            .reset_underline()
            .reset_hyperlink();

        assert_eq!(style, TextStyle::new());
    }

    #[test]
    fn hyperlink_is_one_replaceable_and_removable_property() {
        let style = TextStyle::new()
            .hyperlink("https://first.example")
            .hyperlink(Hyperlink::new("https://second.example").parameter("id", "docs"));

        assert_eq!(
            style.get_hyperlink(),
            Some(&Hyperlink::new("https://second.example").parameter("id", "docs"))
        );
        assert_eq!(render_style(&style.reset_hyperlink(), "link"), "link");
    }

    #[test]
    fn hyperlink_scope_contains_sgr_and_closes_after_its_reset() {
        assert_eq!(
            render_style(
                &TextStyle::new()
                    .hyperlink(Hyperlink::new("https://example.com").parameter("id", "docs"))
                    .bold(),
                "link",
            ),
            "\x1b]8;id=docs;https://example.com\x1b\\\x1b[1mlink\x1b[0m\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn hyperlink_on_empty_text_emits_nothing() {
        assert_eq!(
            render_style(&TextStyle::new().hyperlink("https://example.com"), ""),
            ""
        );
    }

    #[test]
    fn multiline_hyperlink_closes_before_each_newline() {
        let style = TextStyle::new().hyperlink("https://example.com");
        let open = "\x1b]8;;https://example.com\x1b\\";
        let close = "\x1b]8;;\x1b\\";

        assert_eq!(
            render_style(&style, "first\n\nsecond\nthird\n"),
            format!("{open}first{close}\n\n{open}second{close}\n{open}third{close}\n")
        );
    }

    #[test]
    fn removing_a_modifier_removes_it_from_painted_value() {
        let style = TextStyle::new()
            .bold()
            .dim()
            .remove_modifier(Modifier::BOLD);

        assert_eq!(render_style(&style, "text"), "\x1b[2mtext\x1b[0m");
        assert_eq!(
            render_style(&TextStyle::new().remove_modifier(Modifier::all()), "text",),
            "text"
        );
    }

    #[test]
    fn singleton_properties_replace_and_remove_to_defaults() {
        let style = TextStyle::new()
            .foreground(Color::RED)
            .foreground(Color::BLUE)
            .background(Color::GREEN)
            .reset_background();

        assert_eq!(style.get_foreground(), Some(Color::BLUE));
        assert_eq!(style.get_background(), None);
    }

    #[test]
    fn every_underline_shape_paints_its_own_sgr_parameter() {
        let painted = [
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
        ]
        .map(|style| render_style(&TextStyle::new().underline_style(style), "t"));

        assert_eq!(
            painted,
            [
                "\x1b[4mt\x1b[0m",
                "\x1b[4:2mt\x1b[0m",
                "\x1b[4:3mt\x1b[0m",
                "\x1b[4:4mt\x1b[0m",
                "\x1b[4:5mt\x1b[0m",
            ]
        );
    }

    #[test]
    fn an_underline_color_paints_sgr_fifty_eight_in_its_indexed_or_rgb_form() {
        assert_eq!(
            render_style(&TextStyle::new().underline_color(Color::RED), "t"),
            "\x1b[4;58;5;1mt\x1b[0m"
        );
        assert_eq!(
            render_style(&TextStyle::new().underline_color(Color::Ansi256(212)), "t",),
            "\x1b[4;58;5;212mt\x1b[0m"
        );
        assert_eq!(
            render_style(
                &TextStyle::new()
                    .underline_style(UnderlineStyle::Curly)
                    .underline_color(Color::Rgb(1, 2, 3)),
                "t",
            ),
            "\x1b[4:3;58;2;1;2;3mt\x1b[0m"
        );
    }

    #[test]
    fn an_absent_underline_color_paints_no_underline_color_parameter() {
        // A style holds effective values and text rendering closes with a reset, so
        // the terminal default is expressed by emitting nothing — there is no
        // SGR 59 to restore it, and no color to spell it with.
        let painted = render_style(
            &TextStyle::new()
                .foreground(Color::RED)
                .underline_style(UnderlineStyle::Double),
            "t",
        );

        assert_eq!(painted, "\x1b[4:2;31mt\x1b[0m");
        assert!(!painted.contains("58"));
        assert!(!painted.contains("59"));
    }

    #[test]
    fn hidden_paints_sgr_eight() {
        assert_eq!(
            render_style(&TextStyle::new().hide(), "t"),
            "\x1b[8mt\x1b[0m"
        );
        assert_eq!(
            render_style(
                &TextStyle::new().hide().add_modifier(Modifier::REVERSED),
                "t",
            ),
            "\x1b[7;8mt\x1b[0m"
        );
    }

    #[test]
    fn an_underline_color_is_only_reachable_through_an_underline() {
        // Setting a color on a style with no underline adds the underline that
        // draws it, so "invisible underline color" is not a value that exists.
        let style = TextStyle::new().underline_color(Color::RED);

        assert_eq!(
            style.get_underline(),
            Some(Underline {
                style: UnderlineStyle::Single,
                color: Some(Color::RED),
            })
        );
        assert_eq!(
            render_style(
                &TextStyle::new()
                    .underline_color(Color::RED)
                    .reset_underline(),
                "t",
            ),
            "t"
        );
    }

    #[test]
    fn setting_a_shape_keeps_the_color_and_setting_a_color_keeps_the_shape() {
        let expected = Some(Underline {
            style: UnderlineStyle::Dotted,
            color: Some(Color::GREEN),
        });

        assert_eq!(
            TextStyle::new()
                .underline_color(Color::GREEN)
                .underline_style(UnderlineStyle::Dotted)
                .get_underline(),
            expected
        );
        assert_eq!(
            TextStyle::new()
                .underline_style(UnderlineStyle::Dotted)
                .underline_color(Color::GREEN)
                .get_underline(),
            expected
        );
        assert_eq!(
            TextStyle::new()
                .underline_color(Color::RED)
                .underline(Underline::new(UnderlineStyle::Double))
                .get_underline(),
            Some(Underline::new(UnderlineStyle::Double))
        );
    }

    #[test]
    fn canonical_folds_an_underline_color_equal_to_the_foreground() {
        let folded = TextStyle::new()
            .foreground(Color::RED)
            .underline_style(UnderlineStyle::Curly)
            .underline_color(Color::RED)
            .canonical();

        assert_eq!(
            folded,
            TextStyle::new()
                .foreground(Color::RED)
                .underline_style(UnderlineStyle::Curly)
        );
        assert_eq!(render_style(&folded, "t"), "\x1b[4:3;31mt\x1b[0m");
    }

    #[test]
    fn canonical_folds_only_what_the_terminal_cannot_draw_differently() {
        // Equal appearance is not the admission rule: a color the terminal
        // resolves separately is not folded, and neither is reversed video,
        // whose equivalence assumes how a terminal implements `dim`.
        let distinct_spellings = TextStyle::new()
            .foreground(Color::Ansi(1))
            .underline_color(Color::Rgb(255, 0, 0));
        let reversed = TextStyle::new()
            .foreground(Color::RED)
            .background(Color::BLUE)
            .reverse();

        assert_eq!(
            distinct_spellings.clone().canonical(),
            distinct_spellings.clone()
        );
        assert_eq!(reversed.clone().canonical(), reversed);
        // An absent foreground is the terminal's default, whose concrete color
        // is unknown here, so a matching underline color cannot be recognized.
        let default_foreground = TextStyle::new().underline_color(Color::RED);
        assert_eq!(
            default_foreground.clone().canonical(),
            default_foreground.clone()
        );
    }
}
