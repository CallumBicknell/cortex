//! Deterministic agent loop: Observe → Plan (LLM) → Execute tools → … → Done.

use crate::context::ContextBuilder;
use crate::error::{Result, RuntimeError};
use crate::summarize::{maybe_summarize, SummarizeConfig};
use cortex_common::RunId;
use cortex_core::{EventBus, InMemoryEventBus};
use cortex_events::{
    AssistantMessageProduced, AssistantTextDelta, ErrorRaised, LoopPhase, LoopPhaseChanged,
    ToolCallCompleted, ToolCallFailed, ToolCallRequested, UserMessageReceived,
};
use cortex_llm::{ChatRequest, FinishReason, Provider, StreamEvent, Usage};
use cortex_models::{Message, Session, TaskStatus, ToolCall, ToolResult};
use cortex_tools::{is_file_mutating, ToolContext, ToolExecutor};
use futures::StreamExt;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Configuration for a single agent run.
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Maximum LLM turns (each model call counts as one turn).
    pub max_turns: u32,
    /// Context assembly.
    pub context: ContextBuilder,
    /// Optional sampling temperature.
    pub temperature: Option<f32>,
    /// Optional max output tokens.
    pub max_tokens: Option<u32>,
    /// If true, stop when max turns hit even mid-tool-use (default true).
    pub stop_on_max_turns: bool,
    /// Rolling history summarization.
    pub summarize: SummarizeConfig,
    /// Optional wall-clock budget for the entire run (seconds). `0` = unlimited.
    pub max_run_secs: u64,
    /// Cap tool calls executed from a single assistant message. `0` = unlimited.
    pub max_tool_calls_per_turn: usize,
    /// Current sub-agent nesting depth (0 = top-level run).
    pub subagent_depth: u32,
    /// Maximum allowed nesting depth for sub-agents (inclusive of this level).
    pub max_subagent_depth: u32,
    /// When true, inject plan-mode guidance (read/plan first; no silent big rewrites).
    pub plan_mode: bool,
    /// After successful file-mutating tools, run `verify_command` via shell once per turn.
    pub verify_after_writes: bool,
    /// Shell command for verify (e.g. `cargo test` / `forge test`). Required when verify is on.
    pub verify_command: Option<String>,
    /// When true, use provider streaming and emit [`AssistantTextDelta`] events.
    pub stream_tokens: bool,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_turns: 32,
            context: ContextBuilder::default(),
            temperature: None,
            max_tokens: None,
            stop_on_max_turns: true,
            summarize: SummarizeConfig::default(),
            max_run_secs: 600,
            max_tool_calls_per_turn: 16,
            subagent_depth: 0,
            max_subagent_depth: 2,
            plan_mode: false,
            verify_after_writes: false,
            verify_command: None,
            stream_tokens: false,
        }
    }
}

const PLAN_MODE_ADDENDUM: &str = r#"

## Plan mode (active)

You are in **plan mode** for this run:
1. Prefer reading, outlining, and mapping before any write.
2. First substantive reply should include a short **Plan** (bullets) before edits.
3. Keep changes minimal; do not start large rewrites without stating the plan.
4. If the user only asked for a plan, stop after the plan (no tool writes).
"#;

/// Input for one agent run against a session.
pub struct RunInput {
    /// Mutable session (messages are appended in place conceptually; returned updated).
    pub session: Session,
    /// User prompt for this run.
    pub prompt: String,
    /// Cancellation for this run (and nested tool/LLM calls).
    pub cancel: CancellationToken,
    /// Tool execution context (workspace, permissions, approver).
    pub tool_ctx: ToolContext,
}

/// Outcome of an agent run.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// Updated session with full message history.
    pub session: Session,
    /// Run id.
    pub run_id: RunId,
    /// Final status.
    pub status: TaskStatus,
    /// Number of LLM turns consumed.
    pub turns: u32,
    /// Last phase reached.
    pub phase: LoopPhase,
    /// Final assistant text (if any).
    pub final_message: Option<String>,
    /// Accumulated tool results this run (for debugging).
    pub tool_results: Vec<ToolResult>,
    /// Wall time for the run.
    pub duration_ms: u64,
    /// Optional error message when failed/cancelled.
    pub error: Option<String>,
    /// Accumulated token usage across all turns.
    pub total_usage: Usage,
}

