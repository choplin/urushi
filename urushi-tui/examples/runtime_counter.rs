use std::time::Duration;

use urushi::{Align, Color, TextStyle, View};
use urushi_tui::{
    Application, Effect, Error, Input, KeyCode, KeyKind, Modifiers, Subscription, Surface,
};

struct Counter;

#[derive(Default)]
struct Model {
    count: usize,
    last_completed: usize,
    pending: usize,
    surface: Surface,
}

enum Message {
    Input(Input),
    Surface(Surface),
    IncrementCompleted(usize),
}

impl Application for Counter {
    type Model = Model;
    type Message = Message;

    fn init(&self) -> (Self::Model, Effect<Self::Message>) {
        (Model::default(), Effect::none())
    }

    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message> {
        match message {
            Message::Surface(surface) => {
                model.surface = surface;
                Effect::none()
            }
            Message::Input(Input::Key(key))
                if key.kind != KeyKind::Release && requests_quit(key.code, key.modifiers) =>
            {
                Effect::shutdown()
            }
            Message::Input(Input::Key(key)) if key.kind != KeyKind::Release => {
                model.count += 1;
                model.pending += 1;
                let count = model.count;
                Effect::perform(move || {
                    std::thread::sleep(Duration::from_millis(150));
                    Message::IncrementCompleted(count)
                })
            }
            Message::Input(_) => Effect::none(),
            Message::IncrementCompleted(count) => {
                model.last_completed = model.last_completed.max(count);
                model.pending = model.pending.saturating_sub(1);
                Effect::none()
            }
        }
    }

    fn view(&self, model: &Self::Model) -> View {
        let heading = TextStyle::new().foreground(Color::BRIGHT_CYAN);
        let muted = TextStyle::new().foreground(Color::BRIGHT_BLACK);
        View::column(
            Align::Left,
            [
                View::text("Urushi runtime counter", heading),
                View::text(format!("key updates: {}", model.count), TextStyle::new()),
                View::text(
                    format!(
                        "effect completions: {} ({} pending)",
                        model.last_completed, model.pending
                    ),
                    TextStyle::new(),
                ),
                View::text(
                    format!(
                        "surface: {} x {} cells",
                        model.surface.size.columns(),
                        model.surface.size.rows()
                    ),
                    TextStyle::new(),
                ),
                View::text(
                    "Press any key to increment; q, Esc, or Ctrl-C quits.",
                    muted,
                ),
            ],
        )
    }

    fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
        Subscription::batch([
            Subscription::surface(Message::Surface),
            Subscription::input(Message::Input),
        ])
    }
}

fn requests_quit(code: KeyCode, modifiers: Modifiers) -> bool {
    matches!(code, KeyCode::Escape | KeyCode::Char('q'))
        || (code == KeyCode::Char('c') && modifiers.contains(Modifiers::CONTROL))
}

fn main() -> Result<(), Error> {
    let model = urushi_tui::run(Counter)?;
    println!("final count: {}", model.count);
    Ok(())
}
