//! Plans produced by the planner (optional structured steps).

use chrono::{DateTime, Utc};
use cortex_common::PlanId;
use serde::{Deserialize, Serialize};

/// Status of a plan or plan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStatus {
    /// Not yet started.
    Pending,
    /// Currently executing.
    InProgress,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Cancelled.
    Cancelled,
}

/// A single step in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    /// Step index (0-based).
    pub index: u32,
    /// Human-readable description.
    pub description: String,
    /// Optional tool name this step intends to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Step status.
    pub status: PlanStatus,
}

/// A multi-step plan for a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// Plan id.
    pub id: PlanId,
    /// High-level goal.
    pub goal: String,
    /// Ordered steps.
    pub steps: Vec<PlanStep>,
    /// Overall status.
    pub status: PlanStatus,
    /// Creation time.
    pub created_at: DateTime<Utc>,
}

impl Plan {
    /// Create a new pending plan with the given steps descriptions.
    pub fn new(
        goal: impl Into<String>,
        step_descriptions: impl IntoIterator<Item = String>,
    ) -> Self {
        let steps = step_descriptions
            .into_iter()
            .enumerate()
            .map(|(i, description)| PlanStep {
                index: i as u32,
                description,
                tool: None,
                status: PlanStatus::Pending,
            })
            .collect();
        Self {
            id: PlanId::new(),
            goal: goal.into(),
            steps,
            status: PlanStatus::Pending,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_roundtrip() {
        let plan = Plan::new(
            "fix bug",
            vec!["read file".into(), "edit file".into(), "run tests".into()],
        );
        assert_eq!(plan.steps.len(), 3);
        let raw = serde_json::to_string(&plan).unwrap();
        let back: Plan = serde_json::from_str(&raw).unwrap();
        assert_eq!(plan, back);
    }
}
