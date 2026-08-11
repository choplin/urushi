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
    let language_key = FieldKey::new("language");
    let proceed_key = FieldKey::<ConfirmAnswer>::new("proceed");
    let form = Form::builder()
        .group(
            Group::builder()
                .field(
                    Input::new(name_key.clone(), "お名前を入力してください", "")
                        .expect("example key is non-empty")
                        .placeholder("例: 花子")
                        .required(),
                )
                .field(
                    Select::new(
                        language_key.clone(),
                        "表示言語を選んでください",
                        vec![
                            SelectOption::new("日本語", "ja"),
                            SelectOption::new("English", "en"),
                        ],
                    )
                    .expect("example options are non-empty"),
                )
                .field(
                    Confirm::new(proceed_key.clone(), "この内容で開始しますか？", Some(true))
                        .expect("example key is non-empty"),
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
            println!(
                "こんにちは、{} さん（{}）",
                values.get(&name_key).expect("submitted name"),
                values.get(&language_key).expect("submitted language")
            );
        }
        Ok(FormOutcome::Cancelled) => eprintln!("キャンセルしました。"),
        Err(error) => eprintln!("Prompt error: {error:?}"),
    }
}
