//! End-to-end agent loop: mock LLM + real filesystem tools.

use cortex_core::{EventBus, InMemoryEventBus};
use cortex_events::LoopPhase;
use cortex_llm::{MockProvider, MockResponse};
use cortex_models::{Message, Session, TaskStatus, ToolCall};
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput};
use cortex_tools::{
    register_default_tools, AlwaysAllow, PermissionPolicy, ToolContext, ToolExecutor, ToolRegistry,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;
use tokio_util::sync::CancellationToken;

fn setup_tools(workspace: &std::path::Path) -> (ToolExecutor, ToolContext) {
    let mut reg = ToolRegistry::new();
    register_default_tools(&mut reg).unwrap();
    let exec = ToolExecutor::new(Arc::new(reg));
    let ctx = ToolContext {
        workspace_root: workspace.to_path_buf(),
        session_id: None,
        cancel: CancellationToken::new(),
        permissions: Arc::new(PermissionPolicy::default().allow_all()),
        approver: Arc::new(AlwaysAllow),
        default_timeout: Duration::from_secs(10),
    };
    (exec, ctx)
}

#[tokio::test]
async fn multi_step_write_file_then_finish() {
    let dir = tempdir().unwrap();
    let (tools, tool_ctx) = setup_tools(dir.path());
    let bus = Arc::new(InMemoryEventBus::new(256));

    let mock = MockProvider::new(vec![
        // Turn 1: model requests write_file
        MockResponse::with_tools(
            "mock-model",
            Message::assistant_with_tools(
                "I'll write the file.",
                vec![ToolCall::new(
                    "write_file",
                    json!({
                        "path": "hello.txt",
                        "content": "hello from agent\n"
                    }),
                )],
            ),
        ),
        // Turn 2: model finishes
        MockResponse::text("mock-model", "Wrote hello.txt successfully."),
    ]);

    let loop_engine = AgentLoop::new(
        Arc::new(mock),
        "mock-model",
        tools,
        AgentLoopConfig {
            max_turns: 8,
            context: ContextBuilder::new("You are a test agent."),
            temperature: None,
            max_tokens: None,
            stop_on_max_turns: true,
            summarize: cortex_runtime::SummarizeConfig {
                enabled: false,
                ..Default::default()
            },
        },
    )
    .with_event_bus(Arc::clone(&bus));

    let session = Session::new(dir.path().to_string_lossy(), "mock-model");
    let output = loop_engine
        .run(RunInput {
            session,
            prompt: "Create hello.txt with a greeting".into(),
            cancel: CancellationToken::new(),
            tool_ctx,
        })
        .await
        .unwrap();

    assert_eq!(output.status, TaskStatus::Succeeded);
    assert_eq!(output.phase, LoopPhase::Done);
    assert_eq!(output.turns, 2);
    assert_eq!(
        output.final_message.as_deref(),
        Some("Wrote hello.txt successfully.")
    );
    assert_eq!(output.tool_results.len(), 1);
    assert!(!output.tool_results[0].is_error);

    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(content, "hello from agent\n");

    // Session should contain user + assistant(tool) + tool + assistant(final)
    assert!(output.session.messages.len() >= 4);

    let history = bus.history().await;
    let kinds: Vec<_> = history.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"agent.user_message"));
    assert!(kinds.contains(&"agent.tool_call.requested"));
    assert!(kinds.contains(&"agent.tool_call.completed"));
    assert!(kinds.contains(&"agent.loop.phase_changed"));
    assert!(kinds.contains(&"agent.assistant_message"));
}

#[tokio::test]
async fn cancel_before_second_turn() {
    let dir = tempdir().unwrap();
    let (tools, tool_ctx) = setup_tools(dir.path());
    let cancel = CancellationToken::new();

    // First response wants tools; we'll cancel mid-loop after first tool by
    // cancelling in a script that only has one response that finishes... instead
    // use a provider that returns tools, then hang — simpler: cancel before run
    // after scheduling? Better: cancel token already cancelled.
    cancel.cancel();

    let mock = MockProvider::new(vec![MockResponse::text("m", "should not run")]);
    let loop_engine = AgentLoop::new(
        Arc::new(mock),
        "m",
        tools,
        AgentLoopConfig {
            max_turns: 4,
            ..Default::default()
        },
    );

    let output = loop_engine
        .run(RunInput {
            session: Session::new(dir.path().to_string_lossy(), "m"),
            prompt: "hi".into(),
            cancel,
            tool_ctx,
        })
        .await
        .unwrap();

    // User message is accepted, then cancel on first planning turn.
    assert_eq!(output.status, TaskStatus::Cancelled);
    assert!(output.error.as_ref().unwrap().contains("cancel"));
}

#[tokio::test]
async fn max_turns_stops_tool_loop() {
    let dir = tempdir().unwrap();
    let (tools, tool_ctx) = setup_tools(dir.path());

    // Always request another tool call — hits max turns.
    let call = ToolCall::new(
        "read_file",
        json!({"path": "missing.txt"}), // will error but still counts as tool use
    );
    let mock = MockProvider::new(vec![
        MockResponse::with_tools("m", Message::assistant_with_tools("", vec![call.clone()])),
        MockResponse::with_tools("m", Message::assistant_with_tools("", vec![call.clone()])),
        MockResponse::with_tools("m", Message::assistant_with_tools("", vec![call])),
    ]);

    // Create a dummy file so read might fail - missing is fine.
    let loop_engine = AgentLoop::new(
        Arc::new(mock),
        "m",
        tools,
        AgentLoopConfig {
            max_turns: 2,
            stop_on_max_turns: true,
            ..Default::default()
        },
    );

    let output = loop_engine
        .run(RunInput {
            session: Session::new(dir.path().to_string_lossy(), "m"),
            prompt: "loop forever".into(),
            cancel: CancellationToken::new(),
            tool_ctx,
        })
        .await
        .unwrap();

    assert_eq!(output.status, TaskStatus::Failed);
    assert!(output
        .error
        .as_ref()
        .unwrap()
        .to_lowercase()
        .contains("max turns"));
    assert_eq!(output.turns, 2);
}

#[tokio::test]
async fn direct_answer_no_tools() {
    let dir = tempdir().unwrap();
    let (tools, tool_ctx) = setup_tools(dir.path());
    let mock = MockProvider::new(vec![MockResponse::text("m", "42")]);
    let loop_engine = AgentLoop::new(Arc::new(mock), "m", tools, AgentLoopConfig::default());

    let output = loop_engine
        .run(RunInput {
            session: Session::new(dir.path().to_string_lossy(), "m"),
            prompt: "what is 6*7?".into(),
            cancel: CancellationToken::new(),
            tool_ctx,
        })
        .await
        .unwrap();

    assert_eq!(output.status, TaskStatus::Succeeded);
    assert_eq!(output.turns, 1);
    assert_eq!(output.final_message.as_deref(), Some("42"));
    assert!(output.tool_results.is_empty());
}
