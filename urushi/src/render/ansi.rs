//! Output adapters for renderer-neutral views.

use crate::{TerminalProfile, View};

/// Renders a [`View`] as ANSI-capable text for one terminal profile.
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

    /// Renders without adding a trailing newline.
    pub fn render(&self, view: &View) -> String {
        view.lines()
            .iter()
            .map(|line| {
                line.spans()
                    .iter()
                    .map(|span| self.profile.resolve_style(span.style()).render(span.text()))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use crate::{AnsiPolicy, Color, ColorProfile, Line, Style};

    use super::*;

    #[test]
    fn renderer_resolves_styles_at_the_output_boundary() {
        let view = View::line(Line::styled(
            "result",
            Style::new().foreground(Color::Rgb(10, 20, 30)).bold(),
        ));
        let plain = AnsiRenderer::new(TerminalProfile::new(
            ColorProfile::Monochrome,
            AnsiPolicy::Disabled,
        ));

        assert_eq!(plain.render(&view), "result");
        assert_eq!(
            view.lines()[0].spans()[0].style(),
            &Style::new().foreground(Color::Rgb(10, 20, 30)).bold()
        );
    }
}
