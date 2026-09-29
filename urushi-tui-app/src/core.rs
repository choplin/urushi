//! The application loop that coordinates delivery and presentation.

use std::fmt;
use std::io;
use std::sync::Arc;

use super::application::Application;
use super::delivery::{Delivery, DeliveryQueue};
use super::effect::Mapper;
use super::executor::{
    Clock, EffectControl, EffectExecutor, Executor, SourceSpawner, SubscriptionExecutor,
};
use super::presentation::{DrawResult, Presentation};
use super::scheduler::{DEFAULT_MINIMUM_INTERVAL, DrawScheduler};

pub(crate) struct RuntimeCore<A: Application, P: Presentation> {
    application: A,
    model: A::Model,
    deliveries: DeliveryQueue<A::Message>,
    effects: EffectExecutor<A::Message>,
    subscriptions: SubscriptionExecutor<A::Message>,
    source_spawner: Arc<dyn SourceSpawner<A::Message>>,
    scheduler: DrawScheduler,
    presentation: P,
    terminal_error_mapper: Option<Mapper<io::Error, A::Message>>,
    stopping: bool,
}

#[derive(Debug)]
pub(crate) enum RuntimeError<E> {
    Terminal(io::Error),
    Presentation(E),
}

impl<E: fmt::Display> fmt::Display for RuntimeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Terminal(error) => write!(formatter, "terminal operation failed: {error}"),
            Self::Presentation(error) => write!(formatter, "presentation worker failed: {error}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for RuntimeError<E> {}

impl<A, P> RuntimeCore<A, P>
where
    A: Application,
    P: Presentation,
{
    pub(crate) fn new(
        application: A,
        executor: Arc<dyn Executor>,
        clock: Arc<dyn Clock>,
        source_spawner: Arc<dyn SourceSpawner<A::Message>>,
        presentation: P,
    ) -> Result<Self, RuntimeError<P::Error>> {
        let deliveries = DeliveryQueue::new();
        let (model, initial_effect) = application.init();
        let mut effects = EffectExecutor::new(executor, Arc::clone(&clock), deliveries.clone());
        let mut subscriptions =
            SubscriptionExecutor::new(Arc::clone(&source_spawner), deliveries.clone());
        let stopping = effects.start(initial_effect) == EffectControl::Shutdown;
        let terminal_error_mapper = if stopping {
            None
        } else {
            let subscription = application.subscriptions(&model);
            let mapper = subscription.terminal_error_mapper();
            subscriptions
                .reconcile(subscription)
                .map_err(RuntimeError::Terminal)?;
            mapper
        };
        let mut scheduler = DrawScheduler::new(DEFAULT_MINIMUM_INTERVAL, Arc::clone(&clock));
        if !stopping {
            scheduler.invalidate();
        }
        Ok(Self {
            application,
            model,
            deliveries,
            effects,
            subscriptions,
            source_spawner,
            scheduler,
            presentation,
            terminal_error_mapper,
            stopping,
        })
    }

    #[cfg(test)]
    pub(crate) fn deliveries(&self) -> DeliveryQueue<A::Message> {
        self.deliveries.clone()
    }

    pub(crate) fn process_delivery(
        &mut self,
        delivery: Delivery<A::Message>,
    ) -> Result<(), RuntimeError<P::Error>> {
        match delivery {
            Delivery::Async(message) => self.update(message)?,
            Delivery::Sync { first, rest } => {
                self.update(first)?;
                for message in rest {
                    if self.stopping {
                        break;
                    }
                    self.update(message)?;
                }
                self.deliveries.complete_sync();
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn is_stopping(&self) -> bool {
        self.stopping
    }

    pub(crate) async fn run(mut self) -> Result<A::Model, RuntimeError<P::Error>> {
        let outcome = loop {
            if self.stopping {
                break Ok(());
            }

            tokio::select! {
                biased;

                error = self.source_spawner.failure() => {
                    break Err(RuntimeError::Terminal(error));
                }
                _ = self.scheduler.next_draw(|| self.deliveries.sync_fence_allows_draw()) => {
                    let view = self.application.view(&self.model);
                    if let Err(error) = self.presentation.submit(view) {
                        break Err(RuntimeError::Presentation(error));
                    }
                }
                result = self.presentation.completed() => match result {
                    Ok(DrawResult::Completed) => self.scheduler.draw_completed_now(),
                    Ok(DrawResult::Failed { error }) => {
                        self.scheduler.draw_completed_now();
                        let Some(mapper) = self.terminal_error_mapper.as_ref() else {
                            break Err(RuntimeError::Terminal(error));
                        };
                        self.deliveries
                            .ordinary_completion()
                            .complete(mapper(error));
                    }
                    Err(error) => break Err(RuntimeError::Presentation(error)),
                },
                delivery = self.deliveries.next() => {
                    if let Err(error) = self.process_delivery(delivery) {
                        break Err(error);
                    }
                }
            }
        };

        self.effects.stop();
        self.subscriptions.stop();
        let shutdown = self
            .presentation
            .shutdown()
            .await
            .map_err(RuntimeError::Presentation);
        outcome?;
        shutdown?;
        Ok(self.model)
    }

    fn update(&mut self, message: A::Message) -> Result<(), RuntimeError<P::Error>> {
        let effect = self.application.update(&mut self.model, message);
        self.scheduler.invalidate();
        if self.effects.start(effect) == EffectControl::Shutdown {
            self.stopping = true;
            self.subscriptions.stop();
        } else {
            let subscription = self.application.subscriptions(&self.model);
            self.terminal_error_mapper = subscription.terminal_error_mapper();
            self.subscriptions
                .reconcile(subscription)
                .map_err(RuntimeError::Terminal)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
