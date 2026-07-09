//! Durable memory for Cortex: SQLite sessions, messages, checkpoints, and events.

#![deny(missing_docs)]

mod checkpoint;
mod db;
mod error;
mod store;

pub use checkpoint::{Checkpoint, CheckpointState};
pub use db::{migrate, open_sqlite};
pub use error::{MemoryError, Result};
pub use store::{SessionStore, SessionSummary};
