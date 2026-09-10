//! The [`TextStyle`] builder: everything a terminal can express about a run of
//! text.

use crate::{
    Color, Hyperlink, Modifier, TextStyleProperty, TextStylePropertyKey, Underline, UnderlineStyle,
};

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
    underline: Option<Underline>,
    hyperlink: Option<Hyperlink>,
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
            TextStyleProperty::Underline(underline) => self.underline = Some(underline),
            TextStyleProperty::Hyperlink(hyperlink) => self.hyperlink = Some(hyperlink),
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
            TextStylePropertyKey::Underline => self.underline = None,
            TextStylePropertyKey::Hyperlink => self.hyperlink = None,
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

    /// Underlines the text with a single line in the foreground color.
    ///
    /// This is the shorthand for `add(Underline::default())`; the other two
    /// builders below refine an underline that may already be set.
    pub fn underline(self) -> Self {
        self.add(TextStyleProperty::Underline(Underline::default()))
    }

    /// Sets the shape the underline is drawn with, adding an underline in the
    /// foreground color when the style has none.
    pub fn underline_style(self, style: UnderlineStyle) -> Self {
        let underline = Underline {
            style,
            color: self.underline.and_then(|underline| underline.color),
        };
        self.add(TextStyleProperty::Underline(underline))
    }

    /// Sets the color the underline is drawn in, adding a single underline when
    /// the style has none.
    ///
    /// A color is only reachable through an underline, so a style cannot carry
    /// an underline color that nothing draws.
    pub fn underline_color(self, color: impl Into<Color>) -> Self {
        let underline = self.underline.unwrap_or_default().with_color(color.into());
        self.add(TextStyleProperty::Underline(underline))
    }

    /// Attaches an OSC 8 hyperlink to this text.
    ///
    /// A URI converts directly for the ordinary case. Use [`Hyperlink`] when
    /// the link needs parameters such as `id`.
    pub fn hyperlink(self, hyperlink: impl Into<Hyperlink>) -> Self {
        self.add(TextStyleProperty::Hyperlink(hyperlink.into()))
    }

    pub fn blink(self) -> Self {
        self.add(Modifier::SLOW_BLINK)
    }

    pub fn reverse(self) -> Self {
        self.add(Modifier::REVERSED)
    }

    pub fn hide(self) -> Self {
        self.add(Modifier::HIDDEN)
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

    /// Returns the underline instruction, if this style sets one.
    pub const fn underline_value(&self) -> Option<Underline> {
        self.underline
    }

    /// Returns the hyperlink attached to this text, if any.
    pub fn hyperlink_value(&self) -> Option<&Hyperlink> {
        self.hyperlink.as_ref()
    }

    /// Returns the active text modifiers.
    pub const fn modifiers(&self) -> Modifier {
        self.modifiers
    }

    pub(crate) fn overlay(mut self, contribution: &Self) -> Self {
        let Self {
            fg,
            bg,
            underline,
            hyperlink,
            modifiers,
        } = contribution;
        if let Some(color) = fg {
            self.fg = Some(*color);
        }
        if let Some(color) = bg {
            self.bg = Some(*color);
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
    /// Applied once the style is final: [`TerminalProfile`](crate::TerminalProfile)
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
            && underline.color == self.fg
        {
            self.underline = Some(Underline {
                color: None,
                ..underline
            });
        }
        self
    }

    /// Wraps plain `text` in this style's SGR scope.
    ///
    /// A style that emits no sequence returns `text` unchanged. This produces
    /// no rectangle: padding, borders, and dimensions belong to
    /// [`BlockStyle`](crate::BlockStyle).
    ///
    /// `text` is plain. Painting already-rendered output nests SGR scopes, and
    /// the inner scope's reset ends this one early.
    pub fn paint(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        let sgr = self.sgr_prefix();
        let Some(hyperlink) = &self.hyperlink else {
            if sgr.is_empty() {
                return text.to_owned();
            }
            return format!("{sgr}{text}{RESET}");
        };
        let open = hyperlink.open_sequence();
        if !text.contains('\n') {
            return paint_hyperlink_line(text, &open, &sgr);
        }

        let mut output = String::with_capacity(text.len() + open.len());
        for segment in text.split_inclusive('\n') {
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            let (line, carriage_return) = line
                .strip_suffix('\r')
                .map_or((line, false), |line| (line, true));
            output.push_str(&paint_hyperlink_line(line, &open, &sgr));
            if carriage_return {
                output.push('\r');
            }
            if segment.ends_with('\n') {
                output.push('\n');
            }
        }
        output
    }

    /// Replaces every color property while preserving the rest of the style.
    pub(crate) fn map_colors(mut self, map: impl Fn(Color) -> Color) -> Self {
        self.fg = self.fg.map(&map);
        self.bg = self.bg.map(&map);
        self.underline = self.underline.map(|underline| Underline {
            color: underline.color.map(&map),
            ..underline
        });
        self
    }

    /// Removes every color while preserving modifiers and the underline shape.
    ///
    /// An underline survives a colorless profile — it is a shape, not a color —
    /// but its color does not, exactly as a foreground does not.
    pub(crate) fn without_colors(mut self) -> Self {
        self.fg = None;
        self.bg = None;
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
        if let Some(c) = self.fg {
            params.push(c.sgr_params(false));
        }
        if let Some(c) = self.bg {
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

fn paint_hyperlink_line(text: &str, open: &str, sgr: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    if sgr.is_empty() {
        return format!("{open}{text}\x1b]8;;\x1b\\");
    }
    format!("{open}{sgr}{text}{RESET}\x1b]8;;\x1b\\")
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
    fn hyperlink_is_one_replaceable_and_removable_property() {
        let style = TextStyle::new()
            .hyperlink("https://first.example")
            .add(Hyperlink::new("https://second.example").with_parameter("id", "docs"));

        assert_eq!(
            style.hyperlink_value(),
            Some(&Hyperlink::new("https://second.example").with_parameter("id", "docs"))
        );
        assert_eq!(
            style.remove(TextStylePropertyKey::Hyperlink).paint("link"),
            "link"
        );
    }

    #[test]
    fn hyperlink_scope_contains_sgr_and_closes_after_its_reset() {
        assert_eq!(
            TextStyle::new()
                .hyperlink(Hyperlink::new("https://example.com").with_parameter("id", "docs"))
                .bold()
                .paint("link"),
            "\x1b]8;id=docs;https://example.com\x1b\\\x1b[1mlink\x1b[0m\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn hyperlink_on_empty_text_emits_nothing() {
        assert_eq!(
            TextStyle::new().hyperlink("https://example.com").paint(""),
            ""
        );
    }

    #[test]
    fn multiline_hyperlink_closes_before_each_line_boundary() {
        let style = TextStyle::new().hyperlink("https://example.com");
        let open = "\x1b]8;;https://example.com\x1b\\";
        let close = "\x1b]8;;\x1b\\";

        assert_eq!(
            style.paint("first\n\nsecond\r\nthird\n"),
            format!("{open}first{close}\n\n{open}second{close}\r\n{open}third{close}\n")
        );
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

    #[test]
    fn every_underline_shape_paints_its_own_sgr_parameter() {
        let painted = [
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
        ]
        .map(|style| TextStyle::new().underline_style(style).paint("t"));

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
            TextStyle::new().underline_color(Color::RED).paint("t"),
            "\x1b[4;58;5;1mt\x1b[0m"
        );
        assert_eq!(
            TextStyle::new()
                .underline_color(Color::Ansi256(212))
                .paint("t"),
            "\x1b[4;58;5;212mt\x1b[0m"
        );
        assert_eq!(
            TextStyle::new()
                .underline_style(UnderlineStyle::Curly)
                .underline_color(Color::Rgb(1, 2, 3))
                .paint("t"),
            "\x1b[4:3;58;2;1;2;3mt\x1b[0m"
        );
    }

    #[test]
    fn an_absent_underline_color_paints_no_underline_color_parameter() {
        // A style holds effective values and `paint` closes with a reset, so
        // the terminal default is expressed by emitting nothing — there is no
        // SGR 59 to restore it, and no color to spell it with.
        let painted = TextStyle::new()
            .foreground(Color::RED)
            .underline_style(UnderlineStyle::Double)
            .paint("t");

        assert_eq!(painted, "\x1b[4:2;31mt\x1b[0m");
        assert!(!painted.contains("58"));
        assert!(!painted.contains("59"));
    }

    #[test]
    fn hidden_paints_sgr_eight() {
        assert_eq!(TextStyle::new().hide().paint("t"), "\x1b[8mt\x1b[0m");
        assert_eq!(
            TextStyle::new().hide().add(Modifier::REVERSED).paint("t"),
            "\x1b[7;8mt\x1b[0m"
        );
    }

    #[test]
    fn an_underline_color_is_only_reachable_through_an_underline() {
        // Setting a color on a style with no underline adds the underline that
        // draws it, so "invisible underline color" is not a value that exists.
        let style = TextStyle::new().underline_color(Color::RED);

        assert_eq!(
            style.underline_value(),
            Some(Underline {
                style: UnderlineStyle::Single,
                color: Some(Color::RED),
            })
        );
        assert_eq!(
            TextStyle::new()
                .underline_color(Color::RED)
                .remove(TextStylePropertyKey::Underline)
                .paint("t"),
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
                .underline_value(),
            expected
        );
        assert_eq!(
            TextStyle::new()
                .underline_style(UnderlineStyle::Dotted)
                .underline_color(Color::GREEN)
                .underline_value(),
            expected
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
        assert_eq!(folded.paint("t"), "\x1b[4:3;31mt\x1b[0m");
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
