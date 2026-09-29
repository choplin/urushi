//! Selects a stable application theme from the terminal background before the
//! runtime starts.
//!
//! Run this Unix-only example with:
//!
//! ```text
//! cargo run -p urushi-tui --example adaptive_theme
//! ```
//!
//! The caller owns the complete flow: it opens one bidirectional terminal
//! connection, queries its background only for [`ThemeMode::Auto`], constructs
//! the application with the selected themes, and moves that same connection
//! into [`Runtime`]. The application owns every built-in [`ThemePreset`] and a
//! selected light/dark pair. Press Tab to choose which half of the pair to edit,
//! the arrow keys to select its theme, and `l`, `d`, or `a` to preview Light,
//! Dark, or the startup-resolved Auto scheme. Page Up and Page Down jump ten
//! themes at a time. Urushi does not poll the background, query it again after
//! resize or focus events, or automatically replace the theme; every change in
//! this example is explicitly requested by a key press.

use std::io;

use urushi::{
    Align, BlockStyle, ColorScheme, ComponentRole, Length, PanelRole, TextStyle, Theme, ThemeMode,
    ThemePreset, ThemeSet, VerticalAlign, View, measure,
};
use urushi_terminal::TerminalQuery;
use urushi_tui::{Application, Effect, Input, KeyCode, KeyKind, Modifiers, Runtime, Subscription};

struct AdaptiveTheme {
    light_presets: Vec<PresetTheme>,
    dark_presets: Vec<PresetTheme>,
    auto_scheme: ColorScheme,
}

impl AdaptiveTheme {
    fn new(auto_scheme: ColorScheme) -> Self {
        let mut light_presets = Vec::new();
        let mut dark_presets = Vec::new();
        for preset in ThemePreset::all().iter().copied() {
            let entry = PresetTheme {
                preset,
                theme: preset.theme(),
            };
            match preset.scheme() {
                ColorScheme::Light => light_presets.push(entry),
                ColorScheme::Dark => dark_presets.push(entry),
            }
        }
        Self {
            light_presets,
            dark_presets,
            auto_scheme,
        }
    }

    fn presets(&self, scheme: ColorScheme) -> &[PresetTheme] {
        match scheme {
            ColorScheme::Light => &self.light_presets,
            ColorScheme::Dark => &self.dark_presets,
        }
    }

    fn selected_entry(&self, model: &Model, scheme: ColorScheme) -> &PresetTheme {
        let index = match scheme {
            ColorScheme::Light => model.light_index,
            ColorScheme::Dark => model.dark_index,
        };
        &self.presets(scheme)[index]
    }

    fn selected_theme_set(&self, model: &Model) -> ThemeSet {
        ThemeSet::new(
            self.selected_entry(model, ColorScheme::Light).theme.clone(),
            self.selected_entry(model, ColorScheme::Dark).theme.clone(),
        )
    }

    fn selected_theme(&self, model: &Model) -> Theme {
        self.selected_theme_set(model)
            .select(model.choice.resolve(self.auto_scheme))
            .clone()
    }

    fn initial_index(&self, scheme: ColorScheme, name: &str) -> usize {
        self.presets(scheme)
            .iter()
            .position(|entry| entry.preset.name() == name)
            .expect("the example's initial preset is built in")
    }

    fn cycle_edited_preset(&self, model: &mut Model, offset: isize) {
        let presets = self.presets(model.editing_scheme);
        let index = match model.editing_scheme {
            ColorScheme::Light => &mut model.light_index,
            ColorScheme::Dark => &mut model.dark_index,
        };
        *index = (isize::try_from(*index).expect("catalog index fits isize") + offset)
            .rem_euclid(isize::try_from(presets.len()).expect("catalog length fits isize"))
            as usize;
    }
}

