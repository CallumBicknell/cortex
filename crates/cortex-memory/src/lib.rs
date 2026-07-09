//! Durable memory for Cortex: SQLite sessions, messages, checkpoints, events,
//! conversation summaries, and local vector embeddings.

#![deny(missing_docs)]

mod checkpoint;
mod db;
mod error;
mod store;
mod vector;

pub use checkpoint::{Checkpoint, CheckpointState};
pub use db::{migrate, open_sqlite};
pub use error::{MemoryError, Result};
pub use store::{SessionStore, SessionSummary};
pub use vector::{
    cosine_similarity, format_retrieval_section, local_embed, local_embed_dims, EmbeddingChunk,
    ScoredChunk, VectorStore, LOCAL_EMBED_DIMS, LOCAL_EMBED_MODEL,
};
