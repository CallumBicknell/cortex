//! Lifecycle-related event types.

use crate::event::Event;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An event indicating that the kernel has started.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelStarted {
    /// Unique identifier for this kernel instance.
    pub instance_id: Uuid,
    /// Timestamp when the kernel started.
    pub timestamp: DateTime<Utc>,
}

impl KernelStarted {
    /// Create a new `KernelStarted` event.
    pub fn new() -> Self {
        Self {
            instance_id: Uuid::new_v4(),
            timestamp: Utc::now(),
        }
    }
}

impl Default for KernelStarted {
    fn default() -> Self {
        Self::new()
    }
}

impl Event for KernelStarted {
    fn kind(&self) -> &'static str {
        "kernel.started"
    }
}

/// An event indicating that the kernel has stopped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KernelStopped {
    /// Unique identifier for this event.
    pub id: Uuid,
    /// Timestamp when the event was created.
    pub timestamp: DateTime<Utc>,
    /// Optional reason for stopping.
    pub reason: Option<String>,
}

impl KernelStopped {
    /// Create a new `KernelStopped` event.
    pub fn new(reason: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
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

impl Event for KernelStopped {
    fn kind(&self) -> &'static str {
        "kernel.stopped"
    }
}

/// An event indicating the start of a kernel heartbeat iteration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopIterationStarted {
    /// Unique identifier for this event.
    pub id: Uuid,
    /// Timestamp when the event was created.
    pub timestamp: DateTime<Utc>,
    /// Iteration number.
    pub iteration: u64,
}

impl LoopIterationStarted {
    /// Create a new loop iteration started event.
    pub fn new(iteration: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
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

impl Event for LoopIterationStarted {
    fn kind(&self) -> &'static str {
        "kernel.loop.started"
    }
}

/// An event indicating the completion of a kernel heartbeat iteration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoopIterationCompleted {
    /// Unique identifier for this event.
    pub id: Uuid,
    /// Timestamp when the event was created.
    pub timestamp: DateTime<Utc>,
    /// Iteration number.
    pub iteration: u64,
    /// Duration of the iteration in seconds.
    pub duration_seconds: f64,
}

impl LoopIterationCompleted {
    /// Create a new loop iteration completed event.
    pub fn new(iteration: u64, duration_seconds: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
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

impl Event for LoopIterationCompleted {
    fn kind(&self) -> &'static str {
        "kernel.loop.completed"
    }
}
