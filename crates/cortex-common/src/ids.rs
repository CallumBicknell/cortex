//! Strongly typed identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Generate a new random id.
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Wrap an existing UUID.
            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Borrow the inner UUID.
            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            /// Consume and return the inner UUID.
            pub fn into_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

define_id!(
    /// Identifies a durable agent session.
    SessionId
);
define_id!(
    /// Identifies a single agent run within a session (one user task).
    RunId
);
define_id!(
    /// Identifies a chat/tool message.
    MessageId
);
define_id!(
    /// Identifies a tool invocation.
    ToolCallId
);
define_id!(
    /// Identifies a plan artifact.
    PlanId
);
define_id!(
    /// Identifies a stored artifact (file snapshot, log, …).
    ArtifactId
);
define_id!(
    /// Identifies a checkpoint of loop state.
    CheckpointId
);
define_id!(
    /// Identifies a correlation group of events (often a run or turn).
    CorrelationId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_serde_and_parse() {
        let id = SessionId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);

        let parsed: SessionId = id.to_string().parse().unwrap();
        assert_eq!(id, parsed);
    }
}
