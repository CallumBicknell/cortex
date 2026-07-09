//! `audit_lenses` — parallel specialty security sub-agents for Solidity audits.

use crate::audit_bundle::{write_source_bundle, DEFAULT_MAX_BUNDLE_BYTES};
use crate::subagent::{format_subagent_result, run_subagent, SubAgentOptions, SubAgentParent};
use crate::subagent_tool::SubAgentHandle;
use async_trait::async_trait;
use cortex_tools::{Result as ToolResult, Tool, ToolContext, ToolError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::info;

/// Built-in lens definitions (id → specialty prompt body).
pub fn builtin_lenses() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "access",
            include_str!("../../../prompts/skills/audit_lenses/access.md"),
        ),
        (
            "reentrancy",
            include_str!("../../../prompts/skills/audit_lenses/reentrancy.md"),
        ),
        (
            "economic",
            include_str!("../../../prompts/skills/audit_lenses/economic.md"),
        ),
        (
            "proxy",
            include_str!("../../../prompts/skills/audit_lenses/proxy.md"),
        ),
        (
            "invariants",
            include_str!("../../../prompts/skills/audit_lenses/invariants.md"),
        ),
    ]
}

const SHARED_SCHEMA: &str = include_str!("../../../prompts/skills/audit_lenses/_shared_schema.md");

/// Default lens ids when the caller omits `lenses`.
pub fn default_lens_ids() -> Vec<&'static str> {
    // Cap default to 4 for cost; invariants still available on request.
    vec!["access", "reentrancy", "economic", "proxy"]
}

