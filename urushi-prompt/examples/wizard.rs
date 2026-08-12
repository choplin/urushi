use std::io;

use urushi_prompt::{
    Confirm, ConfirmAnswer, FieldKey, Form, FormOutcome, Group, Input, Select, SelectOption,
    urushi::{Color, SemanticTokens, TerminalProfile, Theme},
};

fn theme() -> Theme<()> {
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
    let style_key = FieldKey::new("style");
    let proceed_key = FieldKey::<ConfirmAnswer>::new("proceed");
    let form = Form::builder()
        .group(
            Group::builder()
                .title("Greeting setup")
                .description("Review three values before generating a greeting.")
                .field(
                    Input::new(name_key.clone(), "What is your name?", "")
                        .expect("example key is non-empty")
                        .description("The greeting will address this name.")
                        .placeholder("e.g. Alex")
                        .required()
                        .required_message("Enter a name."),
                )
                .field(
                    Select::new(
                        style_key.clone(),
                        "Choose a greeting style",
                        vec![
                            SelectOption::new("Friendly", "friendly"),
                            SelectOption::new("Formal", "formal"),
                        ],
                    )
                    .expect("example options are non-empty")
                    .description("Use the arrow keys or j/k to move."),
                )
                .field(
                    Confirm::new(proceed_key.clone(), "Generate the greeting?", Some(true))
                        .expect("example key is non-empty")
                        .description("Press y/n to answer immediately."),
                )
                .build()
                .expect("example group has fields"),
        )
        .build()
        .expect("example form has unique field names");

    let stderr = io::stderr();
    let profile = TerminalProfile::detect_for(&stderr);
    match form.run(&theme(), &profile) {
        Ok(FormOutcome::Submitted(values)) => {
            let name = values.get(&name_key).expect("submitted name");
            let style = values.get(&style_key).expect("submitted style");
            let proceed = values.get(&proceed_key).expect("submitted confirmation");
            println!("{}", completion_message(name, style, proceed.value));
        }
        Ok(FormOutcome::Cancelled) => eprintln!("Cancelled."),
        Err(error) => eprintln!("Prompt error: {error:?}"),
    }
}

fn completion_message(name: &str, style: &str, proceed: bool) -> String {
    match (style, proceed) {
        (_, false) => "No greeting generated.".to_owned(),
        ("formal", true) => format!("Good day, {name}."),
        (_, true) => format!("Hi, {name}!"),
    }
}

#[cfg(test)]
mod tests {
    use super::completion_message;

    #[test]
    fn completion_message_follows_style_and_confirmation() {
        assert_eq!(completion_message("Alex", "friendly", true), "Hi, Alex!");
        assert_eq!(
            completion_message("Alex", "formal", true),
            "Good day, Alex."
        );
        assert_eq!(
            completion_message("Alex", "friendly", false),
            "No greeting generated."
        );
    }
}
