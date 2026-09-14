use std::io::{self, Write};

use urushi_prompt::{
    FieldKey, Form, FormOutcome, Group, Input, PromptStart,
    urushi::{Color, SemanticTokens, Theme},
};

const PREFIX: &str = "Configuration: ";

fn theme() -> Theme {
    Theme::from_tokens(SemanticTokens {
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
    })
}

fn main() {
    let name_key = FieldKey::new("name");
    let form = Form::builder()
        .start(PromptStart::CurrentPosition {
            // PREFIX is ASCII, so its byte length is also its terminal width.
            column: PREFIX.len() as u16,
        })
        .width(32)
        .group(
            Group::builder()
                .field(Input::new(name_key.clone(), "Name?", "").expect("example key is non-empty"))
                .build()
                .expect("example group has a field"),
        )
        .build()
        .expect("example form is valid");

    let stderr = io::stderr();
    let mut output = stderr.lock();
    write!(output, "{PREFIX}").expect("write prompt prefix");
    output.flush().expect("flush prompt prefix");
    drop(output);

    match form.run(&theme()) {
        Ok(FormOutcome::Submitted(values)) => {
            let name = values.get(&name_key).expect("submitted name");
            println!("Configured for {name}.");
        }
        Ok(FormOutcome::Cancelled) => eprintln!("Cancelled."),
        Err(error) => eprintln!("Prompt error: {error:?}"),
    }
}
