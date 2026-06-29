#![deny(missing_docs)]

//! Cortex Core Kernel
//!
//! This crate contains the core kernel, event bus traits, and basic types.

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::broadcast::{self};
use tokio::time;
use tracing_subscriber::{fmt, EnvFilter};

/// Type alias for an event handler box.
type HandlerBox =
    Box<dyn Fn(Arc<dyn Event>) -> Pin<Box<dyn Future<Output = ()> + Send + Sync>> + Send + Sync>;

/// Configuration for the kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Interval between loop iterations in milliseconds.
    pub loop_interval_ms: u64,
    /// Log level for tracing.
    pub log_level: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            loop_interval_ms: 100,
            log_level: "info".to_string(),
        }
    }
}

impl Config {
    /// Load configuration from environment variables with fallback to defaults.
    ///
    /// Environment variables:
    ///   - CORTEX_LOOP_INTERVAL_MS: loop interval in milliseconds
    ///   - CORTEX_LOG_LEVEL: tracing log level (e.g., "info", "debug")
    pub fn from_env() -> Self {
        let loop_interval_ms = std::env::var("CORTEX_LOOP_INTERVAL_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(Self::default_loop_interval_ms);
        let log_level = std::env::var("CORTEX_LOG_LEVEL").unwrap_or_else(|_| "info".to_string());

        Self {
            loop_interval_ms,
            log_level,
        }
    }

    fn default_loop_interval_ms() -> u64 {
        100
    }
}

/// Lifecycle states of the kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// Kernel has been created but not started.
    Created,
    /// Kernel is starting.
    Starting,
    /// Kernel is running.
    Running,
    /// Kernel is stopping.
    Stopping,
    /// Kernel has been stopped.
    Stopped,
    /// Kernel has encountered a fatal error.
    Failed,
}

/// A trait for events that can be sent through the event bus.
pub trait Event: Debug + Send + Sync + 'static {}