struct PresetTheme {
    preset: ThemePreset,
    theme: Theme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThemeChoice {
    Light,
    Dark,
    Auto,
}

impl ThemeChoice {
    fn resolve(self, auto_scheme: ColorScheme) -> ColorScheme {
        match self {
            Self::Light => ColorScheme::Light,
            Self::Dark => ColorScheme::Dark,
            Self::Auto => auto_scheme,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::Dark => "Dark",
            Self::Auto => "Auto",
        }
    }
}

struct Model {
    light_index: usize,
    dark_index: usize,
    editing_scheme: ColorScheme,
    choice: ThemeChoice,
}

enum Message {
    Input(Input),
}

impl Application for AdaptiveTheme {
    type Model = Model;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (
            Model {
                light_index: self.initial_index(ColorScheme::Light, "Catppuccin Latte"),
                dark_index: self.initial_index(ColorScheme::Dark, "Catppuccin Mocha"),
                editing_scheme: self.auto_scheme,
                choice: ThemeChoice::Auto,
            },
            Effect::none(),
        )
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Input(Input::Key(key))
                if key.kind != KeyKind::Release && requests_quit(key.code, key.modifiers) =>
            {
                Effect::shutdown()
            }
            Message::Input(Input::Key(key)) if key.kind != KeyKind::Release => {
                match key.code {
                    KeyCode::Tab => model.editing_scheme = opposite(model.editing_scheme),
                    KeyCode::Left | KeyCode::Up | KeyCode::Char('[') => {
                        self.cycle_edited_preset(model, -1)
                    }
                    KeyCode::Right | KeyCode::Down | KeyCode::Char(']') => {
                        self.cycle_edited_preset(model, 1)
                    }
                    KeyCode::PageUp => self.cycle_edited_preset(model, -10),
                    KeyCode::PageDown => self.cycle_edited_preset(model, 10),
                    KeyCode::Char('l') => model.choice = ThemeChoice::Light,
                    KeyCode::Char('d') => model.choice = ThemeChoice::Dark,
                    KeyCode::Char('a') => model.choice = ThemeChoice::Auto,
                    _ => {}
                }
                Effect::none()
            }
            Message::Input(_) => Effect::none(),
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        let theme = self.selected_theme(model);
        let light = self.selected_entry(model, ColorScheme::Light);
        let dark = self.selected_entry(model, ColorScheme::Dark);
        let catalog = canvas_row(
            &theme,
            View::row(
                VerticalAlign::Top,
                [
                    catalog_panel(
                        &theme,
                        ColorScheme::Light,
                        model.editing_scheme,
                        &self.light_presets,
                        model.light_index,
                    ),
                    catalog_panel(
                        &theme,
                        ColorScheme::Dark,
                        model.editing_scheme,
                        &self.dark_presets,
                        model.dark_index,
                    ),
                ],
            ),
        );
        let selector = canvas_row(
            &theme,
            View::row(
                VerticalAlign::Top,
                [
                    mode_option(&theme, ThemeChoice::Light, model.choice, "[l] "),
                    mode_option(&theme, ThemeChoice::Dark, model.choice, "[d] "),
                    mode_option(&theme, ThemeChoice::Auto, model.choice, "[a] "),
                ],
            ),
        );
        let auto = scheme_label(self.auto_scheme);
        let preview = semantic_preview(
            &theme,
            model.choice,
            match model.choice.resolve(self.auto_scheme) {
                ColorScheme::Light => light.preset,
                ColorScheme::Dark => dark.preset,
            },
        );
        let content = View::column(
            Align::Left,
            [
                catalog,
                selector,
                View::block(
                    panel_style(&theme, PanelRole::PanelFocused).width(Length::fill(1)),
                    preview,
                ),
                canvas_row(
                    &theme,
                    View::text(
                        "Tab switches catalog; arrow keys step; PgUp/PgDn jumps 10.",
                        canvas_text_style(&theme, ComponentRole::Muted),
                    ),
                ),
                canvas_row(
                    &theme,
                    View::text(
                        format!(
                            "l/d/a previews Light/Dark/Auto; q quits. Auto resolved to {auto}."
                        ),
                        canvas_text_style(&theme, ComponentRole::Muted),
                    ),
                ),
            ],
        );
        View::block(
            canvas_style(&theme)
                .width(Length::fill(1))
                .height(Length::fill(1)),
            content,
        )
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::input(Message::Input)
    }
}

