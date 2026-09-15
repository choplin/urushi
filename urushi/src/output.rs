//! Convenience output for the process standard streams.

use std::io::{self, Write};

use urushi_terminal::{ColorLevel, TerminalDetection};

use crate::{Available, RenderSettings, StyledText, View, render, render_text, resolve};

/// Resolves `view` for stdout and writes it without a trailing newline.
pub fn print_view(view: &View) -> io::Result<()> {
    let stdout = io::stdout();
    let detection = urushi_terminal::detect(&stdout)?;
    write_view_detected(stdout.lock(), view, detection, false, no_color())
}

/// Resolves `view` for stdout and writes it with a trailing newline.
pub fn println_view(view: &View) -> io::Result<()> {
    let stdout = io::stdout();
    let detection = urushi_terminal::detect(&stdout)?;
    write_view_detected(stdout.lock(), view, detection, true, no_color())
}

/// Resolves `view` for stderr and writes it without a trailing newline.
pub fn eprint_view(view: &View) -> io::Result<()> {
    let stderr = io::stderr();
    let detection = urushi_terminal::detect(&stderr)?;
    write_view_detected(stderr.lock(), view, detection, false, no_color())
}

/// Resolves `view` for stderr and writes it with a trailing newline.
pub fn eprintln_view(view: &View) -> io::Result<()> {
    let stderr = io::stderr();
    let detection = urushi_terminal::detect(&stderr)?;
    write_view_detected(stderr.lock(), view, detection, true, no_color())
}

/// Writes styled text to stdout without resolving layout or adding a newline.
pub fn print(text: &StyledText) -> io::Result<()> {
    let stdout = io::stdout();
    let detection = urushi_terminal::detect(&stdout)?;
    write_text_detected(stdout.lock(), text, detection, false, no_color())
}

/// Writes styled text to stdout without resolving layout, then adds a newline.
pub fn println(text: &StyledText) -> io::Result<()> {
    let stdout = io::stdout();
    let detection = urushi_terminal::detect(&stdout)?;
    write_text_detected(stdout.lock(), text, detection, true, no_color())
}

/// Writes styled text to stderr without resolving layout or adding a newline.
pub fn eprint(text: &StyledText) -> io::Result<()> {
    let stderr = io::stderr();
    let detection = urushi_terminal::detect(&stderr)?;
    write_text_detected(stderr.lock(), text, detection, false, no_color())
}

/// Writes styled text to stderr without resolving layout, then adds a newline.
pub fn eprintln(text: &StyledText) -> io::Result<()> {
    let stderr = io::stderr();
    let detection = urushi_terminal::detect(&stderr)?;
    write_text_detected(stderr.lock(), text, detection, true, no_color())
}

fn no_color() -> bool {
    std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty())
}

fn write_view_detected(
    writer: impl Write,
    view: &View,
    detection: TerminalDetection,
    newline: bool,
    no_color: bool,
) -> io::Result<()> {
    let (available, settings) = output_configuration(detection, no_color);
    write_configured(writer, view, available, settings, newline)
}

fn write_text_detected(
    mut writer: impl Write,
    text: &StyledText,
    detection: TerminalDetection,
    newline: bool,
    no_color: bool,
) -> io::Result<()> {
    let (_, settings) = output_configuration(detection, no_color);
    writer.write_all(render_text(text, &settings).as_bytes())?;
    if newline {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn write_configured(
    mut writer: impl Write,
    view: &View,
    available: Available,
    settings: RenderSettings,
    newline: bool,
) -> io::Result<()> {
    let resolved = resolve(view, available).map_err(io::Error::other)?;
    writer.write_all(render(&resolved, &settings).as_bytes())?;
    if newline {
        writer.write_all(b"\n")?;
    }
    Ok(())
}

fn output_configuration(
    detection: TerminalDetection,
    no_color: bool,
) -> (Available, RenderSettings) {
    match detection {
        TerminalDetection::Terminal(info) => {
            let settings = RenderSettings::from(info.capabilities());
            let settings = apply_no_color(settings, no_color);
            (Available::columns(info.size().columns()), settings)
        }
        TerminalDetection::NonTerminal => (Available::NONE, RenderSettings::default()),
    }
}

fn apply_no_color(settings: RenderSettings, no_color: bool) -> RenderSettings {
    if no_color {
        settings.with_colors(ColorLevel::None)
    } else {
        settings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{Color, Modifier, TextStyle};

    #[test]
    fn terminal_output_uses_its_width_and_capabilities() {
        let mut output = Vec::new();
        let view = View::text(
            "abcdef",
            TextStyle::new().foreground(Color::RED).bold().italic(),
        );

        write_configured(
            &mut output,
            &view,
            Available::columns(3),
            RenderSettings::default()
                .with_colors(ColorLevel::Ansi16)
                .with_modifiers(Modifier::BOLD),
            false,
        )
        .unwrap();

        assert_eq!(
            String::from_utf8(output).unwrap(),
            "\x1b[1;31mabc\x1b[0m\n\x1b[1;31mdef\x1b[0m"
        );
    }

    #[test]
    fn no_color_narrows_settings_without_disabling_other_features() {
        let settings = RenderSettings::default()
            .with_colors(ColorLevel::Ansi16)
            .with_modifiers(Modifier::BOLD);

        let narrowed = apply_no_color(settings, true);

        assert_eq!(narrowed.colors(), ColorLevel::None);
        assert_eq!(narrowed.modifiers(), Modifier::BOLD);
        assert_eq!(apply_no_color(settings, false), settings);
    }

    #[test]
    fn redirected_output_is_an_unbounded_plain_dump() {
        let mut output = Vec::new();
        let view = View::text("abcdef", TextStyle::new().foreground(Color::RED));

        write_view_detected(
            &mut output,
            &view,
            TerminalDetection::NonTerminal,
            true,
            false,
        )
        .unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "abcdef\n");
    }

    #[test]
    fn redirected_direct_text_preserves_tabs_without_layout() {
        let mut output = Vec::new();
        let text = StyledText::new("name\tvalue", TextStyle::new().bold());

        write_text_detected(
            &mut output,
            &text,
            TerminalDetection::NonTerminal,
            true,
            false,
        )
        .unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "name\tvalue\n");
    }
}
