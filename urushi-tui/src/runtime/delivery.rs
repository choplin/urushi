//! Admission before acceptance, followed by one runtime-wide delivery order.
//!
//! The design has two distinct stages:
//!
//! 1. [`inbox`] owns admission for application-defined sources. A message may
//!    wait under bounded FIFO backpressure or occupy a replaceable latest-value
//!    slot; every accepted message is [`Delivery::Async`]. The runtime-owned
//!    surface source instead uses [`surface`], its purpose-built latest slot.
//! 2. [`queue`] owns accepted deliveries. Moving a message from an inbox into
//!    this queue is one atomic operation and gives it its runtime-wide order.
//!
//! [`admission`] contains the public vocabulary an application-defined source
//! uses to select the first-stage policy and send messages. Neither stage
//! inspects an application's message variants.

mod admission;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by subscription reconciliation")
)]
mod inbox;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the effect executor and runtime core")
)]
mod queue;
#[cfg_attr(
    not(test),
    expect(dead_code, reason = "consumed by the runtime-owned surface source")
)]
mod surface;

pub use admission::{Admission, SendError, Sender};
#[cfg(test)]
pub(crate) use admission::{Policy, Sink};

#[cfg_attr(
    not(test),
    expect(unused_imports, reason = "consumed by subscription reconciliation")
)]
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use inbox::{Ready as SourceReady, SourceInbox, SourceInboxCloser, source_inbox};
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "consumed by the effect executor and runtime core"
    )
)]
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use queue::{
    CompletionOutcome, Delivery, DeliveryQueue, EffectCancellation, EffectCompletion,
    Next as NextDelivery,
};
#[cfg_attr(
    not(test),
    expect(
        unused_imports,
        reason = "consumed by the runtime-owned surface source"
    )
)]
#[cfg_attr(test, allow(unused_imports))]
pub(crate) use surface::{
    Ready as SurfaceReady, SurfaceMessagePublisher, SurfaceSlot, surface_slot,
};

#[cfg(test)]
#[path = "delivery_tests.rs"]
mod tests;
