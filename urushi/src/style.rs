//! The [`Style`] builder and its string renderer.

use crate::border::Border;
use crate::color::Color;
use crate::text::{visible_width, wrap};

const RESET: &str = "\x1b[0m";

/// Horizontal alignment of content within a styled block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

/// Spacing values for the four sides of a box.
///
/// Builder methods taking `impl Into<Sides>` accept CSS-like shorthands:
/// a single `u16` (all sides), `(vertical, horizontal)`, or
/// `(top, right, bottom, left)`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Sides {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl From<u16> for Sides {
    fn from(v: u16) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
}

impl From<(u16, u16)> for Sides {
    fn from((v, h): (u16, u16)) -> Self {
        Self {
            top: v,
            right: h,
            bottom: v,
            left: h,
        }
    }
}

impl From<(u16, u16, u16, u16)> for Sides {
    fn from((top, right, bottom, left): (u16, u16, u16, u16)) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Modifiers {
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    strikethrough: bool,
}

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
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    modifiers: Modifiers,
    padding: Sides,
    margin: Sides,
    border: Option<Border>,
    border_fg: Option<Color>,
    border_bg: Option<Color>,
    width: Option<u16>,
    align: Align,
}

impl Style {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the text (and padding) foreground color.
    pub fn foreground(mut self, color: impl Into<Color>) -> Self {
        self.fg = Some(color.into());
        self
    }

    /// Sets the text (and padding) background color.
    pub fn background(mut self, color: impl Into<Color>) -> Self {
        self.bg = Some(color.into());
        self
    }

    pub fn bold(mut self) -> Self {
        self.modifiers.bold = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.modifiers.dim = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.modifiers.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.modifiers.underline = true;
        self
    }

    pub fn blink(mut self) -> Self {
        self.modifiers.blink = true;
        self
    }

    pub fn reverse(mut self) -> Self {
        self.modifiers.reverse = true;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.modifiers.strikethrough = true;
        self
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

    /// Draws a border around the padded content.
    pub fn border(mut self, border: Border) -> Self {
        self.border = Some(border);
        self
    }

    /// Sets the border foreground color.
    pub fn border_foreground(mut self, color: impl Into<Color>) -> Self {
        self.border_fg = Some(color.into());
        self
    }

    /// Sets the border background color.
    pub fn border_background(mut self, color: impl Into<Color>) -> Self {
        self.border_bg = Some(color.into());
        self
    }

    /// Fixes the width of the padded content box (excluding border and
    /// margin). Content is word-wrapped to fit.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Sets the horizontal alignment of content within the box.
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
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
        self.modifiers = Modifiers::default();
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
            Some(w) => wrap(content, w),
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
        let mut rows: Vec<String> = Vec::with_capacity(pt + lines.len() + pb);
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

        // 4. Border.
        if let Some(b) = self.border {
            let bsgr = self.border_sgr_prefix();
            let breset = if bsgr.is_empty() { "" } else { RESET };
            let top: String = std::iter::repeat_n(b.top, total).collect();
            let bottom: String = std::iter::repeat_n(b.bottom, total).collect();
            let mut bordered = Vec::with_capacity(rows.len() + 2);
            bordered.push(format!("{bsgr}{}{top}{}{breset}", b.top_left, b.top_right));
            for row in rows {
                bordered.push(format!(
                    "{bsgr}{}{breset}{row}{bsgr}{}{breset}",
                    b.left, b.right
                ));
            }
            bordered.push(format!(
                "{bsgr}{}{bottom}{}{breset}",
                b.bottom_left, b.bottom_right
            ));
            rows = bordered;
        }

        // 5. Margin: plain, unstyled space outside the border.
        let m = self.margin;
        if m == Sides::default() {
            return rows.join("\n");
        }
        let outer = total + if self.border.is_some() { 2 } else { 0 };
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
        let m = self.modifiers;
        let mut params: Vec<String> = Vec::new();
        for (on, code) in [
            (m.bold, "1"),
            (m.dim, "2"),
            (m.italic, "3"),
            (m.underline, "4"),
            (m.blink, "5"),
            (m.reverse, "7"),
            (m.strikethrough, "9"),
        ] {
            if on {
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

fn reapply_after_reset(content: &str, sgr: &str) -> String {
    if sgr.is_empty() || !content.contains(RESET) {
        return content.to_string();
    }

    content.replace(RESET, &format!("{RESET}{sgr}"))
}