const fn opposite(scheme: ColorScheme) -> ColorScheme {
    match scheme {
        ColorScheme::Light => ColorScheme::Dark,
        ColorScheme::Dark => ColorScheme::Light,
    }
}

const fn scheme_label(scheme: ColorScheme) -> &'static str {
    match scheme {
        ColorScheme::Light => "Light",
        ColorScheme::Dark => "Dark",
    }
}

fn mode_option(theme: &Theme, option: ThemeChoice, selected: ThemeChoice, shortcut: &str) -> View {
    let role = if option == selected {
        ComponentRole::PromptOptionSelected
    } else {
        ComponentRole::PromptOption
    };
    canvas_backdrop(
        theme,
        View::text(
            format!(" {shortcut}{} ", option.label()),
            canvas_text_style(theme, role),
        ),
    )
}

fn canvas_style(theme: &Theme) -> BlockStyle {
    BlockStyle::new().background(theme.tokens().background)
}

fn canvas_row(theme: &Theme, content: View) -> View {
    View::block(canvas_style(theme).width(Length::fill(1)), content)
}

fn canvas_backdrop(theme: &Theme, content: View) -> View {
    View::block(canvas_style(theme), content)
}

fn canvas_text(theme: &Theme, text: impl Into<String>, role: ComponentRole) -> View {
    canvas_backdrop(theme, View::text(text, canvas_text_style(theme, role)))
}

fn canvas_text_style(theme: &Theme, role: ComponentRole) -> TextStyle {
    let style = theme.text_style(role);
    if style.get_background().is_some() {
        style
    } else {
        style.background(theme.tokens().background)
    }
}

fn panel_style(theme: &Theme, role: PanelRole) -> BlockStyle {
    theme
        .block_style(role)
        .background(theme.tokens().background)
        .border_background(theme.tokens().background)
}

fn catalog_panel(
    theme: &Theme,
    scheme: ColorScheme,
    editing: ColorScheme,
    presets: &[PresetTheme],
    selected: usize,
) -> View {
    let mut rows = vec![canvas_text(
        theme,
        format!(
            "{} themes — {}/{}",
            scheme_label(scheme),
            selected + 1,
            presets.len()
        ),
        ComponentRole::Accent,
    )];
    for index in catalog_window(presets.len(), selected) {
        let role = if index == selected {
            ComponentRole::PromptOptionSelected
        } else {
            ComponentRole::PromptOption
        };
        let marker = if index == selected { "›" } else { " " };
        rows.push(canvas_text(
            theme,
            format!("{marker} {}", presets[index].preset.name()),
            role,
        ));
    }
    let panel = if scheme == editing {
        PanelRole::PanelFocused
    } else {
        PanelRole::Panel
    };
    View::block(
        panel_style(theme, panel).width(catalog_panel_width(scheme, presets)),
        View::column(Align::Left, rows),
    )
}

fn catalog_panel_width(scheme: ColorScheme, presets: &[PresetTheme]) -> u16 {
    const MARKER_WIDTH: usize = 2;
    const FRAME_WIDTH: usize = 4;

    let title = format!(
        "{} themes — {}/{}",
        scheme_label(scheme),
        presets.len(),
        presets.len()
    );
    let title_width = measure(&View::text(title, TextStyle::new())).width();
    let item_width = presets
        .iter()
        .map(|entry| {
            MARKER_WIDTH + measure(&View::text(entry.preset.name(), TextStyle::new())).width()
        })
        .max()
        .unwrap_or(0);
    u16::try_from(title_width.max(item_width) + FRAME_WIDTH)
        .expect("the built-in theme catalog fits a terminal block")
}

fn catalog_window(length: usize, selected: usize) -> std::ops::Range<usize> {
    const ROWS: usize = 7;
    let start = selected
        .saturating_sub(ROWS / 2)
        .min(length.saturating_sub(ROWS));
    start..(start + ROWS.min(length))
}

