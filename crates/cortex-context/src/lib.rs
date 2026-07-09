//! Budgeted context assembly for Cortex.
//!
//! Combines system prompts, optional workspace/repo maps, and compressed
//! conversation history under an approximate token budget.

#![deny(missing_docs)]

mod builder;
mod history;
mod token;

pub use builder::{ContextBuilder, DEFAULT_SYSTEM_PROMPT};
pub use history::compress_history;
pub use token::{estimate_tokens, estimate_tokens_many};
