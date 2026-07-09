//! Evaluation harness: fixture-driven agent scoring with mock or live providers.

#![deny(missing_docs)]

use anyhow::{Context, Result};
use cortex_llm::{MockProvider, MockResponse, Provider};
use cortex_models::{Message, Session, TaskStatus, ToolCall};
use cortex_runtime::{AgentLoop, AgentLoopConfig, ContextBuilder, RunInput, SummarizeConfig};
use cortex_tools::{
    register_default_tools, AlwaysAllow, PermissionPolicy, ToolContext, ToolExecutor, ToolRegistry,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// One eval fixture (TOML file).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EvalFixture {
    /// Unique id.
    pub id: String,
    /// Human description.
    #[serde(default)]
    pub description: String,
    /// User prompt.
    pub prompt: String,
    /// Max turns.
    #[serde(default = "default_turns")]
    pub max_turns: u32,
    /// Substrings that must appear in the final message (case-insensitive).
    #[serde(default)]
    pub expect_contains: Vec<String>,
    /// Substrings that must **not** appear.
    #[serde(default)]
    pub expect_not_contains: Vec<String>,
    /// Expected tool names (any order, subset ok unless `expect_tools_exact`).
    #[serde(default)]
    pub expect_tools: Vec<String>,
    /// If true, tool names must match exactly (same multiset after sort).
    #[serde(default)]
    pub expect_tools_exact: bool,
    /// Mock script: sequential responses.
    #[serde(default)]
    pub mock: Vec<MockStep>,
    /// Expected status (`succeeded`, `failed`, …). Default succeeded.
    #[serde(default = "default_status")]
    pub expect_status: String,
}

fn default_turns() -> u32 {
    4
}
fn default_status() -> String {
    "succeeded".into()
}

/// One scripted mock LLM step.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MockStep {
    /// Plain assistant text.
    Text {
        /// Content.
        content: String,
    },
    /// Assistant requests tools.
    Tools {
        /// Optional prose.
        #[serde(default)]
        content: String,
        /// Tool calls.
        calls: Vec<MockToolCall>,
    },
}

/// Tool call in a mock script.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MockToolCall {
    /// Tool name.
    pub name: String,
    /// JSON arguments.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Result of scoring one fixture.
#[derive(Debug, Clone, Serialize)]
pub struct EvalCaseResult {
    /// Fixture id.
    pub id: String,
    /// Passed all checks.
    pub passed: bool,
    /// Human-readable checks.
    pub checks: Vec<String>,
    /// Failures only.
    pub failures: Vec<String>,
    /// Run status.
    pub status: String,
    /// Final message (truncated).
    pub final_message: Option<String>,
    /// Duration ms.
    pub duration_ms: u64,
    /// Turns.
    pub turns: u32,
}

/// Suite summary.
#[derive(Debug, Clone, Serialize)]
pub struct EvalReport {
    /// Case results.
    pub cases: Vec<EvalCaseResult>,
    /// Passed count.
    pub passed: usize,
    /// Failed count.
    pub failed: usize,
}

impl EvalReport {
    /// True if every case passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0
    }
}

/// Load a fixture from TOML text.
pub fn parse_fixture(text: &str) -> Result<EvalFixture> {
    toml::from_str(text).context("parse eval fixture TOML")
}

/// Load fixture from path.
pub fn load_fixture(path: impl AsRef<Path>) -> Result<EvalFixture> {
    let text = std::fs::read_to_string(path.as_ref())
        .with_context(|| format!("read {}", path.as_ref().display()))?;
    parse_fixture(&text)
}

/// Discover `*.toml` fixtures under a directory (non-recursive).
pub fn discover_fixtures(dir: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("read_dir {}", dir.display()))? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) == Some("toml") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}

/// Run one fixture in an isolated temp workspace with mock LLM + default tools.
pub async fn run_fixture(fixture: &EvalFixture) -> Result<EvalCaseResult> {
    let workspace = tempfile::tempdir().context("tempdir")?;
    // Seed a tiny workspace for tools.
    std::fs::write(
        workspace.path().join("README.md"),
        "# eval workspace\nhello cortex\n",
    )?;
    std::fs::write(workspace.path().join("note.txt"), "secret-note-value\n")?;

    let mut reg = ToolRegistry::new();
    register_default_tools(&mut reg)?;
    let tools = ToolExecutor::new(Arc::new(reg));
    let tool_ctx = ToolContext {
        workspace_root: workspace.path().to_path_buf(),
        session_id: None,
        cancel: CancellationToken::new(),
        permissions: Arc::new(PermissionPolicy::default().allow_all()),
        approver: Arc::new(AlwaysAllow),
        default_timeout: Duration::from_secs(15),
    };

    let provider: Arc<dyn Provider> = if fixture.mock.is_empty() {
        Arc::new(MockProvider::echo("eval default reply"))
    } else {
        Arc::new(MockProvider::new(mock_scripts(&fixture.mock)))
    };

    let agent = AgentLoop::new(
        provider,
        "eval-mock",
        tools,
        AgentLoopConfig {
            max_turns: fixture.max_turns,
            context: ContextBuilder::new("You are a test agent under evaluation."),
            temperature: None,
            max_tokens: None,
            stop_on_max_turns: true,
            summarize: SummarizeConfig {
                enabled: false,
                ..Default::default()
            },
            max_run_secs: 60,
            max_tool_calls_per_turn: 16,
            subagent_depth: 0,
            max_subagent_depth: 1,
            plan_mode: false,
            verify_after_writes: false,
            verify_command: None,
        },
    );

    let session = Session::new(workspace.path().to_string_lossy(), "eval-mock");
    let output = agent
        .run(RunInput {
            session,
            prompt: fixture.prompt.clone(),
            cancel: CancellationToken::new(),
            tool_ctx,
        })
        .await
        .context("agent run")?;

    score_fixture(fixture, &output)
}