fn semantic_preview(theme: &Theme, choice: ThemeChoice, preset: ThemePreset) -> View {
    let tokens = theme.tokens();
    View::column(
        Align::Left,
        [
            canvas_text(
                theme,
                format!("{} preview — {}", choice.label(), preset.name()),
                ComponentRole::Accent,
            ),
            canvas_text(
                theme,
                "Body text and muted supporting text",
                ComponentRole::Body,
            ),
            canvas_text(theme, "Muted: secondary information", ComponentRole::Muted),
            canvas_backdrop(
                theme,
                View::row(
                    VerticalAlign::Top,
                    [
                        View::text(
                            " Accent ",
                            TextStyle::new()
                                .foreground(tokens.accent_text)
                                .background(tokens.accent)
                                .bold(),
                        ),
                        View::text(
                            " Success ",
                            canvas_text_style(theme, ComponentRole::Success),
                        ),
                        View::text(
                            " Warning ",
                            TextStyle::new()
                                .foreground(tokens.warning)
                                .background(tokens.background)
                                .bold(),
                        ),
                        View::text(" Error ", canvas_text_style(theme, ComponentRole::Error)),
                    ],
                ),
            ),
            canvas_backdrop(
                theme,
                View::row(
                    VerticalAlign::Top,
                    [
                        View::text(
                            " Surface ",
                            TextStyle::new()
                                .foreground(tokens.text)
                                .background(tokens.surface),
                        ),
                        View::text(
                            " Border ",
                            TextStyle::new()
                                .foreground(tokens.border)
                                .background(tokens.background),
                        ),
                    ],
                ),
            ),
        ],
    )
}

fn requests_quit(code: KeyCode, modifiers: Modifiers) -> bool {
    matches!(code, KeyCode::Escape | KeyCode::Char('q'))
        || (code == KeyCode::Char('c') && modifiers.contains(Modifiers::CONTROL))
}

fn resolve_scheme(terminal: &mut impl TerminalQuery, mode: ThemeMode) -> io::Result<ColorScheme> {
    let background = match mode {
        ThemeMode::Auto { .. } => terminal.terminal_background()?,
        ThemeMode::Light | ThemeMode::Dark => None,
    };
    Ok(mode.resolve(background))
}

#[cfg(unix)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use urushi_terminal::backend::native::NativeTerminal;

    let mode = ThemeMode::Auto {
        fallback: ColorScheme::Dark,
    };
    let mut terminal = NativeTerminal::open()?;
    let auto_scheme = resolve_scheme(&mut terminal, mode)?;
    let application = AdaptiveTheme::new(auto_scheme);

    Runtime::new(application).backend(terminal).run()?;
    Ok(())
}

