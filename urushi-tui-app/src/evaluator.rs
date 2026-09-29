//! Runtime-owned selection of the core view evaluator.

use urushi::{Available, LayoutError, ResolvedView, Resolver, View, resolve};

/// The evaluator lifetime selected by the runtime host.
///
/// Direct evaluation is the default and retains no state. The retained variant
/// owns the opt-in core resolver across frames; the renderer borrows this value
/// for one operation and never exposes it to the application.
#[derive(Default)]
pub(crate) enum Evaluator {
    #[default]
    Direct,
    Retained(Box<Resolver>),
}

impl Evaluator {
    pub(crate) fn retained() -> Self {
        Self::Retained(Box::new(Resolver::new()))
    }

    pub(crate) fn resolve(
        &mut self,
        view: &View,
        available: Available,
    ) -> Result<ResolvedView, LayoutError> {
        match self {
            Self::Direct => resolve(view, available),
            Self::Retained(resolver) => resolver.resolve(view, available),
        }
    }
}
