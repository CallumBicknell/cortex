//! Security policy, secrets redaction, and approval auditing for Cortex.

#![deny(missing_docs)]

mod approver;
mod audit;
mod error;
mod harden;
mod policy;
mod redact;

pub use approver::PolicyApprover;
pub use audit::{AuditRecord, AuditSink, MemoryAuditSink, NullAuditSink};
pub use error::{Result, SecurityError};
pub use harden::{
    bubblewrap_available, bubblewrap_shell_prefix, path_has_parent_escape, reject_absolute_path,
    safe_join,
};
pub use policy::{default_scrub_env, SecurityPolicy};
pub use redact::{redact_json, redact_text, redacted_json};
