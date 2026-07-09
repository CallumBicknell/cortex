//! Kernel: lifecycle, event bus ownership, and service registry.

use crate::bus::{EventBus, InMemoryEventBus, SubscriptionId};
use crate::config::Config;
use crate::event::{EnvelopeHandler, Event};
use crate::lifecycle::LifecycleState;
use crate::lifecycle_events::{KernelStarted, KernelStopped};
use crate::service_registry::ServiceRegistry;
use std::any::Any;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};

/// Errors from kernel operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// Kernel is already running (or starting).
    AlreadyStarted,
    /// Kernel is not in a startable state.
    InvalidState(LifecycleState),
    /// Publishing an event failed.
    Publish(String),
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyStarted => write!(f, "kernel already started"),
            Self::InvalidState(s) => write!(f, "invalid kernel state: {s:?}"),
            Self::Publish(msg) => write!(f, "failed to publish event: {msg}"),
        }
    }
}

impl std::error::Error for KernelError {}

/// Result of a health check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheck {
    /// Status string (e.g. `"ok"`, `"failed"`).
    pub status: String,
    /// Lifecycle state at check time.
    pub state: LifecycleState,
    /// Uptime in seconds since start (0 if never started).
    pub uptime_secs: u64,
    /// Heartbeat / iteration count.
    pub iteration_count: u64,
}

/// Kernel that owns lifecycle, the event bus, and a service registry.
pub struct Kernel {
    config: Config,
    event_bus: Arc<InMemoryEventBus>,
    state: RwLock<LifecycleState>,
    iteration_counter: AtomicU64,
    shutdown_tx: broadcast::Sender<()>,
    service_registry: RwLock<ServiceRegistry>,
    started_at: RwLock<Option<Instant>>,
    /// Optional failure reason when state is Failed.
    failure_reason: RwLock<Option<String>>,
}

impl Kernel {
    /// Create a new kernel with configuration loaded from the environment.
    pub fn new() -> Self {
        Self::with_config(Config::from_env())
    }

    /// Create a new kernel with the given configuration.
    pub fn with_config(config: Config) -> Self {
        let history = config.event_history_size;
        let (shutdown_tx, _) = broadcast::channel(16);
        Self {
            config,
            event_bus: Arc::new(InMemoryEventBus::new(history)),
            state: RwLock::new(LifecycleState::Created),
            iteration_counter: AtomicU64::new(0),
            shutdown_tx,
            service_registry: RwLock::new(ServiceRegistry::new()),
            started_at: RwLock::new(None),
            failure_reason: RwLock::new(None),
        }
    }

    /// Kernel configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Shared handle to the event bus.
    pub fn event_bus(&self) -> Arc<InMemoryEventBus> {
        Arc::clone(&self.event_bus)
    }

    /// Current lifecycle state.
    pub async fn state(&self) -> LifecycleState {
        *self.state.read().await
    }

    /// Synchronous state accessor for simple callers (may block briefly).
    pub fn state_blocking(&self) -> LifecycleState {
        *self
            .state
            .try_read()
            .expect("kernel state lock poisoned or held")
    }

    /// Heartbeat / iteration counter.
    pub fn iteration_count(&self) -> u64 {
        self.iteration_counter.load(Ordering::Relaxed)
    }

    /// Whether the kernel is in the Running state.
    pub async fn is_started(&self) -> bool {
        self.state().await.is_running()
    }

    /// Whether the kernel has stopped cleanly.
    pub async fn is_stopped(&self) -> bool {
        matches!(self.state().await, LifecycleState::Stopped)
    }

    /// Whether the kernel is in the Failed state.
    pub async fn has_failed(&self) -> bool {
        matches!(self.state().await, LifecycleState::Failed)
    }

    /// Failure reason if any.
    pub async fn failure_reason(&self) -> Option<String> {
        self.failure_reason.read().await.clone()
    }

