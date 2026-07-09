//! Shared primitives for Cortex crates.
//!
//! Keep this crate free of runtime, LLM, and tool dependencies so models and
//! events can depend on it without cycles.

#![deny(missing_docs)]

mod error;
mod ids;

pub use error::{CortexError, Result};
pub use ids::{
    ArtifactId, CheckpointId, CorrelationId, MessageId, PlanId, RunId, SessionId, ToolCallId,
};
