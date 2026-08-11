#![cfg(feature = "ratatui")]

use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color as RatatuiColor, Modifier},
};
use urushi::{
    AnsiPolicy, Color, ColorProfile, ComponentRole, SemanticTokens, TerminalProfile, Theme,
    visible_width,
};

#[test]
fn one_theme_component_renders_to_plain_cli_and_ratatui() {
    let theme = Theme::from_tokens(tokens());
    let panel = theme.style(ComponentRole::PanelFocused);

    let plain = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled)
        .resolve_style(panel)
        .render("保存しました");
    assert!(plain.contains("保存しました"));
    assert!(plain.lines().all(|line| visible_width(line) == 16));
    assert!(plain.contains("38;2;80;160;255"));

    let backend = TestBackend::new(16, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| frame.render_widget(panel.widget("保存しました"), frame.area()))
        .expect("draw frame");
    let buffer = terminal.backend().buffer();

    assert_eq!(visual_line(buffer, 0), "╭──────────────╮");
    assert_eq!(visual_line(buffer, 1), "│ 保存しました │");
    assert_eq!(visual_line(buffer, 2), "╰──────────────╯");

    let border = buffer.cell((0, 0)).expect("border cell");
    assert_eq!(border.fg, RatatuiColor::Rgb(80, 160, 255));
    let content = buffer.cell((2, 1)).expect("content cell");
    assert_eq!(content.fg, RatatuiColor::White);
    assert_eq!(content.bg, RatatuiColor::Black);
    assert!(!content.modifier.contains(Modifier::BOLD));
}

#[test]
fn theme_widget_clips_safely_at_a_boundary_size() {
    let theme = Theme::from_tokens(tokens());
    let panel = theme.style(ComponentRole::PanelFocused);
    let backend = TestBackend::new(6, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");

    terminal
        .draw(|frame| frame.render_widget(panel.widget("日本語"), frame.area()))
        .expect("draw frame");

    let buffer = terminal.backend().buffer();
    assert_eq!(visual_line(buffer, 0), "╭────╮");
    assert_eq!(visual_line(buffer, 1), "│ 日 │");
    assert_eq!(visual_line(buffer, 2), "╰────╯");
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

fn visual_line(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
    let mut line = String::new();
    let mut x = buffer.area.left();
    while x < buffer.area.right() {
        let symbol = buffer.cell((x, y)).expect("cell").symbol();
        line.push_str(symbol);
        let width = visible_width(symbol).max(1).min(usize::from(u16::MAX)) as u16;
        x = x.saturating_add(width);
    }
    line
}
