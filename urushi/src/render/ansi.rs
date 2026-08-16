//! Output adapters for renderer-neutral views.

use crate::{Limits, RenderedBlock, TerminalProfile, View, resolve};

/// Renders a [`View`] as ANSI-capable text for one terminal profile.
///
/// The renderer computes no geometry: it resolves the view once and serializes
/// the resulting rectangle, coalescing adjacent graphemes of equal effective
/// style into one SGR scope.
#[derive(Debug, Clone, Copy)]
pub struct AnsiRenderer {
    profile: TerminalProfile,
}

impl AnsiRenderer {
    /// Creates a renderer for explicit terminal capabilities.
    pub const fn new(profile: TerminalProfile) -> Self {
        Self { profile }
    }

    /// Returns the terminal profile used by this renderer.
    pub const fn profile(&self) -> TerminalProfile {
        self.profile
    }

    /// Renders at the view's intrinsic size, without a trailing newline.
    pub fn render(&self, view: &View) -> RenderedBlock {
        self.render_within(view, Limits::NONE)
    }

    /// Renders cropped to `limits` — a terminal width, for instance.
    pub fn render_within(&self, view: &View, limits: Limits) -> RenderedBlock {
        resolve(view, limits)
            .map_styles(|style| self.profile.resolve_text_style(style))
            .into_rendered_block()
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnsiPolicy, Color, ColorProfile, TextStyle};

    use super::*;

    #[test]
    fn renderer_resolves_styles_at_the_output_boundary() {
        let style = TextStyle::new().foreground(Color::Rgb(10, 20, 30)).bold();
        let view = View::text("result", style.clone());
        let plain = AnsiRenderer::new(TerminalProfile::new(
            ColorProfile::Monochrome,
            AnsiPolicy::Disabled,
        ));

        assert_eq!(plain.render(&view).as_str(), "result");
        assert_eq!(
            resolve(&view, Limits::NONE).rows()[0][0].style(),
            &style,
            "the resolved view keeps logical styles"
        );
    }

    #[test]
    fn limits_crop_the_resolved_rectangle() {
        let renderer = AnsiRenderer::new(TerminalProfile::new(
            ColorProfile::TrueColor,
            AnsiPolicy::Disabled,
        ));
        let view = View::text("abcdef", TextStyle::new());

        assert_eq!(
            renderer.render_within(&view, Limits::width(3)).as_str(),
            "abc"
        );
    }
}