    /// Uptime in seconds since start, or 0.
    pub async fn uptime_secs(&self) -> u64 {
        self.started_at
            .read()
            .await
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Snapshot health information.
    pub async fn health_check(&self) -> HealthCheck {
        let state = self.state().await;
        let status = match state {
            LifecycleState::Running => "ok",
            LifecycleState::Failed => "failed",
            LifecycleState::Created => "created",
            LifecycleState::Starting => "starting",
            LifecycleState::Stopping => "stopping",
            LifecycleState::Stopped => "stopped",
        }
        .to_string();
        HealthCheck {
            status,
            state,
            uptime_secs: self.uptime_secs().await,
            iteration_count: self.iteration_count(),
        }
    }

    /// Register a service instance.
    pub async fn register_service<S: Any + Send + Sync>(&self, service: S) {
        self.service_registry.write().await.register(service);
    }

    /// Get a registered service.
    pub async fn get_service<S: Any + Send + Sync>(&self) -> Option<Arc<S>> {
        self.service_registry.read().await.get::<S>()
    }

    /// Remove a registered service.
    pub async fn deregister_service<S: Any + Send + Sync>(&self) -> bool {
        self.service_registry.write().await.deregister::<S>()
    }

    /// Returns true if a service of type `S` is registered.
    pub async fn service_exists<S: Any + Send + Sync>(&self) -> bool {
        self.service_registry.read().await.exists::<S>()
    }

    /// Number of registered services.
    pub async fn service_count(&self) -> usize {
        self.service_registry.read().await.len()
    }

    /// Subscribe a handler to the event bus.
    pub async fn subscribe(&self, handler: Arc<dyn EnvelopeHandler>) -> SubscriptionId {
        self.event_bus.subscribe(handler).await
    }

    /// Publish a typed event.
    pub async fn publish<E: Event + serde::Serialize>(&self, event: E) {
        self.event_bus.publish(event).await;
    }

    /// Signal the kernel to stop. The running `start` future will exit.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Start the kernel and wait until stop is signaled.
    ///
    /// Lifecycle: `Created|Stopped` → `Starting` → `Running` → (wait) → `Stopping` → `Stopped`.
    ///
    /// This is **not** the agent loop. It only keeps the runtime process alive and
    /// emits lifecycle events. Agent work is scheduled separately (Phase 4).
    pub async fn start(&self) -> Result<(), KernelError> {
        {
            let mut state = self.state.write().await;
            match *state {
                LifecycleState::Created | LifecycleState::Stopped => {
                    *state = LifecycleState::Starting;
                }
                LifecycleState::Running | LifecycleState::Starting => {
                    return Err(KernelError::AlreadyStarted);
                }
                other => return Err(KernelError::InvalidState(other)),
            }
        }

        *self.failure_reason.write().await = None;
        *self.started_at.write().await = Some(Instant::now());

        self.event_bus.publish(KernelStarted::new()).await;

        *self.state.write().await = LifecycleState::Running;

        let mut shutdown_rx = self.shutdown_tx.subscribe();

        // Stay running until stop is requested. Optional heartbeat if interval > 0.
        loop {
            let sleep = tokio::time::sleep(Duration::from_millis(self.config.loop_interval_ms));
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = sleep => {
                    // Heartbeat tick — reserved for future metrics. Not the agent loop.
                    self.iteration_counter.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        *self.state.write().await = LifecycleState::Stopping;
        self.event_bus
            .publish(KernelStopped::new(Some(
                "Shutdown signal received".to_string(),
            )))
            .await;
        *self.state.write().await = LifecycleState::Stopped;
        Ok(())
    }

    /// Mark the kernel as failed with a reason.
    pub async fn fail(&self, reason: impl Into<String>) {
        let reason = reason.into();
        tracing::error!(%reason, "kernel failed");
        *self.failure_reason.write().await = Some(reason);
        *self.state.write().await = LifecycleState::Failed;
        let _ = self.shutdown_tx.send(());
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{EnvelopeHandler, EventEnvelope};
    use async_trait::async_trait;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Mutex;

    struct KindRecorder {
        kinds: Mutex<Vec<String>>,
        count: AtomicUsize,
    }

    #[async_trait]
    impl EnvelopeHandler for KindRecorder {
        async fn handle(&self, event: EventEnvelope) {
            self.count.fetch_add(1, Ordering::SeqCst);
            self.kinds.lock().unwrap().push(event.kind);
        }
    }

    #[tokio::test]
    async fn start_stop_emits_lifecycle_events() {
        let kernel = Kernel::with_config(Config {
            loop_interval_ms: 10,
            log_level: "info".into(),
            event_history_size: 32,
        });
        let handler = Arc::new(KindRecorder {
            kinds: Mutex::new(Vec::new()),
            count: AtomicUsize::new(0),
        });
        kernel.subscribe(handler.clone()).await;

        let kernel = Arc::new(kernel);
        let k2 = Arc::clone(&kernel);
        let run = tokio::spawn(async move { k2.start().await });

        // Wait until running.
        for _ in 0..50 {
            if kernel.is_started().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(kernel.is_started().await);

        kernel.stop();
        run.await.expect("join").expect("start ok");

        assert!(kernel.is_stopped().await);
        let kinds = handler.kinds.lock().unwrap().clone();
        assert!(kinds.iter().any(|k| k == "kernel.started"));
        assert!(kinds.iter().any(|k| k == "kernel.stopped"));
    }

    #[tokio::test]
    async fn double_start_fails() {
        let kernel = Arc::new(Kernel::with_config(Config {
            loop_interval_ms: 50,
            log_level: "info".into(),
            event_history_size: 8,
        }));
        let k2 = Arc::clone(&kernel);
        let run = tokio::spawn(async move { k2.start().await });
        for _ in 0..50 {
            if kernel.is_started().await {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let err = kernel.start().await.unwrap_err();
        assert_eq!(err, KernelError::AlreadyStarted);
        kernel.stop();
        let _ = run.await;
    }

    #[tokio::test]
    async fn service_registry_roundtrip() {
        let kernel = Kernel::with_config(Config::default());
        kernel.register_service(String::from("hello")).await;
        assert!(kernel.service_exists::<String>().await);
        let value = kernel.get_service::<String>().await.unwrap();
        assert_eq!(value.as_str(), "hello");
        assert!(kernel.deregister_service::<String>().await);
        assert!(!kernel.service_exists::<String>().await);
    }

    #[tokio::test]
    async fn health_check_reports_state() {
        let kernel = Kernel::with_config(Config::default());
        let health = kernel.health_check().await;
        assert_eq!(health.state, LifecycleState::Created);
        assert_eq!(health.status, "created");
    }
}
