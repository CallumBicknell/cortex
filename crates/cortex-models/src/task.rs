//! User-facing tasks / goals for a run.

use chrono::{DateTime, Utc};
use cortex_common::{RunId, SessionId};
use serde::{Deserialize, Serialize};

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Queued / not started.
    Pending,
    /// Running.
    Running,
    /// Finished successfully.
    Succeeded,
    /// Finished with failure.
    Failed,
    /// Cancelled by user or system.
    Cancelled,
}

/// A unit of work submitted to the agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Run id for this task execution.
    pub run_id: RunId,
    /// Parent session.
    pub session_id: SessionId,
    /// User prompt / goal text.
    pub prompt: String,
    /// Status.
    pub status: TaskStatus,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task finished, if ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
}

impl Task {
    /// Create a new pending task.
    pub fn new(session_id: SessionId, prompt: impl Into<String>) -> Self {
        Self {
            run_id: RunId::new(),
            session_id,
            prompt: prompt.into(),
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            finished_at: None,
        }
    }

    /// Mark the task as running.
    pub fn mark_running(&mut self) {
        self.status = TaskStatus::Running;
    }

    /// Mark the task as succeeded.
    pub fn mark_succeeded(&mut self) {
        self.status = TaskStatus::Succeeded;
        self.finished_at = Some(Utc::now());
    }

    /// Mark the task as failed.
    pub fn mark_failed(&mut self) {
        self.status = TaskStatus::Failed;
        self.finished_at = Some(Utc::now());
    }

    /// Mark the task as cancelled.
    pub fn mark_cancelled(&mut self) {
        self.status = TaskStatus::Cancelled;
        self.finished_at = Some(Utc::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_lifecycle() {
        let mut task = Task::new(SessionId::new(), "do the thing");
        task.mark_running();
        assert_eq!(task.status, TaskStatus::Running);
        task.mark_succeeded();
        assert_eq!(task.status, TaskStatus::Succeeded);
        assert!(task.finished_at.is_some());
    }
}
