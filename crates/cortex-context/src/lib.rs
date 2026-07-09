//! Budgeted context assembly for Cortex.
//!
//! Combines system prompts, optional workspace/repo maps, and compressed
//! conversation history under an approximate token budget.

#![deny(missing_docs)]

mod builder;
mod history;
mod summary;
mod token;

pub use builder::{ContextBuilder, DEFAULT_SYSTEM_PROMPT};
pub use history::compress_history;
pub use summary::{
    apply_rolling_summary, extractive_summary, format_for_summary, needs_summary,
    split_for_summary, summary_system_prompt, summary_user_prompt, DEFAULT_SUMMARY_KEEP_RECENT,
    DEFAULT_SUMMARY_MESSAGE_THRESHOLD,
};
pub use token::{estimate_tokens, estimate_tokens_many};