/// The agent loop engine.
pub struct AgentLoop {
    provider: Arc<dyn Provider>,
    model: String,
    tools: ToolExecutor,
    config: AgentLoopConfig,
    event_bus: Option<Arc<InMemoryEventBus>>,
    /// Rolling summary carried across turns of a run.
    rolling_summary: std::sync::Mutex<Option<String>>,
}

impl AgentLoop {
    /// Create an agent loop.
    pub fn new(
        provider: Arc<dyn Provider>,
        model: impl Into<String>,
        tools: ToolExecutor,
        config: AgentLoopConfig,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            tools,
            config,
            event_bus: None,
            rolling_summary: std::sync::Mutex::new(None),
        }
    }

    /// Seed or replace the rolling conversation summary (e.g. from SQLite).
    pub fn set_rolling_summary(&self, summary: Option<String>) {
        *self.rolling_summary.lock().expect("summary lock") = summary;
    }

    /// Current rolling summary, if any.
    pub fn rolling_summary(&self) -> Option<String> {
        self.rolling_summary.lock().expect("summary lock").clone()
    }

    /// Attach an event bus for observability.
    pub fn with_event_bus(mut self, bus: Arc<InMemoryEventBus>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Model id used for chat requests.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Tool executor.
    pub fn tools(&self) -> &ToolExecutor {
        &self.tools
    }

    /// Run the agent loop until completion, cancellation, failure, or max turns.
    pub async fn run(&self, input: RunInput) -> Result<RunOutput> {
        let started = Instant::now();
        let run_id = RunId::new();
        let mut session = input.session;
        let cancel = input.cancel;
        // Align tool context cancel with run cancel.
        let mut tool_ctx = input.tool_ctx;
        tool_ctx.cancel = cancel.clone();

        let mut phase = LoopPhase::Idle;
        let mut turns = 0u32;
        let mut tool_results: Vec<ToolResult> = Vec::new();
        let mut final_message: Option<String> = None;
        let mut total_usage = Usage::default();

        // --- Observe: accept user input ---
        phase = self
            .transition(&session, Some(run_id), phase, LoopPhase::Observing)
            .await;
        let user_msg = Message::user(&input.prompt);
        self.publish(
            UserMessageReceived::new(session.id, user_msg.id, &input.prompt).with_run_id(run_id),
        )
        .await;
        session.push_message(user_msg);

        let outcome = self
            .run_turns(
                &mut session,
                run_id,
                &cancel,
                &tool_ctx,
                &mut phase,
                &mut turns,
                &mut tool_results,
                &mut final_message,
                &mut total_usage,
            )
            .await;

        let duration_ms = started.elapsed().as_millis() as u64;

        match outcome {
            Ok(status) => {
                info!(
                    %run_id,
                    turns,
                    ?status,
                    duration_ms,
                    "agent run completed"
                );
                Ok(RunOutput {
                    session,
                    run_id,
                    status,
                    turns,
                    phase,
                    final_message,
                    tool_results,
                    duration_ms,
                    error: None,
                    total_usage,
                })
            }
            Err(err) => {
                let status = if matches!(err, RuntimeError::Cancelled(_)) {
                    TaskStatus::Cancelled
                } else {
                    TaskStatus::Failed
                };
                phase = self
                    .transition(&session, Some(run_id), phase, LoopPhase::Failed)
                    .await;
                self.publish(
                    ErrorRaised::new("runtime", err.to_string(), false).with_session_id(session.id),
                )
                .await;
                warn!(%run_id, error = %err, "agent run failed");
                Ok(RunOutput {
                    session,
                    run_id,
                    status,
                    turns,
                    phase,
                    final_message,
                    tool_results,
                    duration_ms,
                    error: Some(err.to_string()),
                    total_usage,
                })
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_turns(
        &self,
        session: &mut Session,
        run_id: RunId,
        cancel: &CancellationToken,
        tool_ctx: &ToolContext,
        phase: &mut LoopPhase,
        turns: &mut u32,
        tool_results: &mut Vec<ToolResult>,
        final_message: &mut Option<String>,
        total_usage: &mut Usage,
    ) -> Result<TaskStatus> {
        let run_started = Instant::now();
        loop {
            if cancel.is_cancelled() {
                return Err(RuntimeError::Cancelled("run cancelled".into()));
            }

            if self.config.max_run_secs > 0
                && run_started.elapsed().as_secs() >= self.config.max_run_secs
            {
                *phase = self
                    .transition(session, Some(run_id), *phase, LoopPhase::Failed)
                    .await;
                return Err(RuntimeError::RunTimeout(self.config.max_run_secs));
            }

            if *turns >= self.config.max_turns && self.config.stop_on_max_turns {
                *phase = self
                    .transition(session, Some(run_id), *phase, LoopPhase::Failed)
                    .await;
                return Err(RuntimeError::MaxTurns(self.config.max_turns));
            }

            // --- Plan: call LLM ---
            *phase = self
                .transition(session, Some(run_id), *phase, LoopPhase::Planning)
                .await;
            *turns += 1;
            let turn = *turns;

            // Fold long histories into a rolling summary before building context.
            {
                let prev = self.rolling_summary();
                if let Some(outcome) = maybe_summarize(
                    &self.provider,
                    &self.model,
                    &session.messages,
                    prev.as_deref(),
                    &self.config.summarize,
                )
                .await
                {
                    self.set_rolling_summary(Some(outcome.summary));
                }
            }

            let mut ctx = self.config.context.clone();
            if self.config.plan_mode {
                ctx.system_prompt = format!("{}{PLAN_MODE_ADDENDUM}", ctx.system_prompt);
            }
            if let Some(summary) = self.rolling_summary() {
                ctx = ctx.with_rolling_summary(summary);
            }
            let messages = ctx.build_messages(&session.messages);
            let tools = ctx.build_tools(self.tools.registry().specs());

            let mut req = ChatRequest::new(&self.model, messages)
                .with_tools(tools)
                .with_cancel(cancel.clone());
            if let Some(t) = self.config.temperature {
                req = req.with_temperature(t);
            }
            if let Some(m) = self.config.max_tokens {
                req = req.with_max_tokens(m);
            }

            debug!(
                turn,
                model = %self.model,
                stream = self.config.stream_tokens,
                "calling provider"
            );
            let response = if self.config.stream_tokens {
                let mut stream = self.provider.stream(req).await?;
                let mut final_response = None;
                while let Some(item) = stream.next().await {
                    let ev = item?;
                    match ev {
                        StreamEvent::TextDelta { text } => {
                            if !text.is_empty() {
                                self.publish(
                                    AssistantTextDelta::new(session.id, text).with_run_id(run_id),
                                )
                                .await;
                            }
                        }
                        StreamEvent::ToolCallDelta { .. } => {
                            // Aggregated in Done; ignore fragments for now.
                        }
                        StreamEvent::Done { response } => {
                            final_response = Some(response);
                        }
                        StreamEvent::Error { message } => {
                            warn!(%message, "provider stream error event");
                        }
                    }
                }
                final_response.ok_or_else(|| {
                    RuntimeError::Invalid("stream ended without Done event".into())
                })?
            } else {
                self.provider.chat(req).await?
            };

            let assistant = response.message.clone();
            // Accumulate token usage across turns.
            total_usage.prompt_tokens += response.usage.prompt_tokens;
            total_usage.completion_tokens += response.usage.completion_tokens;
            total_usage.total_tokens += response.usage.total_tokens;
            self.publish(
                AssistantMessageProduced::new(session.id, assistant.id, &assistant.content)
                    .with_run_id(run_id)
                    .with_tool_calls(assistant.tool_calls.clone()),
            )
            .await;
            session.push_message(assistant.clone());

            let wants_tools = response.has_tool_calls()
                || response.finish_reason == FinishReason::ToolCalls
                || !assistant.tool_calls.is_empty();

            if !wants_tools {
                // --- Verify (stub) + Reflect (stub) + Done ---
                *phase = self
                    .transition(session, Some(run_id), *phase, LoopPhase::Verifying)
                    .await;
                // Stub: always pass.
                *phase = self
                    .transition(session, Some(run_id), *phase, LoopPhase::Reflecting)
                    .await;
                // Stub: no memory write yet.
                *final_message = Some(assistant.content.clone());
                *phase = self
                    .transition(session, Some(run_id), *phase, LoopPhase::Done)
                    .await;
                return Ok(TaskStatus::Succeeded);
            }

            // --- Execute tools ---
            *phase = self
                .transition(session, Some(run_id), *phase, LoopPhase::Executing)
                .await;

            let calls = assistant.tool_calls.clone();
            if self.config.max_tool_calls_per_turn > 0
                && calls.len() > self.config.max_tool_calls_per_turn
            {
                return Err(RuntimeError::TooManyToolCalls {
                    got: calls.len(),
                    max: self.config.max_tool_calls_per_turn,
                });
            }

            for call in &calls {
                self.publish(ToolCallRequested::from_call(session.id, call).with_run_id(run_id))
                    .await;
            }

            // Safe parallel batches for read-only tools; serial for writes/shell/agents.
            let batch_results = self.tools.execute_all(tool_ctx, &calls).await;
            let mut wrote_files = false;
            for (call, result) in calls.iter().zip(batch_results.iter()) {
                if cancel.is_cancelled() {
                    return Err(RuntimeError::Cancelled(
                        "run cancelled during tool execution".into(),
                    ));
                }
                if self.config.max_run_secs > 0
                    && run_started.elapsed().as_secs() >= self.config.max_run_secs
                {
                    return Err(RuntimeError::RunTimeout(self.config.max_run_secs));
                }
                self.emit_tool_result(session.id, run_id, call, result)
                    .await;
                if !result.is_error && is_file_mutating(&call.name) {
                    wrote_files = true;
                }
                tool_results.push(result.clone());
                let tool_msg =
                    Message::tool_result(result.tool_call_id, &result.name, &result.output);
                session.push_message(tool_msg);
            }

            // Optional verify hook after successful file mutations.
            if wrote_files
                && self.config.verify_after_writes
                && self
                    .config
                    .verify_command
                    .as_ref()
                    .map(|c| !c.trim().is_empty())
                    .unwrap_or(false)
            {
                let cmd = self.config.verify_command.as_ref().unwrap().clone();
                info!(%cmd, "verify_after_writes: running project test command");
                let verify_call = ToolCall::new("shell", serde_json::json!({ "command": cmd }));
                self.publish(
                    ToolCallRequested::from_call(session.id, &verify_call).with_run_id(run_id),
                )
                .await;
                let result = self.tools.execute(tool_ctx, &verify_call).await;
                self.emit_tool_result(session.id, run_id, &verify_call, &result)
                    .await;
                tool_results.push(result.clone());
                let tool_msg =
                    Message::tool_result(result.tool_call_id, &result.name, &result.output);
                session.push_message(tool_msg);
            }
            // Continue loop for next planning turn.
        }
    }

    async fn emit_tool_result(
        &self,
        session_id: cortex_common::SessionId,
        run_id: RunId,
        call: &ToolCall,
        result: &ToolResult,
    ) {
        if result.is_error {
            // Distinguish policy/runtime-ish failures vs tool-level errors via message.
            if result.output.contains("denied") || result.output.contains("cancelled") {
                self.publish(
                    ToolCallFailed::new(session_id, call.id, &call.name, &result.output)
                        .with_run_id_opt(Some(run_id)),
                )
                .await;
            } else {
                self.publish(
                    ToolCallCompleted::new(session_id, call.id, &call.name, &result.output, true)
                        .with_run_id_opt(Some(run_id))
                        .with_duration_ms(result.duration_ms),
                )
                .await;
            }
        } else {
            self.publish(
                ToolCallCompleted::new(session_id, call.id, &call.name, &result.output, false)
                    .with_run_id_opt(Some(run_id))
                    .with_duration_ms(result.duration_ms),
            )
            .await;
        }
    }

    async fn transition(
        &self,
        session: &Session,
        run_id: Option<RunId>,
        from: LoopPhase,
        to: LoopPhase,
    ) -> LoopPhase {
        if from != to {
            debug!(?from, ?to, "loop phase changed");
            let mut ev = LoopPhaseChanged::new(session.id, from, to);
            if let Some(id) = run_id {
                ev = ev.with_run_id(id);
            }
            self.publish(ev).await;
        }
        to
    }

    async fn publish<E: cortex_core::Event + serde::Serialize>(&self, event: E) {
        if let Some(bus) = &self.event_bus {
            bus.publish(event).await;
        }
    }
}

// Small helpers on events that didn't have with_run_id / duration — extend via local trait.
trait ToolCallCompletedExt {
    fn with_run_id_opt(self, run_id: Option<RunId>) -> Self;
    fn with_duration_ms(self, ms: Option<u64>) -> Self;
}

impl ToolCallCompletedExt for ToolCallCompleted {
    fn with_run_id_opt(mut self, run_id: Option<RunId>) -> Self {
        self.run_id = run_id;
        self
    }

    fn with_duration_ms(mut self, ms: Option<u64>) -> Self {
        self.duration_ms = ms;
        self
    }
}

trait ToolCallFailedExt {
    fn with_run_id_opt(self, run_id: Option<RunId>) -> Self;
}

impl ToolCallFailedExt for ToolCallFailed {
    fn with_run_id_opt(mut self, run_id: Option<RunId>) -> Self {
        self.run_id = run_id;
        self
    }
}
