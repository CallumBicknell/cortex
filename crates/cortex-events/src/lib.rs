//! Cortex Events
//!
//! Re-exports core event types and bus primitives. Agent-specific events (tool calls,
//! plan phases, etc.) will be defined here in Phase 1+.

#![deny(missing_docs)]

pub use cortex_core::{
    EnvelopeHandler, Event, EventBus, EventEnvelope, EventHandler, InMemoryEventBus, KernelStarted,
    KernelStopped, LoopIterationCompleted, LoopIterationStarted, SubscriptionId,
};
