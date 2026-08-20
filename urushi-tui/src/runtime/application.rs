//! What a full-screen application is.

use urushi::View;

use super::effect::Effect;
use super::subscription::Subscription;

/// A full-screen terminal application, as a value that describes a program
/// rather than the program's running state.
///
/// The implementing type holds what the program is made of — configuration, a
/// theme, the paths it operates on — and [`Model`](Application::Model) holds
/// what changes while it runs. The runtime owns the live model: it holds a
/// `Model`, borrows the `Application`, lends the model to
/// [`update`](Application::update) mutably one message at a time, lends it to
/// [`view`](Application::view) and [`subscriptions`](Application::subscriptions)
/// immutably, and returns it when the application shuts down. Nothing here
/// reads a terminal, spawns a task, or draws.
///
/// That split is what makes the four methods testable without a terminal. A
/// test builds the application, calls `init` for a model and its first effect,
/// drives `update` with messages of its own, and reads `view` as a value:
///
/// ```
/// use urushi::{TextStyle, View};
/// use urushi_tui::{Application, Effect, Subscription};
///
/// struct Counter;
///
/// enum Message {
///     Increment,
/// }
///
/// impl Application for Counter {
///     type Model = i32;
///     type Message = Message;
///
///     fn init(&self) -> (Self::Model, Effect<Self::Message>) {
///         (0, Effect::none())
///     }
///
///     fn update(&self, model: &mut Self::Model, message: Self::Message)
///         -> Effect<Self::Message> {
///         match message {
///             Message::Increment => *model += 1,
///         }
///         Effect::none()
///     }
///
///     fn view(&self, model: &Self::Model) -> View {
///         View::text(model.to_string(), TextStyle::new())
///     }
///
///     fn subscriptions(&self, _model: &Self::Model) -> Subscription<Self::Message> {
///         Subscription::none()
///     }
/// }
///
/// let application = Counter;
/// let (mut model, _effect) = application.init();
/// application.update(&mut model, Message::Increment);
/// assert_eq!(model, 1);
/// ```
///
/// # Bounds
///
/// [`Message`](Application::Message) is `Send + 'static` because an effect
/// completes on another thread or task and must send its message back. `Model`
/// and `Self` carry no bound: the runtime keeps the model on the thread that
/// runs `update` and `view`, and the entry point that runs an application
/// blocks that thread rather than handing the model to another.
pub trait Application {
    /// The state the runtime owns while the application runs.
    type Model;

    /// What the application reacts to.
    type Message: Send + 'static;

    /// The initial model, and the work to start with it.
    fn init(&self) -> (Self::Model, Effect<Self::Message>);

    /// Applies one message to the model and returns the work it asks for.
    ///
    /// This is the only method that may change the model, which is what the
    /// receivers say: `update` takes it mutably and every other method takes it
    /// immutably. Returning [`Effect::shutdown`] here ends the run.
    fn update(&self, model: &mut Self::Model, message: Self::Message) -> Effect<Self::Message>;

    /// The view of the current model.
    ///
    /// It receives no surface or rendering-environment input: an application
    /// that needs such a fact subscribes to it, stores what it needs in the
    /// model, and reads it here like any other state.
    fn view(&self, model: &Self::Model) -> View;

    /// The sources the application wants to hear from in this state.
    ///
    /// The runtime reconciles this against what it is running after each
    /// `update`, so a source appears by being declared and stops by no longer
    /// being declared.
    fn subscriptions(&self, model: &Self::Model) -> Subscription<Self::Message>;
}

#[cfg(test)]
#[path = "application_tests.rs"]
mod tests;
