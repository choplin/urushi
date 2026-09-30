//! Animates a representative release pipeline through the Urushi TUI runtime.
//!
//! Stage completion is simulated with delayed effects; the example does not
//! run Cargo commands or report live CI results.

use std::time::Duration;

use urushi::{
    Align, BlockStyle, ComponentRole, Length, PanelRole, TextStyle, Theme, ThemePreset,
    VerticalAlign, View,
};
use urushi_tui_app::{
    Application, Effect, Error, Input, KeyCode, KeyKind, Modifiers, Runtime, Subscription,
};

const STAGES: [(&str, &str); 4] = [
    ("Compile", "9 workspace crates"),
    ("Test", "Default and all features"),
    ("Package", "9 release archives"),
    ("Verify", "Linux · macOS · Windows"),
];

struct ReleaseMonitor {
    theme: Theme,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum StageState {
    #[default]
    Waiting,
    Running,
    Complete,
}

struct Model {
    stages: [StageState; STAGES.len()],
    current: usize,
}

impl Model {
    fn new() -> Self {
        let mut stages = [StageState::Waiting; STAGES.len()];
        stages[0] = StageState::Running;
        Self { stages, current: 0 }
    }

    fn advance(&mut self) -> bool {
        if self.current >= self.stages.len() {
            return false;
        }
        self.stages[self.current] = StageState::Complete;
        self.current += 1;
        if self.current < self.stages.len() {
            self.stages[self.current] = StageState::Running;
            true
        } else {
            false
        }
    }

    fn complete_count(&self) -> usize {
        self.stages
            .iter()
            .filter(|state| **state == StageState::Complete)
            .count()
    }
}

enum Message {
    Input(Input),
    Advance,
}

fn next_stage() -> Effect<Message> {
    Effect::perform(|| {
        std::thread::sleep(Duration::from_millis(900));
        Message::Advance
    })
}

impl ReleaseMonitor {
    fn canvas_text(&self, value: impl Into<String>, role: ComponentRole) -> View {
        View::block(
            BlockStyle::new().background(self.theme.tokens().background),
            View::text(
                value,
                self.theme
                    .text_style(role)
                    .background(self.theme.tokens().background),
            ),
        )
    }

    fn surface_text(&self, value: impl Into<String>, role: ComponentRole) -> View {
        View::block(
            BlockStyle::new().background(self.theme.tokens().surface),
            View::text(
                value,
                self.theme
                    .text_style(role)
                    .background(self.theme.tokens().surface),
            ),
        )
    }

    fn canvas_space(&self, value: &'static str) -> View {
        View::block(
            BlockStyle::new().background(self.theme.tokens().background),
            View::text(
                value,
                TextStyle::new().background(self.theme.tokens().background),
            ),
        )
    }

    fn canvas_backdrop(&self, content: View) -> View {
        View::block(
            BlockStyle::new().background(self.theme.tokens().background),
            content,
        )
    }

    fn stage_card(&self, index: usize, state: StageState) -> View {
        let (name, detail) = STAGES[index];
        let (marker, role, panel) = match state {
            StageState::Waiting => ("○", ComponentRole::Muted, PanelRole::Panel),
            StageState::Running => ("◆", ComponentRole::Accent, PanelRole::PanelFocused),
            StageState::Complete => ("✓", ComponentRole::Success, PanelRole::Panel),
        };
        View::block(
            self.theme
                .block_style(panel)
                .background(self.theme.tokens().surface)
                .border_background(self.theme.tokens().surface)
                .width(18)
                .padding((1, 1)),
            View::column(
                Align::Left,
                [
                    self.surface_text(format!("{marker} {name}"), role),
                    self.surface_text(detail, ComponentRole::Muted),
                ],
            ),
        )
    }

    fn progress(&self, model: &Model) -> View {
        let complete = model.complete_count();
        let filled = "━".repeat(complete * 8);
        let empty = "─".repeat((STAGES.len() - complete) * 8);
        self.canvas_backdrop(View::row(
            VerticalAlign::Center,
            [
                self.canvas_text(filled, ComponentRole::Success),
                self.canvas_text(empty, ComponentRole::Muted),
                self.canvas_text(
                    format!("  {:>3}%", complete * 100 / STAGES.len()),
                    ComponentRole::Body,
                ),
            ],
        ))
    }
}

impl Application for ReleaseMonitor {
    type Model = Model;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (Model::new(), next_stage())
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Input(Input::Key(key))
                if key.kind != KeyKind::Release && requests_quit(key.code, key.modifiers) =>
            {
                Effect::shutdown()
            }
            Message::Input(Input::Key(key))
                if key.kind != KeyKind::Release
                    && matches!(key.code, KeyCode::Char('r') | KeyCode::Char(' ')) =>
            {
                *model = Model::new();
                next_stage()
            }
            Message::Input(_) => Effect::none(),
            Message::Advance if model.advance() => next_stage(),
            Message::Advance => Effect::none(),
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        let cards = self.canvas_backdrop(View::row(
            VerticalAlign::Top,
            model
                .stages
                .iter()
                .copied()
                .enumerate()
                .flat_map(|(index, state)| {
                    let mut views = vec![self.stage_card(index, state)];
                    if index + 1 < STAGES.len() {
                        views.push(self.canvas_space(" "));
                    }
                    views
                }),
        ));
        let status = if model.current == STAGES.len() {
            self.canvas_text("✓ Release candidate is ready", ComponentRole::Success)
        } else {
            self.canvas_text(
                format!("Running {}…", STAGES[model.current].0.to_lowercase()),
                ComponentRole::Accent,
            )
        };

        let content = View::column(
            Align::Left,
            [
                self.canvas_text("URUSHI / RELEASE MONITOR", ComponentRole::Accent),
                self.canvas_text(
                    "A TEA-style runtime coordinating effects, updates, and terminal drawing.",
                    ComponentRole::Muted,
                ),
                self.canvas_space(""),
                cards,
                self.canvas_space(""),
                self.progress(model),
                status,
                self.canvas_space(""),
                self.canvas_text("r / space restart  ·  q / esc quit", ComponentRole::Muted),
            ],
        );

        View::block(
            BlockStyle::new()
                .background(self.theme.tokens().background)
                .width(Length::fill(1))
                .height(Length::fill(1))
                .padding((2, 3)),
            content,
        )
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::input(Message::Input)
    }
}

fn requests_quit(code: KeyCode, modifiers: Modifiers) -> bool {
    matches!(code, KeyCode::Escape | KeyCode::Char('q'))
        || (code == KeyCode::Char('c') && modifiers.contains(Modifiers::CONTROL))
}

fn main() -> Result<(), Error> {
    let theme = ThemePreset::get("TokyoNight")
        .expect("the built-in TokyoNight theme is available")
        .theme();
    Runtime::new(ReleaseMonitor { theme })
        .keyboard_enhancement(None)
        .run()?;
    println!("Release monitor closed; terminal session restored.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Model, STAGES, StageState};

    #[test]
    fn advancing_completes_each_stage_in_order() {
        let mut model = Model::new();
        for completed in 1..=STAGES.len() {
            model.advance();
            assert_eq!(model.complete_count(), completed);
        }
        assert!(
            model
                .stages
                .iter()
                .all(|state| *state == StageState::Complete)
        );
        assert!(!model.advance());
    }
}
