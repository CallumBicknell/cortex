//! Cortex Events
//!
//! Lifecycle events live in `cortex-core`. Agent-loop events are defined here.
//! Bus primitives are re-exported for convenience.

#![deny(missing_docs)]

mod agent;

pub use agent::{
    AssistantMessageProduced, CheckpointSaved, ErrorRaised, LoopPhase, LoopPhaseChanged,
    MessageAppended, PlanUpdated, SubAgentFinished, SubAgentStarted, ToolCallCompleted,
    ToolCallFailed, ToolCallRequested, UserMessageReceived,
};

pub use cortex_core::{
    EnvelopeHandler, Event, EventBus, EventEnvelope, EventHandler, InMemoryEventBus, KernelStarted,
    KernelStopped, LoopIterationCompleted, LoopIterationStarted, SubscriptionId,
};
