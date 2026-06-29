//! Cortex Runtime
//!
//! This crate contains the runtime that manages the kernel, scheduler, and event loop.

#![deny(missing_docs)]

use cortex_core::{Config, Kernel};

/// A runtime that manages the kernel.
pub struct Runtime {
    /// The kernel instance.
    kernel: Kernel,
}

impl Runtime {
    /// Create a new runtime with default configuration.
    pub fn new() -> Self {
        Self {
            kernel: Kernel::new(),
        }
    }

    /// Create a new runtime with the given configuration.
    pub fn with_config(config: Config) -> Self {
        Self {
            kernel: Kernel::with_config(config),
        }
    }

    /// Start the runtime.
    ///
    /// This delegates to the kernel's start method.
    pub async fn start(&mut self) {
        self.kernel.start().await;
    }

    /// Stop the runtime.
    ///
    /// This delegates to the kernel's stop method.
    pub fn stop(&self) {
        self.kernel.stop();
    }

    /// Get a reference to the kernel.
    ///
    /// This can be used for advanced operations or health checks.
    pub fn kernel(&self) -> &Kernel {
        &self.kernel
    }

    /// Get the current lifecycle state of the kernel.
    pub fn state(&self) -> cortex_core::LifecycleState {
        self.kernel.state()
    }

    /// Get the current iteration count.
    pub fn iteration_count(&self) -> u64 {
        self.kernel.iteration_count()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
