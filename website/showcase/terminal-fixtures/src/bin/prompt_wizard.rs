use urushi_prompt::{
    Confirm, ConfirmAnswer, FieldKey, Form, FormOutcome, Group, Input, Select, SelectOption,
    urushi::{Color, SemanticTokens, Theme},
};
use urushi_showcase_terminal_fixtures::CaptureTerminal;
use urushi_terminal::{
    ClearRegion, Command, CommandWriter, CursorMove, Position, TerminalOutput,
};

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
    let style_key = FieldKey::new("style");
    let proceed_key = FieldKey::<ConfirmAnswer>::new("proceed");
    let form = Form::builder()
        .group(
            Group::builder()
                .title("Greeting setup")
                .description("Review three values before generating a greeting.")
                .field(
                    Input::new(name_key.clone(), "What is your name?", "")
                        .expect("capture key is non-empty")
                        .description("The greeting will address this name.")
                        .placeholder("e.g. Alex")
                        .required(),
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
                    .expect("capture options are non-empty")
                    .description("Use the arrow keys or j/k to move."),
                )
                .field(
                    Confirm::new(proceed_key.clone(), "Generate the greeting?", Some(true))
                        .expect("capture key is non-empty")
                        .description("Press y/n to answer immediately."),
                )
                .build()
                .expect("capture group has fields"),
        )
        .build()
        .expect("capture form has unique field names");

    let mut terminal = CaptureTerminal::stderr();
    terminal
        .write_command(Command::Clear(ClearRegion::Screen))
        .expect("capture terminal can clear its surface");
    terminal
        .write_command(Command::MoveCursor(CursorMove::To(Position::new(0, 0))))
        .expect("capture terminal can move its cursor");
    terminal
        .flush()
        .expect("capture terminal can display its cleared surface");
    match form.run_with_terminal(&mut terminal, &theme()) {
        Ok(FormOutcome::Submitted(values)) => {
            let name = values.get(&name_key).expect("submitted name");
            let style = values.get(&style_key).expect("submitted style");
            let proceed = values.get(&proceed_key).expect("submitted confirmation");
            let greeting = match (*style, proceed.value) {
                (_, false) => "No greeting generated.".to_owned(),
                ("formal", true) => format!("Good day, {name}."),
                (_, true) => format!("Hi, {name}!"),
            };
            println!("{greeting}");
        }
        Ok(FormOutcome::Cancelled) => eprintln!("Cancelled."),
        Err(error) => eprintln!("Prompt error: {error:?}"),
    }
}
