//! Persist smart-contract audit / x-ray reports under `.cortex/audits/`.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Write a durable audit or pre-audit report into the workspace.
pub struct WriteAuditReportTool;

#[derive(Deserialize)]
struct WriteInput {
    /// Full markdown report body.
    markdown: String,
    /// Optional findings JSON (array or object); written alongside as `.json`.
    #[serde(default)]
    findings_json: Option<Value>,
    /// `audit` (default) or `xray`.
    #[serde(default = "default_kind")]
    kind: String,
    /// Short slug for the filename (sanitized).
    #[serde(default)]
    title: Option<String>,
    /// Relative path override under workspace (optional).
    #[serde(default)]
    path: Option<String>,
}

fn default_kind() -> String {
    "audit".into()
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars().take(48) {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if (c == '-' || c == '_' || c.is_whitespace())
            && !out.ends_with('-')
            && !out.is_empty()
        {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "report".into()
    } else {
        out
    }
}

fn timestamp_slug() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Compact UTC-ish stamp without chrono dependency.
    format!("{secs}")
}

fn simple_sha256_hex(data: &[u8]) -> String {
    // Lightweight non-crypto fingerprint for artifact metadata (not for security).
    // Prefer real sha2 if added later; for now FNV-ish stable hex for size+content.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h = h.wrapping_add(data.len() as u64);
    format!("{h:016x}")
}

fn audits_dir(workspace: &Path) -> PathBuf {
    workspace.join(".cortex").join("audits")
}

#[async_trait]
impl Tool for WriteAuditReportTool {
    fn name(&self) -> &str {
        "write_audit_report"
    }

    fn description(&self) -> &str {
        "Write a smart-contract audit or x-ray report under `.cortex/audits/` \
         (markdown + optional findings JSON). Call at the end of an audit or \
         pre-audit when the user wants a durable artifact. Returns paths and a \
         content fingerprint."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "markdown": {
                    "type": "string",
                    "description": "Full markdown report body"
                },
                "findings_json": {
                    "description": "Optional findings array/object (schema_version + findings)"
                },
                "kind": {
                    "type": "string",
                    "enum": ["audit", "xray"],
                    "default": "audit"
                },
                "title": {
                    "type": "string",
                    "description": "Short title used in the filename"
                },
                "path": {
                    "type": "string",
                    "description": "Optional relative path under workspace for the .md file"
                }
            },
            "required": ["markdown"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: WriteInput = serde_json::from_value(input).map_err(|e| {
            ToolError::InvalidInput(format!("invalid write_audit_report args: {e}"))
        })?;
        if args.markdown.trim().is_empty() {
            return Err(ToolError::InvalidInput("markdown must not be empty".into()));
        }
        let kind = args.kind.to_ascii_lowercase();
        if kind != "audit" && kind != "xray" {
            return Err(ToolError::InvalidInput(
                "kind must be `audit` or `xray`".into(),
            ));
        }

        let title_slug = slugify(args.title.as_deref().unwrap_or(kind.as_str()));
        let ts = timestamp_slug();
        let default_rel = format!(".cortex/audits/{ts}-{title_slug}-{kind}-report.md");
        let rel_md = args
            .path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .unwrap_or(default_rel);

        // Ensure under workspace and preferably under .cortex/audits when using default.
        let abs_md = ctx.resolve_path(&rel_md)?;
        if let Some(parent) = abs_md.parent() {
            fs::create_dir_all(parent).map_err(ToolError::Io)?;
        }

        // Prepend front matter if missing.
        let body = if args.markdown.trim_start().starts_with("---") {
            args.markdown.clone()
        } else {
            let session = ctx
                .session_id
                .map(|id| id.to_string())
                .unwrap_or_else(|| "none".into());
            format!(
                "---\nkind: {kind}\ntitle: {title_slug}\nsession_id: {session}\n---\n\n{}",
                args.markdown.trim_start()
            )
        };

        fs::write(&abs_md, &body).map_err(ToolError::Io)?;
        let fingerprint = simple_sha256_hex(body.as_bytes());
        let size = body.len();

        let mut json_rel: Option<String> = None;
        if let Some(findings) = args.findings_json {
            let rel_json = rel_md
                .strip_suffix(".md")
                .map(|s| format!("{s}.json"))
                .unwrap_or_else(|| format!("{rel_md}.json"));
            let abs_json = ctx.resolve_path(&rel_json)?;
            if let Some(parent) = abs_json.parent() {
                fs::create_dir_all(parent).map_err(ToolError::Io)?;
            }
            let payload = if findings.get("schema_version").is_some() {
                findings
            } else {
                json!({
                    "schema_version": 1,
                    "kind": kind,
                    "title": title_slug,
                    "findings": findings,
                })
            };
            let text = serde_json::to_string_pretty(&payload)
                .map_err(|e| ToolError::Execution(e.to_string()))?;
            fs::write(&abs_json, text).map_err(ToolError::Io)?;
            json_rel = Some(rel_json);
        }

        // Sidecar meta for session linkage without requiring DB in the tool.
        let meta_rel = rel_md
            .strip_suffix(".md")
            .map(|s| format!("{s}.meta.json"))
            .unwrap_or_else(|| format!("{rel_md}.meta.json"));
        if let Ok(abs_meta) = ctx.resolve_path(&meta_rel) {
            let meta = json!({
                "kind": kind,
                "title": title_slug,
                "markdown_path": rel_md,
                "json_path": json_rel,
                "session_id": ctx.session_id.map(|id| id.to_string()),
                "fingerprint": fingerprint,
                "size_bytes": size,
            });
            let _ = fs::write(
                abs_meta,
                serde_json::to_string_pretty(&meta).unwrap_or_default(),
            );
        }

        let mut out = format!(
            "Wrote audit artifact:\n- markdown: `{rel_md}` ({size} bytes)\n- fingerprint: `{fingerprint}`\n"
        );
        if let Some(j) = &json_rel {
            out.push_str(&format!("- findings_json: `{j}`\n"));
        }
        out.push_str(&format!("- meta: `{meta_rel}`\n"));
        out.push_str(
            "Tip: run `cortex memory index` (or memory tools) to make the report searchable.\n",
        );
        // Ensure audits dir exists even if custom path outside it.
        let _ = fs::create_dir_all(audits_dir(&ctx.workspace_root));
        Ok(ctx.truncate_output(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolContext;
    use tempfile::tempdir;

    #[tokio::test]
    async fn writes_markdown_and_json() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        let tool = WriteAuditReportTool;
        let out = tool
            .execute(
                &ctx,
                json!({
                    "markdown": "### [High] Reentrancy\n- **Root cause:** CEI\n",
                    "kind": "audit",
                    "title": "Vault reentrancy",
                    "findings_json": [{
                        "id": "F-001",
                        "severity": "high",
                        "title": "Reentrancy",
                        "bug_class": "reentrancy"
                    }]
                }),
            )
            .await
            .unwrap();
        assert!(out.contains("markdown:"), "{out}");
        assert!(out.contains("findings_json:"), "{out}");
        // File exists
        let audits = dir.path().join(".cortex/audits");
        assert!(audits.is_dir());
        let count = fs::read_dir(&audits).unwrap().count();
        assert!(count >= 2, "expected md + json + meta, got {count}");
    }

    #[test]
    fn slugify_basic() {
        assert_eq!(slugify("Vault Reentrancy!"), "vault-reentrancy");
    }
}