fn mock_scripts(steps: &[MockStep]) -> Vec<MockResponse> {
    steps
        .iter()
        .map(|s| match s {
            MockStep::Text { content } => MockResponse::text("eval-mock", content),
            MockStep::Tools { content, calls } => {
                let tool_calls: Vec<ToolCall> = calls
                    .iter()
                    .map(|c| {
                        ToolCall::new(
                            &c.name,
                            if c.arguments.is_null() {
                                serde_json::json!({})
                            } else {
                                c.arguments.clone()
                            },
                        )
                    })
                    .collect();
                MockResponse::with_tools(
                    "eval-mock",
                    Message::assistant_with_tools(content, tool_calls),
                )
            }
        })
        .collect()
}

fn score_fixture(
    fixture: &EvalFixture,
    output: &cortex_runtime::RunOutput,
) -> Result<EvalCaseResult> {
    let mut checks = Vec::new();
    let mut failures = Vec::new();

    let status = format!("{:?}", output.status).to_ascii_lowercase();
    let expect_status = fixture.expect_status.to_ascii_lowercase();
    if status == expect_status
        || (expect_status == "succeeded" && matches!(output.status, TaskStatus::Succeeded))
    {
        checks.push(format!("status == {expect_status}"));
    } else {
        failures.push(format!("status: got {status}, want {expect_status}"));
    }

    let final_msg = output.final_message.clone().unwrap_or_default();
    let final_lc = final_msg.to_ascii_lowercase();

    for needle in &fixture.expect_contains {
        if final_lc.contains(&needle.to_ascii_lowercase()) {
            checks.push(format!("contains `{needle}`"));
        } else {
            failures.push(format!("missing expected substring `{needle}`"));
        }
    }
    for needle in &fixture.expect_not_contains {
        if final_lc.contains(&needle.to_ascii_lowercase()) {
            failures.push(format!("unexpected substring `{needle}`"));
        } else {
            checks.push(format!("does not contain `{needle}`"));
        }
    }

    let used: Vec<String> = output.tool_results.iter().map(|t| t.name.clone()).collect();
    if fixture.expect_tools_exact {
        let mut a = used.clone();
        let mut b = fixture.expect_tools.clone();
        a.sort();
        b.sort();
        if a == b {
            checks.push(format!("tools exact: {a:?}"));
        } else {
            failures.push(format!("tools exact: got {a:?}, want {b:?}"));
        }
    } else {
        for t in &fixture.expect_tools {
            if used.iter().any(|u| u == t) {
                checks.push(format!("used tool `{t}`"));
            } else {
                failures.push(format!("expected tool `{t}` not used (got {used:?})"));
            }
        }
    }

    let passed = failures.is_empty();
    info!(id = %fixture.id, passed, "eval case scored");

    Ok(EvalCaseResult {
        id: fixture.id.clone(),
        passed,
        checks,
        failures,
        status,
        final_message: output.final_message.clone().map(|s| {
            if s.len() > 500 {
                format!("{}…", &s[..500])
            } else {
                s
            }
        }),
        duration_ms: output.duration_ms,
        turns: output.turns,
    })
}

/// Run all fixtures in a directory.
pub async fn run_suite(dir: impl AsRef<Path>) -> Result<EvalReport> {
    let paths = discover_fixtures(dir)?;
    let mut cases = Vec::new();
    for p in paths {
        let fixture = load_fixture(&p).with_context(|| format!("load {}", p.display()))?;
        let result = run_fixture(&fixture)
            .await
            .with_context(|| format!("run {}", fixture.id))?;
        cases.push(result);
    }
    let passed = cases.iter().filter(|c| c.passed).count();
    let failed = cases.len() - passed;
    Ok(EvalReport {
        cases,
        passed,
        failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal() {
        let f = parse_fixture(
            r#"
id = "t1"
prompt = "hi"
expect_contains = ["hello"]
[[mock]]
type = "text"
content = "hello there"
"#,
        )
        .unwrap();
        assert_eq!(f.id, "t1");
        assert_eq!(f.mock.len(), 1);
    }

    #[tokio::test]
    async fn run_text_fixture() {
        let f = parse_fixture(
            r#"
id = "hello"
prompt = "greet"
expect_contains = ["hello"]
[[mock]]
type = "text"
content = "hello from mock"
"#,
        )
        .unwrap();
        let r = run_fixture(&f).await.unwrap();
        assert!(r.passed, "{:?}", r.failures);
    }

    #[tokio::test]
    async fn run_tool_fixture() {
        let f = parse_fixture(
            r#"
id = "read"
prompt = "read note"
expect_tools = ["read_file"]
expect_contains = ["secret-note"]
[[mock]]
type = "tools"
content = "reading"
[[mock.calls]]
name = "read_file"
arguments = { path = "note.txt" }
[[mock]]
type = "text"
content = "The file says secret-note-value"
"#,
        )
        .unwrap();
        let r = run_fixture(&f).await.unwrap();
        assert!(r.passed, "{:?}", r.failures);
    }
}
