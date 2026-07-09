//! Nested sub-agent runs with depth and tool limits.

use crate::agent_loop::{AgentLoop, AgentLoopConfig, RunInput, RunOutput};
use crate::error::{Result, RuntimeError};
use crate::summarize::SummarizeConfig;
use cortex_core::{EventBus, InMemoryEventBus};
use cortex_events::{SubAgentFinished, SubAgentStarted};
use cortex_llm::Provider;
use cortex_models::Session;
use cortex_tools::{ToolContext, ToolExecutor};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// Options for spawning a sub-agent.
#[derive(Debug, Clone)]
pub struct SubAgentOptions {
    /// Task prompt for the sub-agent.
    pub prompt: String,
    /// Max turns for the child (defaults to 8).
    pub max_turns: u32,
    /// Optional tool allow-list (names). If empty, inherit parent tools but
    /// strip `spawn_subagent` to prevent unbounded recursion.
    pub allowed_tools: Vec<String>,
    /// Disable conversation summarization in the child (default true for speed).
    pub disable_summarize: bool,
}

impl Default for SubAgentOptions {
    fn default() -> Self {
        Self {
            prompt: String::new(),
            max_turns: 8,
            allowed_tools: Vec::new(),
            disable_summarize: true,
        }
    }
}

/// Optional parent context for event correlation.
#[derive(Clone, Default)]
pub struct SubAgentParent {
    /// Parent session.
    pub session_id: Option<cortex_common::SessionId>,
    /// Parent run id.
    pub run_id: Option<cortex_common::RunId>,
    /// Event bus to publish sub-agent lifecycle events onto.
    pub bus: Option<Arc<InMemoryEventBus>>,
}

/// Run a nested agent loop as a sub-agent of `parent_config`.
pub async fn run_subagent(
    provider: Arc<dyn Provider>,
    model: impl Into<String>,
    tools: ToolExecutor,
    parent_config: &AgentLoopConfig,
    tool_ctx: ToolContext,
    cancel: CancellationToken,
    opts: SubAgentOptions,
    parent: SubAgentParent,
) -> Result<RunOutput> {
    let next_depth = parent_config.subagent_depth.saturating_add(1);
    if next_depth > parent_config.max_subagent_depth {
        return Err(RuntimeError::SubAgentDepth(
            parent_config.max_subagent_depth,
        ));
    }

    let model = model.into();
    let mut child_cfg = parent_config.clone();
    child_cfg.max_turns = opts.max_turns.max(1);
    child_cfg.subagent_depth = next_depth;
    if child_cfg.max_run_secs > 0 {
        child_cfg.max_run_secs = child_cfg.max_run_secs.min(300);
    }
    if opts.disable_summarize {
        child_cfg.summarize = SummarizeConfig {
            enabled: false,
            ..child_cfg.summarize
        };
    }

    let mut allowed = opts.allowed_tools;
    if allowed.is_empty() {
        allowed = tools.registry().names();
    }
    allowed.retain(|n| n != "spawn_subagent" && n != "audit_lenses");
    child_cfg.context = child_cfg.context.with_allowed_tools(allowed);

    info!(
        depth = next_depth,
        max_turns = child_cfg.max_turns,
        "starting sub-agent"
    );

    let mut agent = AgentLoop::new(provider, model.clone(), tools, child_cfg);
    if let Some(bus) = &parent.bus {
        agent = agent.with_event_bus(Arc::clone(bus));
    }
    let session = Session::new(
        tool_ctx.workspace_root.to_string_lossy(),
        format!("subagent/{model}"),
    );
    let child_session_id = session.id;
    let prompt_preview: String = opts.prompt.chars().take(200).collect();

    let out = agent
        .run(RunInput {
            session,
            prompt: opts.prompt,
            cancel,
            tool_ctx,
        })
        .await?;

    if let (Some(bus), Some(parent_sid)) = (&parent.bus, parent.session_id) {
        let mut started = SubAgentStarted::new(
            parent_sid,
            child_session_id,
            out.run_id,
            next_depth,
            prompt_preview,
        );
        if let Some(rid) = parent.run_id {
            started = started.with_parent_run_id(rid);
        }
        bus.publish(started).await;

        let mut finished = SubAgentFinished::new(
            parent_sid,
            out.session.id,
            out.run_id,
            next_depth,
            format!("{:?}", out.status),
            out.turns,
            out.duration_ms,
        );
        if let Some(rid) = parent.run_id {
            finished = finished.with_parent_run_id(rid);
        }
        if let Some(m) = &out.final_message {
            finished = finished.with_final_message(m.chars().take(500).collect::<String>());
        }
        if let Some(e) = &out.error {
            finished = finished.with_error(e);
        }
        bus.publish(finished).await;
    }

    Ok(out)
}

/// Format a sub-agent result for tool output.
pub fn format_subagent_result(out: &RunOutput) -> String {
    let mut s = format!(
        "sub-agent status={:?} turns={} duration_ms={}\n",
        out.status, out.turns, out.duration_ms
    );
    if let Some(msg) = &out.final_message {
        s.push_str("--- final ---\n");
        s.push_str(msg);
        s.push('\n');
    }
    if let Some(err) = &out.error {
        s.push_str("--- error ---\n");
        s.push_str(err);
        s.push('\n');
    }
    if !out.tool_results.is_empty() {
        s.push_str("--- tools ---\n");
        for t in &out.tool_results {
            let flag = if t.is_error { "ERR" } else { "ok" };
            let preview: String = t.output.chars().take(160).collect();
            s.push_str(&format!("[{flag}] {}: {preview}\n", t.name));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_llm::MockProvider;
    use cortex_tools::{register_default_tools, AlwaysAllow, PermissionPolicy, ToolRegistry};
    use std::time::Duration;

    #[tokio::test]
    async fn depth_limit_blocks() {
        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let tools = ToolExecutor::new(Arc::new(reg));
        let parent = AgentLoopConfig {
            max_subagent_depth: 1,
            subagent_depth: 1,
            ..Default::default()
        };
        let ctx = ToolContext {
            workspace_root: std::env::temp_dir(),
            session_id: None,
            cancel: CancellationToken::new(),
            permissions: Arc::new(PermissionPolicy::default().allow_all()),
            approver: Arc::new(AlwaysAllow),
            default_timeout: Duration::from_secs(5),
        };
        let err = run_subagent(
            Arc::new(MockProvider::echo("x")),
            "m",
            tools,
            &parent,
            ctx,
            CancellationToken::new(),
            SubAgentOptions {
                prompt: "hi".into(),
                ..Default::default()
            },
            SubAgentParent::default(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, RuntimeError::SubAgentDepth(1)));
    }
}
