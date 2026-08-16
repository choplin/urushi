//! Output adapters for renderer-neutral views.

use crate::{Available, RenderedBlock, TerminalProfile, View, resolve};

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
        self.render_within(view, Available::NONE)
    }

    /// Renders the view resolved under `available` — a terminal width, for
    /// instance.
    pub fn render_within(&self, view: &View, available: Available) -> RenderedBlock {
        resolve(view, available)
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
            resolve(&view, Available::NONE).rows()[0][0].style(),
            &style,
            "the resolved view keeps logical styles"
        );
    }

    #[test]
    fn a_bare_text_leaf_wraps_under_a_width_bound() {
        let renderer = AnsiRenderer::new(TerminalProfile::new(
            ColorProfile::TrueColor,
            AnsiPolicy::Disabled,
        ));
        let view = View::text("abcdef", TextStyle::new());

        assert_eq!(
            renderer
                .render_within(&view, Available::columns(3))
                .as_str(),
            "abc\ndef"
        );
    }
}
