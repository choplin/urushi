//! Serialization of a resolved rectangle.

use crate::{RenderSettings, ResolvedView, StyledGrapheme};

/// Serializes `view` using the selected output features.
///
/// Rendering does not perform layout, inspect a terminal, or write bytes. The
/// caller must resolve the view first and explicitly choose the settings.
pub fn render(view: &ResolvedView, settings: &RenderSettings) -> String {
    view.rows()
        .iter()
        .map(|row| serialize_row(row, settings))
        .collect::<Vec<_>>()
        .join("\n")
}

fn serialize_row(row: &[StyledGrapheme], settings: &RenderSettings) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < row.len() {
        let style = settings.resolve_text_style(row[index].style());
        let mut run = String::new();
        while index < row.len() && settings.resolve_text_style(row[index].style()) == style {
            run.push_str(row[index].symbol());
            index += 1;
        }
        output.push_str(&style.paint(&run));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Available, Color, Hyperlink, TextStyle, View, resolve};

    #[test]
    fn rendering_preserves_logical_styles_in_the_resolved_view() {
        let style = TextStyle::new().foreground(Color::Rgb(10, 20, 30)).bold();
        let resolved = resolve(&View::text("result", style.clone()), Available::NONE).unwrap();

        assert_eq!(render(&resolved, &RenderSettings::default()), "result");
        assert_eq!(resolved.rows()[0][0].style(), &style);
    }

    #[test]
    fn hyperlinks_are_selected_independently_from_sgr_features() {
        let style = TextStyle::new()
            .hyperlink(Hyperlink::new("https://example.com").with_parameter("id", "docs"));
        let resolved = resolve(&View::text("link", style), Available::NONE).unwrap();

        assert_eq!(render(&resolved, &RenderSettings::default()), "link");
        assert_eq!(
            render(&resolved, &RenderSettings::default().with_hyperlinks(true)),
            "\x1b]8;id=docs;https://example.com\x1b\\link\x1b]8;;\x1b\\"
        );
    }
}
