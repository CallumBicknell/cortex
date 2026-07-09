//! `spawn_subagent` tool — nests an AgentLoop with depth limits.

use crate::agent_loop::AgentLoopConfig;
use crate::subagent::{format_subagent_result, run_subagent, SubAgentOptions, SubAgentParent};
use async_trait::async_trait;
use cortex_core::InMemoryEventBus;
use cortex_llm::Provider;
use cortex_tools::{Result as ToolResult, Tool, ToolContext, ToolError, ToolExecutor};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::sync::Mutex;

/// Shared handle for spawning sub-agents from tools.
#[derive(Clone)]
pub struct SubAgentHandle {
    /// LLM provider for child runs.
    pub(crate) provider: Arc<dyn Provider>,
    /// Model id for child runs.
    pub(crate) model: String,
    /// Tool executor available to children (without nesting tools).
    pub(crate) tools: ToolExecutor,
    /// Parent loop config (depth, budgets). Updated carefully per parent run if needed.
    pub(crate) parent_config: Arc<Mutex<AgentLoopConfig>>,
    /// Optional event bus for sub-agent lifecycle events.
    pub(crate) bus: Option<Arc<InMemoryEventBus>>,
}

impl SubAgentHandle {
    /// Create a handle bound to provider/tools/config.
    pub fn new(
        provider: Arc<dyn Provider>,
        model: impl Into<String>,
        tools: ToolExecutor,
        parent_config: AgentLoopConfig,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            tools,
            parent_config: Arc::new(Mutex::new(parent_config)),
            bus: None,
        }
    }

    /// Attach event bus for parent/child correlation events.
    pub fn with_event_bus(mut self, bus: Arc<InMemoryEventBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    /// Replace parent config (e.g. after CLI builds full context).
    pub fn set_parent_config(&self, cfg: AgentLoopConfig) {
        *self.parent_config.lock().expect("config lock") = cfg;
    }
}

/// Tool that launches a nested agent for a focused subtask.
pub struct SpawnSubagentTool {
    handle: SubAgentHandle,
}

impl SpawnSubagentTool {
    /// Create tool.
    pub fn new(handle: SubAgentHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct SpawnInput {
    prompt: String,
    #[serde(default)]
    max_turns: Option<u32>,
    #[serde(default)]
    tools: Option<Vec<String>>,
}

#[async_trait]
impl Tool for SpawnSubagentTool {
    fn name(&self) -> &str {
        "spawn_subagent"
    }

    fn description(&self) -> &str {
        "Spawn a nested sub-agent to complete a focused subtask. \
         The child has limited turns/depth and cannot nest further spawn_subagent calls. \
         Use for parallelizable research or isolated multi-step work."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "Task description for the sub-agent"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max LLM turns for the child (default 8)",
                    "default": 8
                },
                "tools": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional tool allow-list for the child"
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> ToolResult<String> {
        ctx.check_cancelled()?;
        let args: SpawnInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid spawn_subagent args: {e}")))?;
        if args.prompt.trim().is_empty() {
            return Err(ToolError::InvalidInput("prompt must not be empty".into()));
        }

        let parent_cfg = self
            .handle
            .parent_config
            .lock()
            .expect("config lock")
            .clone();

        let opts = SubAgentOptions {
            prompt: args.prompt,
            max_turns: args.max_turns.unwrap_or(8),
            allowed_tools: args.tools.unwrap_or_default(),
            disable_summarize: true,
        };

        // Child gets its own cancel linked to parent.
        let child_cancel = ctx.cancel.child_token();
        let mut child_ctx = ctx.clone();
        child_ctx.cancel = child_cancel.clone();

        let parent = SubAgentParent {
            session_id: ctx.session_id,
            run_id: None,
            bus: self.handle.bus.clone(),
        };
        let out = run_subagent(
            Arc::clone(&self.handle.provider),
            self.handle.model.clone(),
            self.handle.tools.clone(),
            &parent_cfg,
            child_ctx,
            child_cancel,
            opts,
            parent,
        )
        .await
        .map_err(|e| ToolError::Execution(e.to_string()))?;

        Ok(ctx.truncate_output(format_subagent_result(&out)))
    }
}
