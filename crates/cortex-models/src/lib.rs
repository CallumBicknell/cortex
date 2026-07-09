//! Domain models for Cortex agent sessions.
//!
//! These types are serializable and provider-agnostic. They form the shared
//! vocabulary for the runtime, memory store, and (later) SDKs.

#![deny(missing_docs)]

mod artifact;
mod message;
mod plan;
mod session;
mod task;
mod tool;

pub use artifact::{Artifact, ArtifactKind};
pub use message::{Message, Role};
pub use plan::{Plan, PlanStatus, PlanStep};
pub use session::{Session, SessionStatus, Turn};
pub use task::{Task, TaskStatus};
pub use tool::{ToolCall, ToolResult, ToolSpec};
