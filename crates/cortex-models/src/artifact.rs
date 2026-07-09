//! Artifacts produced or captured during a run (files, logs, patches).

use chrono::{DateTime, Utc};
use cortex_common::{ArtifactId, SessionId};
use serde::{Deserialize, Serialize};

/// Kind of artifact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// A file on disk (relative path under workspace).
    File,
    /// A unified diff / patch.
    Diff,
    /// Free-form log blob.
    Log,
    /// Other / opaque.
    Other,
}

/// Metadata for a stored artifact (payload may live on disk or in DB).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// Artifact id.
    pub id: ArtifactId,
    /// Owning session.
    pub session_id: SessionId,
    /// Kind.
    pub kind: ArtifactKind,
    /// Relative path or logical name.
    pub name: String,
    /// Optional content hash (e.g. sha256 hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Optional byte size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Artifact {
    /// Create a file artifact reference.
    pub fn file(session_id: SessionId, name: impl Into<String>) -> Self {
        Self {
            id: ArtifactId::new(),
            session_id,
            kind: ArtifactKind::File,
            name: name.into(),
            sha256: None,
            size_bytes: None,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_roundtrip() {
        let a = Artifact::file(SessionId::new(), "src/lib.rs");
        let raw = serde_json::to_string(&a).unwrap();
        let back: Artifact = serde_json::from_str(&raw).unwrap();
        assert_eq!(a, back);
        assert_eq!(back.kind, ArtifactKind::File);
    }
}
