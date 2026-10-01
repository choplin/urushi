//! Serialization of a resolved rectangle.

use urushi_terminal::{
    HyperlinkParameter, TerminalHyperlink, TerminalStyle, backend::ansi::AnsiWriter,
};

use super::TerminalTextStyle;
use crate::{Hyperlink, RenderSettings, ResolvedView, StyledGrapheme, StyledText, TextStyle};

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
    let terminal = TerminalTextStyle::from(style);
    let terminal_style = terminal.style();
    let Some(hyperlink) = terminal.hyperlink() else {
        if terminal_style == TerminalStyle::default() {
            return text.to_owned();
        }
        return paint_line(text, terminal_style, None);
    };
    let parameters = hyperlink_parameters(hyperlink);
    let hyperlink = TerminalHyperlink {
        uri: hyperlink.uri(),
        parameters: &parameters,
    };
    if !text.contains('\n') {
        return paint_line(text, terminal_style, Some(hyperlink));
    }

    let mut output = String::with_capacity(text.len());
    for segment in text.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        output.push_str(&paint_line(line, terminal_style, Some(hyperlink)));
        if segment.ends_with('\n') {
            output.push('\n');
        }
    }
    output
}

fn hyperlink_parameters(hyperlink: &Hyperlink) -> Vec<HyperlinkParameter<'_>> {
    hyperlink
        .parameters()
        .iter()
        .map(|(key, value)| HyperlinkParameter { key, value })
        .collect()
}

fn paint_line(
    text: &str,
    style: TerminalStyle,
    hyperlink: Option<TerminalHyperlink<'_>>,
) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut encoder = AnsiWriter::new(Vec::with_capacity(text.len()));
    if let Some(hyperlink) = hyperlink {
        encoder
            .write_hyperlink_start(hyperlink)
            .expect("writing to a Vec cannot fail");
    }
    encoder
        .write_style_prefix(style)
        .expect("writing to a Vec cannot fail");
    encoder.writer_mut().extend_from_slice(text.as_bytes());
    if style != TerminalStyle::default() {
        encoder
            .write_style_reset()
            .expect("writing to a Vec cannot fail");
    }
    if hyperlink.is_some() {
        encoder
            .write_hyperlink_end()
            .expect("writing to a Vec cannot fail");
    }
    String::from_utf8(encoder.into_inner()).expect("ANSI encoding preserves UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;
    use urushi_terminal::{
        Command, CommandWriter, TerminalText, TextAttributes, Underline, UnderlineStyle,
        backend::ansi::AnsiWriter,
    };

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

    #[test]
    fn static_and_command_paths_share_every_style_parameter_mapping() {
        for underline_style in [
            UnderlineStyle::Single,
            UnderlineStyle::Double,
            UnderlineStyle::Curly,
            UnderlineStyle::Dotted,
            UnderlineStyle::Dashed,
        ] {
            let style = TextStyle::new()
                .foreground(Color::Ansi(3))
                .background(Color::Ansi256(212))
                .add_attributes(TextAttributes::all())
                .underline(Underline::new(underline_style).color(Color::Rgb(1, 2, 3)));
            let static_output =
                render_text(&StyledText::new("x", style.clone()), &RenderSettings::all());
            let terminal_style = TerminalTextStyle::from(&style).style();
            let mut command_output = AnsiWriter::new(Vec::new());
            command_output
                .write_command(Command::SetStyle(terminal_style))
                .expect("command style encodes");
            command_output
                .write_command(Command::Print(
                    TerminalText::try_from("x").expect("text is printable"),
                ))
                .expect("command text encodes");
            command_output
                .write_command(Command::ResetStyle)
                .expect("command reset encodes");
            let command_output =
                String::from_utf8(command_output.into_inner()).expect("ANSI is UTF-8");

            let static_parameters = static_output
                .strip_prefix("\x1b[")
                .and_then(|output| output.split_once('m'))
                .map(|(parameters, _)| parameters)
                .expect("static output starts with SGR");
            let command_parameters = command_output
                .strip_prefix("\x1b[0;")
                .and_then(|output| output.split_once('m'))
                .map(|(parameters, _)| parameters)
                .expect("command output starts with reset plus SGR");

            assert_eq!(static_parameters, command_parameters);
            assert!(static_output.ends_with("mx\x1b[0m"));
            assert!(command_output.ends_with("mx\x1b[0m"));
        }
    }

    #[test]
    fn static_and_command_paths_share_hyperlink_escaping_and_framing() {
        let hyperlink = Hyperlink::new("https://example.test/a\u{1b}\u{9c}")
            .parameter("i:d=;\u{9d}", "v:a=l;ue\n\u{9b}");
        let static_output = render_text(
            &StyledText::new("link", TextStyle::new().hyperlink(hyperlink.clone())),
            &RenderSettings::all(),
        );
        let parameters = hyperlink_parameters(&hyperlink);
        let mut command_output = AnsiWriter::new(Vec::new());
        command_output
            .write_command(Command::SetHyperlink(Some(TerminalHyperlink {
                uri: hyperlink.uri(),
                parameters: &parameters,
            })))
            .expect("command hyperlink encodes");
        command_output
            .write_command(Command::Print(
                TerminalText::try_from("link").expect("text is printable"),
            ))
            .expect("command text encodes");
        command_output
            .write_command(Command::SetHyperlink(None))
            .expect("command hyperlink closes");

        assert_eq!(static_output.as_bytes(), command_output.into_inner());
    }
}
