use urushi::{AnsiPolicy, Border, Color, ColorProfile, Style, TerminalProfile};

#[test]
fn explicit_profile_resolves_a_public_style_without_environment_access() {
    let profile = TerminalProfile::new(ColorProfile::Ansi16, AnsiPolicy::Enabled);
    assert_eq!(profile.color_profile(), ColorProfile::Ansi16);
    assert_eq!(profile.ansi_policy(), AnsiPolicy::Enabled);

    let style = Style::new()
        .foreground(Color::Rgb(255, 0, 0))
        .background(Color::Rgb(0, 0, 0))
        .border(Border::ASCII)
        .border_foreground(Color::Rgb(0, 255, 0))
        .bold();
    assert_eq!(
        profile.resolve_style(&style).render("x"),
        "\x1b[92m+-+\x1b[0m\n\x1b[92m|\x1b[0m\x1b[1;91;40mx\x1b[0m\x1b[92m|\x1b[0m\n\x1b[92m+-+\x1b[0m"
    );
}

#[test]
fn detection_uses_the_given_non_tty_writer() {
    let file = std::fs::File::open("Cargo.toml").unwrap();
    let profile = TerminalProfile::detect_for(&file);
    assert_eq!(profile.color_profile(), ColorProfile::Monochrome);
    assert_eq!(profile.ansi_policy(), AnsiPolicy::Disabled);

    let resolved = profile.resolve_style(&Style::new().foreground(Color::RED).bold());
    assert_eq!(resolved.render("x"), "x");
}
