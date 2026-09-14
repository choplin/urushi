use std::error::Error;

use ratatui::{Terminal, backend::TestBackend};
use urushi::{Color, PanelRole, SemanticTokens, Theme, View};
use urushi_tui::ratatui::RatatuiStyleExt as _;

fn main() -> Result<(), Box<dyn Error>> {
    let theme = Theme::from_tokens(tokens());
    let panel = theme.block_style(PanelRole::PanelFocused);
    std::println!("plain CLI:");
    urushi::println(&View::block(
        panel.clone(),
        View::text("保存しました", panel.text().clone()),
    ))?;

    let backend = TestBackend::new(16, 3);
    let mut terminal = Terminal::new(backend)?;
    terminal.draw(|frame| {
        frame.render_widget(panel.widget("保存しました"), frame.area());
    })?;

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer.cell((0, 0)).expect("top-left cell").symbol(), "╭");
    assert_eq!(buffer.cell((2, 1)).expect("first text cell").symbol(), "保");
    println!("ratatui: rendered the same Theme into a 16x3 TestBackend");
    Ok(())
}

fn tokens() -> SemanticTokens {
    SemanticTokens {
        text: Color::BRIGHT_WHITE,
        text_muted: Color::BRIGHT_BLACK,
        background: Color::BLACK,
        surface: Color::BLACK,
        accent: Color::Rgb(80, 160, 255),
        accent_text: Color::BLACK,
        success: Color::GREEN,
        warning: Color::YELLOW,
        error: Color::RED,
        border: Color::BRIGHT_BLACK,
    }
}
