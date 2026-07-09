//! Cortex Core Kernel
//!
//! Core lifecycle, configuration, service registry, and in-memory event bus.
//!
//! The agent loop (plan/execute tools) lives in `cortex-runtime` — this crate is the
//! process kernel only.

#![deny(missing_docs)]

mod bus;
mod config;
mod event;
mod kernel;
mod lifecycle;
mod lifecycle_events;
mod service_registry;

pub use bus::{EventBus, InMemoryEventBus, SubscriptionId};
pub use config::{Config, ConfigError};
pub use event::{EnvelopeHandler, Event, EventEnvelope, EventHandler};
pub use kernel::{HealthCheck, Kernel, KernelError};
pub use lifecycle::LifecycleState;
pub use lifecycle_events::{
    KernelStarted, KernelStopped, LoopIterationCompleted, LoopIterationStarted,
};
pub use service_registry::ServiceRegistry;
