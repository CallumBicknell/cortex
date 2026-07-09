//! Cortex Runtime
//!
//! Process kernel facade plus the **agent loop** that drives:
//! Observe → Plan (LLM) → Execute tools → Verify → Reflect → Done.

#![deny(missing_docs)]

mod agent_loop;
mod context;
mod error;
mod runtime;
mod subagent;
mod subagent_tool;
mod summarize;

pub use agent_loop::{AgentLoop, AgentLoopConfig, RunInput, RunOutput};
pub use context::{ContextBuilder, DEFAULT_SYSTEM_PROMPT};
pub use error::{Result, RuntimeError};
pub use runtime::Runtime;
pub use subagent::{format_subagent_result, run_subagent, SubAgentOptions};
pub use subagent_tool::{SpawnSubagentTool, SubAgentHandle};

use cortex_tools::{ToolExecutor, ToolRegistry};
use std::sync::Arc;

/// Clone tools from `base` and register `spawn_subagent` for nested runs.
///
/// The sub-agent handle uses a copy of `base` (without `spawn_subagent`) so
/// children cannot re-enter nesting via the tool table; depth limits still apply
/// if the parent allow-list is customized later.
pub fn tools_with_subagent(
    base: &ToolExecutor,
    provider: Arc<dyn cortex_llm::Provider>,
    model: impl Into<String>,
    parent_config: AgentLoopConfig,
) -> ToolExecutor {
    let mut child_reg = ToolRegistry::new();
    for name in base.registry().names() {
        if name == "spawn_subagent" {
            continue;
        }
        if let Ok(tool) = base.registry().get(&name) {
            let _ = child_reg.register(tool);
        }
    }
    let child_tools = ToolExecutor::new(Arc::new(child_reg));
    let handle = SubAgentHandle::new(provider, model, child_tools, parent_config);

    let mut parent_reg = ToolRegistry::new();
    for name in base.registry().names() {
        if let Ok(tool) = base.registry().get(&name) {
            let _ = parent_reg.register(tool);
        }
    }
    parent_reg.register_or_replace(Arc::new(SpawnSubagentTool::new(handle)));
    ToolExecutor::new(Arc::new(parent_reg))
}
pub use summarize::{maybe_summarize, SummarizeConfig, SummarizeOutcome};

// Re-export loop phase and workspace helpers for callers.
pub use cortex_events::LoopPhase;
pub use cortex_prompts::{PromptCatalog, PromptError};
pub use cortex_skills::{select_skills, Skill, SkillRegistry, SkillSelection};
pub use cortex_workspace::{ProjectInfo, RepoMap};
