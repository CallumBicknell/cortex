//! Kernel lifecycle states.

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
    /// Kernel has been stopped cleanly.
    Stopped,
    /// Kernel has encountered a fatal error.
    Failed,
}

impl LifecycleState {
    /// Returns true if the kernel is currently running.
    pub fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the kernel has fully stopped (cleanly or via failure).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::Failed)
    }
}
