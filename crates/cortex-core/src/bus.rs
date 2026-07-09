//! Event bus trait and in-memory implementation.

use crate::event::{EnvelopeHandler, Event, EventEnvelope};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Identifier for a bus subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionId(u64);

impl SubscriptionId {
    /// Raw numeric id.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// Publish/subscribe bus for runtime events.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish a typed event (serialized to an envelope).
    async fn publish<E: Event + serde::Serialize>(&self, event: E);

    /// Publish a pre-built envelope.
    async fn publish_envelope(&self, envelope: EventEnvelope);

    /// Subscribe to all envelopes. Returns a subscription id for unsubscribe.
    async fn subscribe(&self, handler: Arc<dyn EnvelopeHandler>) -> SubscriptionId;

    /// Remove a subscription.
    async fn unsubscribe(&self, id: SubscriptionId) -> bool;

    /// Number of active subscribers.
    async fn subscriber_count(&self) -> usize;

    /// Snapshot of retained history (oldest first).
    async fn history(&self) -> Vec<EventEnvelope>;

    /// Replay events at or after the given id (inclusive if present).
    async fn replay_since(&self, id: Uuid) -> Vec<EventEnvelope>;

    /// Clear retained history.
    async fn clear_history(&self);

    /// Current history length.
    async fn history_len(&self) -> usize;
}

struct Subscription {
    id: SubscriptionId,
    handler: Arc<dyn EnvelopeHandler>,
}

/// In-memory event bus with ring-buffer history.
pub struct InMemoryEventBus {
    subscribers: RwLock<Vec<Subscription>>,
    history: RwLock<VecDeque<EventEnvelope>>,
    history_capacity: usize,
    next_sub_id: AtomicU64,
}

impl InMemoryEventBus {
    /// Create a bus with the given history capacity (minimum 1).
    pub fn new(history_capacity: usize) -> Self {
        Self {
            subscribers: RwLock::new(Vec::new()),
            history: RwLock::new(VecDeque::new()),
            history_capacity: history_capacity.max(1),
            next_sub_id: AtomicU64::new(1),
        }
    }

    async fn retain(&self, envelope: &EventEnvelope) {
        let mut history = self.history.write().await;
        if history.len() >= self.history_capacity {
            history.pop_front();
        }
        history.push_back(envelope.clone());
    }

    async fn dispatch(&self, envelope: EventEnvelope) {
        self.retain(&envelope).await;
        let subscribers: Vec<Arc<dyn EnvelopeHandler>> = {
            let guard = self.subscribers.read().await;
            guard.iter().map(|s| Arc::clone(&s.handler)).collect()
        };
        // Handlers are invoked sequentially. A handler should not panic; panics still
        // unwind the calling task (true isolation can be added later via futures::FutureExt).
        for handler in subscribers {
            handler.handle(envelope.clone()).await;
        }
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish<E: Event + serde::Serialize>(&self, event: E) {
        let envelope = EventEnvelope::from_typed(event);
        self.publish_envelope(envelope).await;
    }

    async fn publish_envelope(&self, envelope: EventEnvelope) {
        self.dispatch(envelope).await;
    }

    async fn subscribe(&self, handler: Arc<dyn EnvelopeHandler>) -> SubscriptionId {
        let id = SubscriptionId(self.next_sub_id.fetch_add(1, Ordering::SeqCst));
        self.subscribers
            .write()
            .await
            .push(Subscription { id, handler });
        id
    }

    async fn unsubscribe(&self, id: SubscriptionId) -> bool {
        let mut subs = self.subscribers.write().await;
        let before = subs.len();
        subs.retain(|s| s.id != id);
        before != subs.len()
    }

    async fn subscriber_count(&self) -> usize {
        self.subscribers.read().await.len()
    }

    async fn history(&self) -> Vec<EventEnvelope> {
        self.history.read().await.iter().cloned().collect()
    }

    async fn replay_since(&self, id: Uuid) -> Vec<EventEnvelope> {
        let history = self.history.read().await;
        if let Some(pos) = history.iter().position(|e| e.id == id) {
            history.iter().skip(pos).cloned().collect()
        } else {
            // Id not found: return empty rather than replaying everything unexpectedly.
            Vec::new()
        }
    }

    async fn clear_history(&self) {
        self.history.write().await.clear();
    }

    async fn history_len(&self) -> usize {
        self.history.read().await.len()
    }
}

impl Default for InMemoryEventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EnvelopeHandler;
    use crate::lifecycle_events::KernelStarted;
    use async_trait::async_trait;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    struct RecordingHandler {
        count: AtomicUsize,
        kinds: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl EnvelopeHandler for RecordingHandler {
        async fn handle(&self, event: EventEnvelope) {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.kinds.lock().unwrap().push(event.kind);
        }
    }

    #[tokio::test]
    async fn publish_delivers_to_subscriber() {
        let bus = InMemoryEventBus::new(16);
        let handler = Arc::new(RecordingHandler {
            count: AtomicUsize::new(0),
            kinds: Mutex::new(Vec::new()),
        });
        bus.subscribe(handler.clone()).await;
        bus.publish(KernelStarted::new()).await;
        assert_eq!(handler.count.load(Ordering::SeqCst), 1);
        assert_eq!(handler.kinds.lock().unwrap().as_slice(), ["kernel.started"]);
        assert_eq!(bus.history_len().await, 1);
    }

    #[tokio::test]
    async fn history_rings() {
        let bus = InMemoryEventBus::new(2);
        bus.publish(KernelStarted::new()).await;
        bus.publish(KernelStarted::new()).await;
        bus.publish(KernelStarted::new()).await;
        assert_eq!(bus.history_len().await, 2);
    }

    #[tokio::test]
    async fn unsubscribe_stops_delivery() {
        let bus = InMemoryEventBus::new(8);
        let handler = Arc::new(RecordingHandler {
            count: AtomicUsize::new(0),
            kinds: Mutex::new(Vec::new()),
        });
        let id = bus.subscribe(handler.clone()).await;
        assert!(bus.unsubscribe(id).await);
        bus.publish(KernelStarted::new()).await;
        assert_eq!(handler.count.load(Ordering::SeqCst), 0);
    }
}
