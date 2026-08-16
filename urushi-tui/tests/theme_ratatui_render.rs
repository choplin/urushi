use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color as RatatuiColor, Modifier},
};
use urushi::{
    AnsiPolicy, Color, ColorProfile, Modifier as UrushiModifier, PanelRole, SemanticTokens,
    TerminalProfile, TextStyle, Theme, visible_width,
};
use urushi_tui::{RatatuiStyle, RatatuiStyleExt as _};

#[test]
fn one_theme_component_renders_to_plain_cli_and_ratatui() {
    let theme = Theme::from_tokens(tokens());
    let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
    let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));

    let plain = panel.render("保存しました").into_string();
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
    let profile = TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Enabled);
    let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
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

#[test]
fn ratatui_uses_the_same_terminal_profile_degradation_as_plain_output() {
    let theme = Theme::from_tokens(tokens());

    for (color_profile, expected_border) in [
        (ColorProfile::TrueColor, RatatuiColor::Rgb(80, 160, 255)),
        (ColorProfile::Ansi256, RatatuiColor::Indexed(75)),
        (ColorProfile::Ansi16, RatatuiColor::LightCyan),
    ] {
        let profile = TerminalProfile::new(color_profile, AnsiPolicy::Enabled);
        let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
        let buffer = render_panel(&panel);
        assert_eq!(
            buffer.cell((0, 0)).expect("border cell").fg,
            expected_border
        );
        assert!(panel.render("保存しました").as_str().contains('\x1b'));
    }

    for profile in [
        TerminalProfile::new(ColorProfile::Monochrome, AnsiPolicy::Enabled),
        TerminalProfile::new(ColorProfile::TrueColor, AnsiPolicy::Disabled),
    ] {
        let panel = profile.resolve_block_style(&theme.block_style(PanelRole::PanelFocused));
        let buffer = render_panel(&panel);
        let border = buffer.cell((0, 0)).expect("border cell");
        let content = buffer.cell((2, 1)).expect("content cell");
        assert_eq!(border.fg, RatatuiColor::Reset);
        assert_eq!(content.fg, RatatuiColor::Reset);
        assert_eq!(content.bg, RatatuiColor::Reset);
        assert!(!panel.render("保存しました").as_str().contains('\x1b'));
    }
}

#[test]
fn ratatui_converts_the_active_modifier_set() {
    let converted = RatatuiStyle::from(
        &TextStyle::new()
            .add(UrushiModifier::BOLD | UrushiModifier::ITALIC)
            .remove(UrushiModifier::ITALIC),
    )
    .into_inner();

    assert_eq!(converted.add_modifier, Modifier::BOLD);
    assert!(converted.sub_modifier.is_empty());
}

fn render_panel(style: &urushi::BlockStyle) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(16, 3);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| frame.render_widget(style.widget("保存しました"), frame.area()))
        .expect("draw frame");
    terminal.backend().buffer().clone()
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