/// A simple event bus trait.
#[async_trait]
pub trait EventBus: Send + Sync {
    /// Publish an event.
    async fn publish(
        &self,
        event: Arc<dyn Event>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    /// Subscribe to events.
    fn subscribe(&self, handler: HandlerBox);
}

/// A kernel that manages the event bus and services.
pub struct Kernel {
    /// Configuration.
    config: Config,
    /// The event bus.
    event_bus: InMemoryEventBus,
    /// Current lifecycle state.
    state: RwLock<LifecycleState>,
    /// Iteration counter.
    iteration_counter: AtomicU64,
    /// Shutdown signal broadcaster.
    shutdown_tx: broadcast::Sender<()>,
    /// Shutdown signal receiver (cloned for internal use).
    shutdown_rx: broadcast::Receiver<()>,
}

impl Kernel {
    /// Create a new kernel with default configuration.
    pub fn new() -> Self {
        // Initialize tracing if not already done.
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        fmt().with_env_filter(filter).init();

        let (shutdown_tx, shutdown_rx) = broadcast::channel(16);
        let event_bus = InMemoryEventBus::new();
        // Add a default logger subscriber for all events (optional).
        event_bus.subscribe(Box::new(|_event| {
            Box::pin(async {
                tracing::trace!("Event received");
            })
        }));
        Self {
            config: Config::from_env(),
            event_bus,
            state: RwLock::new(LifecycleState::Created),
            iteration_counter: AtomicU64::new(0),
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Create a new kernel with the given configuration.
    pub fn with_config(config: Config) -> Self {
        // Initialize tracing if not already done.
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        fmt().with_env_filter(filter).init();

        let (shutdown_tx, shutdown_rx) = broadcast::channel(16);
        let event_bus = InMemoryEventBus::new();
        // Add a default logger subscriber for all events (optional).
        event_bus.subscribe(Box::new(|_event| {
            Box::pin(async {
                tracing::trace!("Event received");
            })
        }));
        Self {
            config,
            event_bus,
            state: RwLock::new(LifecycleState::Created),
            iteration_counter: AtomicU64::new(0),
            shutdown_tx,
            shutdown_rx,
        }
    }

    /// Get the current lifecycle state.
    pub fn state(&self) -> LifecycleState {
        *self
            .state
            .read()
            .expect("Failed to read lock on kernel state")
    }

    /// Get the current iteration count.
    pub fn iteration_count(&self) -> u64 {
        self.iteration_counter.load(Ordering::Relaxed)
    }

    /// Start the kernel.
    ///
    /// This transitions the kernel through the lifecycle:
    /// Created -> Starting -> Running -> (loop) -> Stopping -> Stopped.
    pub async fn start(&mut self) {
        // Transition to Starting
        *self
            .state
            .write()
            .expect("Failed to write lock on kernel state") = LifecycleState::Starting;
        // Publish KernelStarted event.
        let start_event = KernelStarted::new();
        if let Err(e) = self.event_broadcast(start_event).await {
            tracing::error!("Failed to publish KernelStarted event: {e}");
            *self
                .state
                .write()
                .expect("Failed to write lock on kernel state") = LifecycleState::Failed;
            return;
        }

        // Transition to Running
        *self
            .state
            .write()
            .expect("Failed to write lock on kernel state") = LifecycleState::Running;

        // Main loop
        loop {
            // Check for shutdown signal
            if let Ok(()) = self.shutdown_rx.try_recv() {
                break;
            }

            // Increment iteration counter.
            let iteration = self.iteration_counter.fetch_add(1, Ordering::SeqCst) + 1;

            // Publish LoopIterationStarted event.
            let iter_start = LoopIterationStarted::new(iteration);
            if let Err(e) = self.event_broadcast(iter_start).await {
                tracing::error!("Failed to publish LoopIterationStarted event: {e}");
                *self
                    .state
                    .write()
                    .expect("Failed to write lock on kernel state") = LifecycleState::Failed;
                break;
            }

            // Sleep for the configured interval.
            let sleep_duration = Duration::from_millis(self.config.loop_interval_ms);
            tokio::select! {
                _ = time::sleep(sleep_duration) => {},
                _ = self.shutdown_rx.recv() => {
                    break;
                }
            }

            // Publish LoopIterationCompleted event.
            let iter_end = LoopIterationCompleted::new(
                iteration,
                self.config.loop_interval_ms as f64 / 1000.0, // duration in seconds
            );
            if let Err(e) = self.event_broadcast(iter_end).await {
                tracing::error!("Failed to publish LoopIterationCompleted event: {e}");
                *self
                    .state
                    .write()
                    .expect("Failed to write lock on kernel state") = LifecycleState::Failed;
                break;
            }
        }

        // Transition to Stopping
        *self
            .state
            .write()
            .expect("Failed to write lock on kernel state") = LifecycleState::Stopping;
        // Publish KernelStopped event.
        let stop_event = KernelStopped::new(Some("Shutdown signal received".to_string()));
        if let Err(e) = self.event_broadcast(stop_event).await {
            tracing::error!("Failed to publish KernelStopped event: {e}");
            *self
                .state
                .write()
                .expect("Failed to write lock on kernel state") = LifecycleState::Failed;
        } else {
            *self
                .state
                .write()
                .expect("Failed to write lock on kernel state") = LifecycleState::Stopped;
        }
    }

    /// Stop the kernel.
    ///
    /// This sends a shutdown signal to break the loop.
    pub fn stop(&self) {
        // Send a shutdown signal (we ignore errors because if there are no receivers, it's fine).
        let _ = self.shutdown_tx.send(());
        // Note: The loop will break on the next iteration when it receives the signal.
        // We do not wait for the loop to finish here; if you want to wait, you would need
        // to join a task or use a different synchronization mechanism.
        // For simplicity, we just signal and let the loop stop on its own.
    }

    /// Helper to broadcast an event.
    async fn event_broadcast<E: Event>(
        &self,
        event: E,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let event_arc = Arc::new(event);
        self.event_bus.publish(event_arc).await?;
        Ok(())
    }
}

impl Default for Kernel {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple in-memory event bus for testing.
#[derive(Default)]
pub struct InMemoryEventBus {
    /// Handlers for events.
    handlers: RwLock<Vec<HandlerBox>>,
}

impl InMemoryEventBus {
    /// Create a new in-memory event bus.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EventBus for InMemoryEventBus {
    async fn publish(
        &self,
        event: Arc<dyn Event>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Get the number of handlers while holding a read lock.
        let len = {
            let guard = self.handlers.read().unwrap();
            guard.len()
        };
        // Iterate over each handler index, acquiring a read lock for each.
        for i in 0..len {
            let fut = {
                let guard = self.handlers.read().unwrap();
                let handler = &guard[i];
                handler(event.clone())
            };
            fut.await;
        }
        Ok(())
    }

    fn subscribe(&self, handler: HandlerBox) {
        // Acquire write lock on the handlers vector.
        let mut handlers_guard = self.handlers.write().unwrap();
        // Push the handler.
        handlers_guard.push(handler);
    }
}

/// An event indicating that the kernel has started.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStarted {
    /// Unique identifier for this kernel instance.
    pub instance_id: uuid::Uuid,
    /// Timestamp when the kernel started.
    pub timestamp: chrono::DateTime<Utc>,
}

impl KernelStarted {
    /// Create a new KernelStarted event.
    pub fn new() -> Self {
        Self {
            instance_id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
        }
    }
}

impl Default for KernelStarted {
    fn default() -> Self {
        Self::new()
    }
}

impl Event for KernelStarted {}

/// An event indicating that the kernel has stopped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelStopped {
    /// Unique identifier for this event.
    pub id: uuid::Uuid,
    /// Timestamp when the event was created.
    pub timestamp: chrono::DateTime<Utc>,
    /// Optional reason for stopping.
    pub reason: Option<String>,
}

impl KernelStopped {
    /// Create a new KernelStopped event.
    pub fn new(reason: Option<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            reason,
        }
    }
}

impl Default for KernelStopped {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Event for KernelStopped {}

/// An event indicating the start of a loop iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIterationStarted {
    /// Unique identifier for this event.
    pub id: uuid::Uuid,
    /// Timestamp when the event was created.
    pub timestamp: chrono::DateTime<Utc>,
    /// Iteration number.
    pub iteration: u64,
}

impl LoopIterationStarted {
    /// Create a new loop iteration started event.
    pub fn new(iteration: u64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            iteration,
        }
    }
}

impl Default for LoopIterationStarted {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Event for LoopIterationStarted {}

/// An event indicating the completion of a loop iteration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopIterationCompleted {
    /// Unique identifier for this event.
    pub id: uuid::Uuid,
    /// Timestamp when the event was created.
    pub timestamp: chrono::DateTime<Utc>,
    /// Iteration number.
    pub iteration: u64,
    /// Duration of the interval in seconds.
    pub duration_seconds: f64,
}

impl LoopIterationCompleted {
    /// Create a new loop iteration completed event.
    pub fn new(iteration: u64, duration_seconds: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            iteration,
            duration_seconds,
        }
    }
}

impl Default for LoopIterationCompleted {
    fn default() -> Self {
        Self::new(0, 0.0)
    }
}

impl Event for LoopIterationCompleted {}
