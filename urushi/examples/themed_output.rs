use std::io::stdout;

use urushi::{Color, ColorScheme, PanelRole, SemanticTokens, TerminalProfile, Theme, ThemeSet};

fn light_tokens() -> SemanticTokens {
    SemanticTokens {
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
    }
}

fn dark_tokens() -> SemanticTokens {
    SemanticTokens {
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
    }
}

fn main() {
    let themes = ThemeSet::new(
        Theme::from_tokens(light_tokens()),
        Theme::from_tokens(dark_tokens()),
    );
    let theme = themes.select(ColorScheme::Dark);

    let output = stdout();
    let profile = TerminalProfile::detect_for(&output);
    let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));

    println!("{}", panel.render("保存しました"));
}
