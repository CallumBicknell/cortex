//! Event model and handler traits.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Debug;
use uuid::Uuid;

/// Marker trait for typed events that can be published on the bus.
pub trait Event: Debug + Send + Sync + 'static {
    /// Stable kind string for this event type (e.g. `"kernel.started"`).
    fn kind(&self) -> &'static str;

    /// Serialize this event into an envelope for the bus.
    fn into_envelope(self) -> EventEnvelope
    where
        Self: Serialize + Sized,
    {
        EventEnvelope::from_typed(self)
    }
}

/// Envelope used as the unit of transport on the event bus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventEnvelope {
    /// Unique event identifier.
    pub id: Uuid,
    /// When the event was created (UTC).
    pub timestamp: DateTime<Utc>,
    /// Stable kind string (e.g. `"kernel.started"`).
    pub kind: String,
    /// Optional correlation id for relating events in a session/run.
    pub correlation_id: Option<Uuid>,
    /// Opaque JSON payload.
    pub payload: Value,
}

impl EventEnvelope {
    /// Create an envelope from a typed serializable event.
    pub fn from_typed<E>(event: E) -> Self
    where
        E: Event + Serialize,
    {
        let kind = event.kind().to_string();
        let payload = serde_json::to_value(&event).unwrap_or(Value::Null);
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            kind,
            correlation_id: None,
            payload,
        }
    }

    /// Attach a correlation id (builder style).
    pub fn with_correlation_id(mut self, id: Uuid) -> Self {
        self.correlation_id = Some(id);
        self
    }

    /// Deserialize the payload into a typed value.
    pub fn payload_as<T: for<'de> Deserialize<'de>>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

/// Handler for typed events.
#[async_trait]
pub trait EventHandler<E: Event>: Send + Sync {
    /// Handle an event.
    async fn handle(&self, event: E);
}

/// Handler for event envelopes (type-erased path used by the bus).
#[async_trait]
pub trait EnvelopeHandler: Send + Sync {
    /// Handle an event envelope.
    async fn handle(&self, event: EventEnvelope);
}
