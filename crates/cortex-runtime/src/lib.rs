//! Cortex Runtime
//!
//! Process kernel facade plus the **agent loop** that drives:
//! Observe → Plan (LLM) → Execute tools → Verify → Reflect → Done.

#![deny(missing_docs)]

mod agent_loop;
mod context;
mod error;
mod runtime;

pub use agent_loop::{AgentLoop, AgentLoopConfig, RunInput, RunOutput};
pub use context::{ContextBuilder, DEFAULT_SYSTEM_PROMPT};
pub use error::{Result, RuntimeError};
pub use runtime::Runtime;

// Re-export loop phase for callers.
pub use cortex_events::LoopPhase;