#[cfg(not(unix))]
fn main() {
    eprintln!("adaptive_theme requires the Unix NativeTerminal backend");
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex, MutexGuard};
    use std::time::Duration;

    use urushi::{Available, resolve};
    use urushi_terminal::{
        Command, CommandWriter, Event, EventSource, KeyEvent, KeyboardEnhancementQuery, Position,
        RawModeControl, TerminalBackground, TerminalOutput, TerminalSize, WindowSize,
    };

    use super::*;

    #[derive(Default)]
    struct Observation {
        background_queries: usize,
        session_operations: usize,
        events: VecDeque<Event>,
    }

    struct FakeTerminal {
        observation: Arc<Mutex<Observation>>,
    }

    impl TerminalOutput for FakeTerminal {
        fn flush(&mut self) -> io::Result<()> {
            lock(&self.observation).session_operations += 1;
            Ok(())
        }
    }

    impl CommandWriter for FakeTerminal {
        fn write_command(&mut self, _command: Command<'_>) -> io::Result<()> {
            lock(&self.observation).session_operations += 1;
            Ok(())
        }
    }

    impl EventSource for FakeTerminal {
        fn read_event(&mut self) -> io::Result<Event> {
            self.poll_event()?
                .ok_or_else(|| io::Error::other("no event"))
        }

        fn poll_event(&mut self) -> io::Result<Option<Event>> {
            Ok(lock(&self.observation).events.pop_front())
        }

        fn poll_event_timeout(&mut self, _timeout: Duration) -> io::Result<Option<Event>> {
            self.poll_event()
        }
    }

    impl RawModeControl for FakeTerminal {
        fn is_interactive(&self) -> bool {
            true
        }

        fn enable_raw_mode(&mut self) -> io::Result<()> {
            lock(&self.observation).session_operations += 1;
            Ok(())
        }

        fn disable_raw_mode(&mut self) -> io::Result<()> {
            lock(&self.observation).session_operations += 1;
            Ok(())
        }
    }

    impl TerminalQuery for FakeTerminal {
        fn terminal_size(&mut self) -> io::Result<TerminalSize> {
            Ok(TerminalSize::new(80, 24))
        }

        fn cursor_position(&mut self) -> io::Result<Position> {
            Ok(Position::new(0, 0))
        }

        fn window_size(&mut self) -> io::Result<WindowSize> {
            Ok(WindowSize::new(TerminalSize::new(80, 24), None))
        }

        fn raw_mode_enabled(&mut self) -> io::Result<bool> {
            Ok(false)
        }

        fn terminal_background(&mut self) -> io::Result<Option<TerminalBackground>> {
            lock(&self.observation).background_queries += 1;
            Ok(Some(TerminalBackground::new(0, 0, 0)))
        }
    }

    impl KeyboardEnhancementQuery for FakeTerminal {
        fn supports_keyboard_enhancement(&mut self) -> io::Result<bool> {
            Ok(false)
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn fake_terminal() -> (FakeTerminal, Arc<Mutex<Observation>>) {
        let observation = Arc::new(Mutex::new(Observation {
            events: VecDeque::from([Event::Key(KeyEvent::new(KeyCode::Char('q')))]),
            ..Observation::default()
        }));
        (
            FakeTerminal {
                observation: Arc::clone(&observation),
            },
            observation,
        )
    }

    #[test]
    fn auto_queries_before_construction_and_reuses_the_connection() {
        let (mut terminal, observation) = fake_terminal();
        let auto_scheme = resolve_scheme(
            &mut terminal,
            ThemeMode::Auto {
                fallback: ColorScheme::Light,
            },
        )
        .expect("background query succeeds");

        assert_eq!(lock(&observation).background_queries, 1);
        assert_eq!(lock(&observation).session_operations, 0);
        let application = AdaptiveTheme::new(auto_scheme);
        let (model, _) = application.init();
        assert_eq!(
            application.selected_theme(&model).tokens().background,
            ThemePreset::get("Catppuccin Mocha")
                .expect("preset exists")
                .theme()
                .tokens()
                .background
        );

        Runtime::new(application)
            .backend(terminal)
            .run()
            .expect("runtime exits on q");

        let observation = lock(&observation);
        assert_eq!(observation.background_queries, 1);
        assert!(observation.session_operations > 0);
    }

    #[test]
    fn explicit_modes_do_not_query_the_terminal() {
        for mode in [ThemeMode::Light, ThemeMode::Dark] {
            let (mut terminal, observation) = fake_terminal();
            resolve_scheme(&mut terminal, mode).expect("selection succeeds");
            assert_eq!(lock(&observation).background_queries, 0);
        }
    }

    #[test]
    fn catalog_window_keeps_the_selection_visible() {
        assert_eq!(catalog_window(78, 0), 0..7);
        assert_eq!(catalog_window(78, 20), 17..24);
        assert_eq!(catalog_window(78, 77), 71..78);
        assert_eq!(catalog_window(3, 2), 0..3);
    }

    #[test]
    fn catalog_panels_keep_the_full_catalog_width() {
        let application = AdaptiveTheme::new(ColorScheme::Dark);
        let (model, _) = application.init();
        let theme = application.selected_theme(&model);
        for scheme in [ColorScheme::Light, ColorScheme::Dark] {
            let presets = application.presets(scheme);
            let expected = usize::from(catalog_panel_width(scheme, presets));
            for selected in [0, presets.len() / 2, presets.len() - 1] {
                let panel = catalog_panel(&theme, scheme, scheme, presets, selected);
                assert_eq!(measure(&panel).width(), expected);
            }
        }
    }

    #[test]
    fn view_paints_the_theme_background_across_the_viewport() {
        let application = AdaptiveTheme::new(ColorScheme::Dark);
        let (model, _) = application.init();
        let background = application.selected_theme(&model).tokens().background;
        let resolved = resolve(&application.view(&model), Available::size(80, 24))
            .expect("the example view resolves");

        assert_eq!(resolved.size().width(), 80);
        assert_eq!(resolved.size().height(), 24);
        let unpainted = resolved
            .rows()
            .iter()
            .enumerate()
            .flat_map(|(row, graphemes)| {
                graphemes
                    .iter()
                    .enumerate()
                    .filter(|(_, grapheme)| grapheme.style().get_background().is_none())
                    .map(move |(column, grapheme)| (row, column, grapheme.symbol().to_owned()))
            })
            .collect::<Vec<_>>();
        assert!(unpainted.is_empty(), "unpainted cells: {unpainted:?}");
        assert!(
            resolved
                .rows()
                .last()
                .expect("viewport has rows")
                .iter()
                .all(|grapheme| grapheme.style().get_background() == Some(background))
        );
    }

    #[test]
    fn preset_and_scheme_controls_switch_between_application_owned_themes() {
        let application = AdaptiveTheme::new(ColorScheme::Dark);
        let (mut model, _) = application.init();
        assert_eq!(model.choice, ThemeChoice::Auto);
        assert_eq!(model.editing_scheme, ColorScheme::Dark);
        assert_eq!(
            application
                .selected_entry(&model, ColorScheme::Light)
                .preset
                .name(),
            "Catppuccin Latte"
        );
        assert_eq!(
            application
                .selected_entry(&model, ColorScheme::Dark)
                .preset
                .name(),
            "Catppuccin Mocha"
        );
        assert_eq!(
            application.selected_theme(&model).tokens().background,
            ThemePreset::get("Catppuccin Mocha")
                .expect("preset exists")
                .theme()
                .tokens()
                .background
        );

        for (key, choice, scheme) in [
            ('l', ThemeChoice::Light, ColorScheme::Light),
            ('d', ThemeChoice::Dark, ColorScheme::Dark),
            ('a', ThemeChoice::Auto, ColorScheme::Dark),
        ] {
            let _effect = application.update(
                &mut model,
                Message::Input(Input::Key(KeyEvent::new(KeyCode::Char(key)))),
            );
            assert_eq!(model.choice, choice);
            let expected_name = match scheme {
                ColorScheme::Light => "Catppuccin Latte",
                ColorScheme::Dark => "Catppuccin Mocha",
            };
            assert_eq!(
                application.selected_theme(&model).tokens().background,
                ThemePreset::get(expected_name)
                    .expect("preset exists")
                    .theme()
                    .tokens()
                    .background
            );
        }

        let dark_index = model.dark_index;
        let _effect = application.update(
            &mut model,
            Message::Input(Input::Key(KeyEvent::new(KeyCode::Right))),
        );
        assert_eq!(model.dark_index, dark_index + 1);
        assert_ne!(
            application
                .selected_entry(&model, ColorScheme::Dark)
                .preset
                .name(),
            "Catppuccin Mocha"
        );

        let _effect = application.update(
            &mut model,
            Message::Input(Input::Key(KeyEvent::new(KeyCode::Tab))),
        );
        assert_eq!(model.editing_scheme, ColorScheme::Light);
        let light_index = model.light_index;
        let _effect = application.update(
            &mut model,
            Message::Input(Input::Key(KeyEvent::new(KeyCode::Left))),
        );
        assert_eq!(model.light_index, light_index - 1);
        assert_eq!(model.dark_index, dark_index + 1);

        let themes = application.selected_theme_set(&model);
        assert_eq!(
            themes.light(),
            &application.selected_entry(&model, ColorScheme::Light).theme
        );
        assert_eq!(
            themes.dark(),
            &application.selected_entry(&model, ColorScheme::Dark).theme
        );
    }
}