/// Tools children may use (no nesting / no audit fan-out).
fn child_tool_allowlist() -> Vec<String> {
    [
        "read_file",
        "list_dir",
        "glob_files",
        "code_outline",
        "workspace_symbols",
        "code_definition",
        "shell",
        "git_status",
        "git_diff",
        "git_log",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Tool that fans out specialty audit lenses in parallel.
pub struct AuditLensesTool {
    handle: SubAgentHandle,
}

impl AuditLensesTool {
    /// Create tool bound to a sub-agent handle.
    pub fn new(handle: SubAgentHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct AuditLensesInput {
    /// Relative path or `.` for whole workspace.
    #[serde(default = "default_scope")]
    scope: String,
    /// Lens ids: access, reentrancy, economic, proxy, invariants.
    #[serde(default)]
    lenses: Option<Vec<String>>,
    /// Max turns per lens sub-agent.
    #[serde(default)]
    max_turns: Option<u32>,
    /// Max concurrent lens agents (default 4, hard cap 5).
    #[serde(default)]
    max_concurrent: Option<u32>,
    /// Skip writing source bundle (lenses only use tools).
    #[serde(default)]
    skip_bundle: bool,
}

fn default_scope() -> String {
    ".".into()
}

fn resolve_lenses(requested: Option<Vec<String>>) -> Result<Vec<(String, String)>, ToolError> {
    let all = builtin_lenses();
    let ids: Vec<String> = match requested {
        Some(v) if !v.is_empty() => v.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
        _ => default_lens_ids().into_iter().map(String::from).collect(),
    };
    if ids.len() > 5 {
        return Err(ToolError::InvalidInput(
            "at most 5 lenses per audit_lenses call".into(),
        ));
    }
    let mut out = Vec::new();
    for id in ids {
        let prompt = all
            .iter()
            .find(|(k, _)| *k == id.as_str())
            .map(|(_, p)| (*p).to_string())
            .ok_or_else(|| {
                ToolError::InvalidInput(format!(
                    "unknown lens `{id}`; valid: access, reentrancy, economic, proxy, invariants"
                ))
            })?;
        out.push((id, prompt));
    }
    Ok(out)
}

fn build_lens_prompt(
    lens_id: &str,
    specialty: &str,
    scope: &str,
    bundle_rel: Option<&str>,
    file_count: usize,
) -> String {
    let mut p = String::new();
    p.push_str(&format!("# Multi-lens audit — specialty `{lens_id}`\n\n"));
    p.push_str(specialty);
    p.push('\n');
    p.push_str(SHARED_SCHEMA);
    p.push_str("\n## Scope\n\n");
    p.push_str(&format!("Workspace scope path: `{scope}`\n"));
    p.push_str(&format!(
        "In-scope Solidity files in bundle: {file_count}\n"
    ));
    if let Some(rel) = bundle_rel {
        p.push_str(&format!(
            "\nRead the shared source snapshot first with `read_file`:\n`{rel}`\n\
             Prefer the bundle for the initial pass. Use tools only for cross-file checks.\n"
        ));
    } else {
        p.push_str(
            "\nNo source bundle was written. Use `glob_files` / `read_file` to load `.sol` \
             sources (skip lib/, test/, mocks).\n",
        );
    }
    p.push_str(
        "\nWhen finished, reply with your FINDING/LEAD list only (no tool calls). \
         If nothing material, say so in one short paragraph.\n",
    );
    p
}

#[async_trait]
impl Tool for AuditLensesTool {
    fn name(&self) -> &str {
        "audit_lenses"
    }

    fn description(&self) -> &str {
        "Run parallel specialty smart-contract security lenses (access control, reentrancy, \
         economic/oracle, proxy/storage, optional invariants). Builds a shared Solidity source \
         bundle, spawns bounded sub-agents concurrently, and returns concatenated findings. \
         Use during audits; then dedupe findings in your final report."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "description": "Relative path to scan (default `.`)",
                    "default": "."
                },
                "lenses": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Lens ids: access, reentrancy, economic, proxy, invariants \
                                    (default: access, reentrancy, economic, proxy)"
                },
                "max_turns": {
                    "type": "integer",
                    "description": "Max turns per lens (default 6)",
                    "default": 6
                },
                "max_concurrent": {
                    "type": "integer",
                    "description": "Max concurrent lens agents (default 4, max 5)",
                    "default": 4
                },
                "skip_bundle": {
                    "type": "boolean",
                    "description": "If true, do not write a source.md bundle",
                    "default": false
                }
            }
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> ToolResult<String> {
        ctx.check_cancelled()?;
        let args: AuditLensesInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid audit_lenses args: {e}")))?;

        let lenses = resolve_lenses(args.lenses)?;
        let max_turns = args.max_turns.unwrap_or(6).clamp(1, 12);
        let max_concurrent = args.max_concurrent.unwrap_or(4).clamp(1, 5) as usize;

        let parent_cfg = self
            .handle
            .parent_config
            .lock()
            .expect("config lock")
            .clone();

        let (bundle_abs, bundle_meta) = if args.skip_bundle {
            (None, None)
        } else {
            let (path, meta) =
                write_source_bundle(&ctx.workspace_root, &args.scope, DEFAULT_MAX_BUNDLE_BYTES)
                    .map_err(|e| ToolError::Execution(format!("source bundle failed: {e}")))?;
            (Some(path), Some(meta))
        };

        let bundle_rel = bundle_abs.as_ref().map(|p| {
            p.strip_prefix(&ctx.workspace_root)
                .unwrap_or(p)
                .display()
                .to_string()
        });
        let file_count = bundle_meta.as_ref().map(|m| m.files.len()).unwrap_or(0);
        let truncated = bundle_meta.as_ref().map(|m| m.truncated).unwrap_or(false);

        info!(
            lenses = lenses.len(),
            file_count, max_turns, max_concurrent, "starting audit_lenses"
        );

        let sem = Arc::new(Semaphore::new(max_concurrent));
        let mut join_set = tokio::task::JoinSet::new();

        for (lens_id, specialty) in lenses {
            let permit = Arc::clone(&sem)
                .acquire_owned()
                .await
                .map_err(|e| ToolError::Execution(e.to_string()))?;

            let prompt = build_lens_prompt(
                &lens_id,
                &specialty,
                &args.scope,
                bundle_rel.as_deref(),
                file_count,
            );
            let provider = Arc::clone(&self.handle.provider);
            let model = self.handle.model.clone();
            let tools = self.handle.tools.clone();
            let parent_cfg = parent_cfg.clone();
            let cancel = ctx.cancel.child_token();
            let mut child_ctx = ctx.clone();
            child_ctx.cancel = cancel.clone();
            let parent = SubAgentParent {
                session_id: ctx.session_id,
                run_id: None,
                bus: self.handle.bus.clone(),
            };
            let opts = SubAgentOptions {
                prompt,
                max_turns,
                allowed_tools: child_tool_allowlist(),
                disable_summarize: true,
            };

            join_set.spawn(async move {
                let _permit = permit;
                let result = run_subagent(
                    provider,
                    model,
                    tools,
                    &parent_cfg,
                    child_ctx,
                    cancel,
                    opts,
                    parent,
                )
                .await;
                (lens_id, result)
            });
        }

        let mut sections = Vec::new();
        sections.push(format!(
            "# audit_lenses results\n\nscope: `{}`\nfiles_in_bundle: {file_count}\n\
             truncated: {truncated}\nbundle: {}\n\n---\n",
            args.scope,
            bundle_rel.as_deref().unwrap_or("(none)"),
        ));

        while let Some(joined) = join_set.join_next().await {
            ctx.check_cancelled()?;
            match joined {
                Ok((lens_id, Ok(out))) => {
                    sections.push(format!(
                        "## Lens: `{lens_id}`\n\nstatus: {:?}\nturns: {}\n\n{}\n",
                        out.status,
                        out.turns,
                        format_subagent_result(&out)
                    ));
                }
                Ok((lens_id, Err(e))) => {
                    sections.push(format!("## Lens: `{lens_id}`\n\n**error:** {e}\n"));
                }
                Err(e) => {
                    sections.push(format!("## Lens join error\n\n{e}\n"));
                }
            }
        }

        sections.push(
            "\n---\n## Orchestrator next step\n\nDeduplicate findings by \
             (contract, function, bug_class). Keep best proof. Promote LEADs only with evidence. \
             Emit the final audit report with severity counts. Do not re-run all lenses unless gaps remain.\n"
                .into(),
        );

        Ok(ctx.truncate_output(sections.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_loop::AgentLoopConfig;
    use crate::subagent_tool::SubAgentHandle;
    use cortex_llm::MockProvider;
    use cortex_tools::{
        register_default_tools, AlwaysAllow, PermissionPolicy, ToolContext, ToolExecutor,
        ToolRegistry,
    };
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio_util::sync::CancellationToken;

    #[test]
    fn resolve_default_lenses() {
        let v = resolve_lenses(None).unwrap();
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].0, "access");
    }

    #[test]
    fn resolve_rejects_unknown() {
        assert!(resolve_lenses(Some(vec!["nope".into()])).is_err());
    }

    #[tokio::test]
    async fn parallel_lenses_with_mock() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(
            dir.path().join("src/Vault.sol"),
            r#"// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;
contract Vault {
    mapping(address => uint256) public balances;
    function withdraw() external {
        uint256 bal = balances[msg.sender];
        (bool ok,) = msg.sender.call{value: bal}("");
        require(ok);
        balances[msg.sender] = 0;
    }
}
"#,
        )
        .unwrap();

        let mut reg = ToolRegistry::new();
        register_default_tools(&mut reg).unwrap();
        let tools = ToolExecutor::new(Arc::new(reg));

        // Fallback text so each lens gets a final answer without tool calls.
        let provider = Arc::new(
            MockProvider::echo(
                "### [High] Reentrancy on withdraw\n\
                 - **Contract / function:** Vault.sol:withdraw\n\
                 - **Bug class:** reentrancy\n\
                 - **Root cause:** balance zeroed after call\n\
                 - **Impact:** drain\n\
                 - **Proof:** CEI violation\n\
                 - **Minimal fix:** zero before call\n\
                 - **Confidence:** high\n",
            )
            .with_stream_deltas(false),
        );

        let cfg = AgentLoopConfig {
            max_turns: 4,
            max_subagent_depth: 2,
            subagent_depth: 0,
            ..Default::default()
        };
        let handle = SubAgentHandle::new(provider, "mock", tools, cfg);
        let tool = AuditLensesTool::new(handle);

        let ctx = ToolContext {
            workspace_root: dir.path().to_path_buf(),
            session_id: None,
            cancel: CancellationToken::new(),
            permissions: Arc::new(PermissionPolicy::default().allow_all()),
            approver: Arc::new(AlwaysAllow),
            default_timeout: Duration::from_secs(30),
        };

        let out = tool
            .execute(
                &ctx,
                json!({
                    "scope": ".",
                    "lenses": ["access", "reentrancy"],
                    "max_turns": 2,
                    "max_concurrent": 2
                }),
            )
            .await
            .unwrap();

        assert!(out.contains("Lens: `access`"), "{out}");
        assert!(out.contains("Lens: `reentrancy`"), "{out}");
        assert!(
            out.contains("Reentrancy") || out.contains("reentrancy"),
            "{out}"
        );
        assert!(out.contains("Orchestrator next step"));
        // Bundle should exist under .cortex/tmp
        assert!(
            out.contains(".cortex/tmp") || out.contains("files_in_bundle: 1"),
            "{out}"
        );
    }
}
