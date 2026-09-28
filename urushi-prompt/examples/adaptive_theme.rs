#[cfg(unix)]
use urushi_prompt::{
    Confirm, ConfirmAnswer, FieldKey, Form, FormOutcome, Group,
    urushi::{Color, ColorScheme, SemanticTokens, Theme, ThemeMode, ThemeSet},
};
#[cfg(unix)]
use urushi_terminal::{TerminalQuery, backend::native::NativeTerminal};

#[cfg(unix)]
fn tokens(scheme: ColorScheme) -> SemanticTokens {
    match scheme {
        ColorScheme::Light => SemanticTokens {
            text: Color::Rgb(20, 30, 40),
            text_muted: Color::Rgb(90, 100, 110),
            background: Color::Rgb(255, 255, 255),
            surface: Color::Rgb(245, 245, 245),
            accent: Color::Rgb(40, 100, 180),
            accent_text: Color::Rgb(255, 255, 255),
            success: Color::Rgb(0, 128, 0),
            warning: Color::Rgb(180, 120, 0),
            error: Color::Rgb(200, 0, 0),
            border: Color::Rgb(130, 130, 130),
        },
        ColorScheme::Dark => SemanticTokens {
            text: Color::Rgb(230, 230, 230),
            text_muted: Color::Rgb(150, 150, 150),
            background: Color::Rgb(20, 24, 28),
            surface: Color::Rgb(32, 36, 44),
            accent: Color::Rgb(100, 170, 255),
            accent_text: Color::Rgb(20, 24, 28),
            success: Color::Rgb(80, 200, 120),
            warning: Color::Rgb(240, 190, 70),
            error: Color::Rgb(255, 100, 100),
            border: Color::Rgb(110, 120, 130),
        },
    }
}

#[cfg(unix)]
fn requested_mode() -> ThemeMode {
    match std::env::args().nth(1).as_deref() {
        Some("--light") => ThemeMode::Light,
        Some("--dark") => ThemeMode::Dark,
        _ => ThemeMode::Auto {
            fallback: ColorScheme::Dark,
        },
    }
}

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let themes = ThemeSet::new(
        Theme::from_tokens(tokens(ColorScheme::Light)),
        Theme::from_tokens(tokens(ColorScheme::Dark)),
    );
    let mode = requested_mode();
    let mut terminal = NativeTerminal::open()?;

    // Explicit light/dark modes perform no background query. Auto queries the
    // same physical connection that the form will use and has an explicit
    // fallback for terminals that do not answer.
    let background = match mode {
        ThemeMode::Auto { .. } => terminal.terminal_background()?,
        ThemeMode::Light | ThemeMode::Dark => None,
    };
    let theme = themes.select(mode.resolve(background));

    let confirmation = FieldKey::<ConfirmAnswer>::new("confirmation");
    let form = Form::builder()
        .group(
            Group::builder()
                .field(Confirm::new(
                    confirmation.clone(),
                    "Use the selected theme?",
                    Some(true),
                )?)
                .build()?,
        )
        .build()?;

    match form.run_with_terminal(&mut terminal, theme)? {
        FormOutcome::Submitted(values) => {
            if let Some(answer) = values.get(&confirmation) {
                println!("answer: {}", answer.value);
            }
        }
        FormOutcome::Cancelled => println!("cancelled"),
    }
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("this example requires the native Unix terminal backend");
}
