//! Workspace discovery for Cortex agents.
//!
//! Detects project roots, applies ignore rules, fingerprints the stack, and
//! builds compact repo maps for the context window.

#![deny(missing_docs)]

mod error;
mod ignore_rules;
mod instructions;
mod project;
mod repomap;
mod root;

pub use error::{Result, WorkspaceError};
pub use ignore_rules::list_files;
pub use instructions::{
    instruction_candidates, load_project_instructions, ProjectInstructions, MAX_INSTRUCTION_BYTES,
};
pub use project::ProjectInfo;
pub use repomap::RepoMap;
pub use root::{detect_root, is_under};
