//! Serialization of a resolved rectangle.

use crate::{RenderSettings, ResolvedView, StyledGrapheme, StyledText, TextStyle};

const RESET: &str = "\x1b[0m";

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

/// Serializes styled source text without resolving terminal-cell layout.
///
/// Tabs and line boundaries are written as authored. Callers choosing this
/// path accept the destination terminal's tab-stop behavior; use a `View` and
/// [`render`] when width, wrapping, alignment, or backend geometry matters.
pub fn render_text(text: &StyledText, settings: &RenderSettings) -> String {
    let mut output = String::new();
    for (segment, style) in text.spans() {
        output.push_str(&serialize_run(segment, &settings.resolve_text_style(style)));
    }
    output
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
        output.push_str(&serialize_run(&run, &style));
    }
    output
}

fn serialize_run(text: &str, style: &TextStyle) -> String {
    if text.is_empty() {
        return String::new();
    }
    let sgr = style.sgr_prefix();
    let Some(hyperlink) = style.get_hyperlink() else {
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
        output.push_str(&paint_hyperlink_line(line, &open, &sgr));
        if segment.ends_with('\n') {
            output.push('\n');
        }
    }
    output
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
            .hyperlink(Hyperlink::new("https://example.com").parameter("id", "docs"));
        let resolved = resolve(&View::text("link", style), Available::NONE).unwrap();

        assert_eq!(render(&resolved, &RenderSettings::default()), "link");
        assert_eq!(
            render(&resolved, &RenderSettings::default().hyperlinks(true)),
            "\x1b]8;id=docs;https://example.com\x1b\\link\x1b]8;;\x1b\\"
        );
    }

    #[test]
    fn direct_text_rendering_preserves_tabs_and_style_spans() {
        let text = StyledText::try_from_spans([
            crate::TextSpan::new("name\t", TextStyle::new().bold()),
            crate::TextSpan::new("値", TextStyle::new().foreground(Color::RED)),
        ])
        .unwrap();
        let settings = RenderSettings::default()
            .color_level(crate::ColorLevel::Ansi16)
            .attributes(crate::TextAttribute::Bold.into());

        assert_eq!(
            render_text(&text, &settings),
            "\x1b[1mname\t\x1b[0m\x1b[31m値\x1b[0m"
        );
    }
}
