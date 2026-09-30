//! Collects an in-memory release plan through typed prompt fields on a
//! caller-owned Crossterm backend.
//!
//! The example does not publish artifacts or change remote state.

use std::{io, time::Duration};

use urushi_prompt::{
    Confirm, ConfirmAnswer, FieldKey, Form, FormOutcome, Group, Input, Select, SelectOption,
    ValidationError,
    urushi::{
        Align, Available, ComponentRole, PanelRole, RenderSettings, Theme, ThemePreset, View,
        render, resolve,
    },
};
use urushi_terminal::{
    ColorLevel, Command, CommandWriter, Event, EventSource, KeyboardEnhancementQuery, Position,
    RawModeControl, TerminalCapabilities, TerminalOutput, TerminalQuery, TerminalSize,
    TextAttributes, UnderlineStyles, WindowSize, backend::crossterm::CrosstermBackend,
};

struct TrueColorTerminal {
    inner: CrosstermBackend<io::Stdout>,
}

impl TrueColorTerminal {
    fn stdout() -> Self {
        Self {
            inner: CrosstermBackend::new(io::stdout()),
        }
    }
}

impl TerminalOutput for TrueColorTerminal {
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl CommandWriter for TrueColorTerminal {
    fn write_command(&mut self, command: Command<'_>) -> io::Result<()> {
        self.inner.write_command(command)
    }
}

impl EventSource for TrueColorTerminal {
    fn read_event(&mut self) -> io::Result<Event> {
        self.inner.read_event()
    }

    fn poll_event(&mut self) -> io::Result<Option<Event>> {
        self.inner.poll_event()
    }

    fn poll_event_timeout(&mut self, timeout: Duration) -> io::Result<Option<Event>> {
        self.inner.poll_event_timeout(timeout)
    }
}

impl RawModeControl for TrueColorTerminal {
    fn is_interactive(&self) -> bool {
        self.inner.is_interactive()
    }

    fn enable_raw_mode(&mut self) -> io::Result<()> {
        self.inner.enable_raw_mode()
    }

    fn disable_raw_mode(&mut self) -> io::Result<()> {
        self.inner.disable_raw_mode()
    }
}

impl TerminalQuery for TrueColorTerminal {
    fn terminal_size(&mut self) -> io::Result<TerminalSize> {
        self.inner.terminal_size()
    }

    fn cursor_position(&mut self) -> io::Result<Position> {
        self.inner.cursor_position()
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        self.inner.window_size()
    }

    fn raw_mode_enabled(&mut self) -> io::Result<bool> {
        self.inner.raw_mode_enabled()
    }

    fn terminal_capabilities(&mut self) -> io::Result<TerminalCapabilities> {
        Ok(TerminalCapabilities::none()
            .with_color_level(ColorLevel::TrueColor)
            .with_attributes(TextAttributes::all())
            .with_underline_styles(UnderlineStyles::all())
            .with_underline_colors(true))
    }
}

impl KeyboardEnhancementQuery for TrueColorTerminal {
    fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
        self.inner.supports_keyboard_enhancement()
    }
}

fn theme() -> Theme {
    let base = ThemePreset::get("TokyoNight")
        .expect("the built-in TokyoNight theme is available")
        .theme();
    let tokens = *base.tokens();
    let mut components = base.components().clone();
    for role in [
        ComponentRole::Body,
        ComponentRole::Muted,
        ComponentRole::Accent,
        ComponentRole::Success,
        ComponentRole::Error,
        ComponentRole::PromptQuestion,
        ComponentRole::PromptAnswer,
        ComponentRole::PromptPlaceholder,
        ComponentRole::PromptCursor,
        ComponentRole::PromptOption,
        ComponentRole::PromptOptionSelected,
        ComponentRole::PromptButton,
        ComponentRole::PromptButtonFocused,
        ComponentRole::PromptHelp,
        ComponentRole::PromptError,
    ] {
        let style = base.text_style(role);
        components = components.text_style(
            role,
            if style.get_background().is_some() {
                style
            } else {
                style.background(tokens.background)
            },
        );
    }
    Theme::new(tokens, components)
}

fn valid_version(value: &str) -> Result<(), ValidationError> {
    let valid = value
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .is_ok_and(|parts| parts.len() == 3);
    if valid {
        Ok(())
    } else {
        Err(ValidationError::new(
            "Use a semantic version such as 0.2.0.",
        ))
    }
}

fn completion_view(theme: &Theme, version: &str, target: &str) -> View {
    let surface_text = |value, role| {
        View::block(
            urushi_prompt::urushi::BlockStyle::new().background(theme.tokens().surface),
            View::text(
                value,
                theme.text_style(role).background(theme.tokens().surface),
            ),
        )
    };
    View::block(
        theme
            .block_style(PanelRole::PanelFocused)
            .background(theme.tokens().surface)
            .border_background(theme.tokens().surface)
            .padding((1, 2)),
        View::column(
            Align::Left,
            [
                surface_text("✓ Release plan ready".to_owned(), ComponentRole::Success),
                surface_text(format!("Urushi {version} · {target}"), ComponentRole::Body),
                surface_text(
                    "The terminal session was restored before this summary was written.".to_owned(),
                    ComponentRole::Muted,
                ),
            ],
        ),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = theme();
    let version_key = FieldKey::new("version");
    let target_key = FieldKey::new("target");
    let confirm_key = FieldKey::<ConfirmAnswer>::new("confirm");

    let form = Form::builder()
        .group(
            Group::builder()
                .title("Prepare an Urushi release")
                .description("Choose the version and the first publication target.")
                .field(
                    Input::new(version_key.clone(), "Which version is ready?", "0.2.0")?
                        .description("Use semantic versioning without a leading v.")
                        .validate(Box::new(|value: &String| valid_version(value))),
                )
                .field(
                    Select::new(
                        target_key.clone(),
                        "Where should the release go first?",
                        vec![
                            SelectOption::new("All supported platforms", "Linux, macOS, Windows"),
                            SelectOption::new("Linux packages", "Linux"),
                            SelectOption::new("macOS packages", "macOS"),
                            SelectOption::new("Windows packages", "Windows"),
                            SelectOption::new("Crates.io only", "Crates.io"),
                        ],
                    )?
                    .description("Press / to filter the target list."),
                )
                .field(
                    Confirm::new(confirm_key.clone(), "Create this release plan?", Some(true))?
                        .description("The example does not publish or change remote state.")
                        .labels("Create plan", "Go back"),
                )
                .build()?,
        )
        .build()?;

    let mut terminal = TrueColorTerminal::stdout();
    match form.run_with_terminal(&mut terminal, &theme)? {
        FormOutcome::Submitted(values) => {
            let version = values.get(&version_key).expect("submitted version");
            let target = values.get(&target_key).expect("submitted target");
            let confirmed = values.get(&confirm_key).expect("submitted confirmation");
            if confirmed.value {
                let resolved = resolve(
                    &completion_view(&theme, version, target),
                    Available::columns(100),
                )?;
                println!("{}", render(&resolved, &RenderSettings::all()));
            }
        }
        FormOutcome::Cancelled => eprintln!("Release planning cancelled."),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::valid_version;

    #[test]
    fn accepts_three_numeric_version_parts() {
        assert!(valid_version("0.2.0").is_ok());
        assert!(valid_version("v0.2.0").is_err());
        assert!(valid_version("0.2").is_err());
    }
}
