//! Process runtime facade around the kernel.

use cortex_core::{Config, HealthCheck, Kernel, KernelError, LifecycleState};
use std::sync::Arc;

/// A runtime that owns a kernel instance.
pub struct Runtime {
    /// The kernel instance (shared so stop can be signaled from another task).
    kernel: Arc<Kernel>,
}

impl Runtime {
    /// Create a new runtime with configuration from the environment.
    pub fn new() -> Self {
        Self {
            kernel: Arc::new(Kernel::new()),
        }
    }

    /// Create a new runtime with the given configuration.
    pub fn with_config(config: Config) -> Self {
        Self {
            kernel: Arc::new(Kernel::with_config(config)),
        }
    }

    /// Shared kernel handle.
    pub fn kernel(&self) -> Arc<Kernel> {
        Arc::clone(&self.kernel)
    }

    /// Start the runtime (blocks until [`Self::stop`] is called from another task).
    pub async fn start(&self) -> Result<(), KernelError> {
        self.kernel.start().await
    }

    /// Signal the runtime to stop.
    pub fn stop(&self) {
        self.kernel.stop();
    }

    /// Current lifecycle state.
    pub async fn state(&self) -> LifecycleState {
        self.kernel.state().await
    }

    /// Heartbeat / iteration count.
    pub fn iteration_count(&self) -> u64 {
        self.kernel.iteration_count()
    }

    /// Health snapshot.
    pub async fn health_check(&self) -> HealthCheck {
        self.kernel.health_check().await
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn runtime_start_stop() {
        let rt = Arc::new(Runtime::with_config(Config {
            loop_interval_ms: 10,
            log_level: "info".into(),
            event_history_size: 16,
        }));
        let rt2 = Arc::clone(&rt);
        let handle = tokio::spawn(async move { rt2.start().await });

        for _ in 0..50 {
            if matches!(rt.state().await, LifecycleState::Running) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(matches!(rt.state().await, LifecycleState::Running));
        rt.stop();
        handle.await.unwrap().unwrap();
        assert!(matches!(rt.state().await, LifecycleState::Stopped));
    }
}
