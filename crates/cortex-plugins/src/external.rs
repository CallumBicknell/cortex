//! Directory / manifest plugins (dynamic, no recompile).

use crate::error::{PluginError, Result};
use crate::plugin::{Plugin, PluginContext, PluginMeta};
use async_trait::async_trait;
use cortex_tools::{Tool, ToolContext, ToolError};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;
use tracing::info;

/// Manifest for an on-disk plugin (`plugin.toml`).
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalManifest {
    /// Plugin id.
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// Description.
    #[serde(default)]
    pub description: String,
    /// Tools contributed by this plugin.
    #[serde(default)]
    pub tools: Vec<ExternalToolDef>,
}

fn default_version() -> String {
    "0.1.0".into()
}

/// One tool defined in a plugin manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalToolDef {
    /// Tool name (must be unique globally when registered).
    pub name: String,
    /// Description for the LLM.
    #[serde(default)]
    pub description: String,
    /// Command argv (first element is the binary).
    /// Placeholders: `{workspace}`, `{args_json}`, `{arg:KEY}`.
    pub command: Vec<String>,
    /// Working directory relative to plugin dir (default: plugin root).
    #[serde(default)]
    pub cwd: Option<String>,
    /// Timeout seconds (default 60).
    #[serde(default = "default_tool_timeout")]
    pub timeout_secs: u64,
    /// When true, non-zero process exit still returns stdout/stderr as success.
    /// Needed for analyzers like Slither that exit non-zero when findings exist.
    #[serde(default)]
    pub allow_nonzero: bool,
    /// JSON schema fragment for parameters (object properties).
    #[serde(default)]
    pub parameters: Value,
}

fn default_tool_timeout() -> u64 {
    60
}

/// Load `plugin.toml` from a directory.
pub fn load_manifest(dir: impl AsRef<Path>) -> Result<ExternalManifest> {
    let path = dir.as_ref().join("plugin.toml");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| PluginError::Config(format!("read {}: {e}", path.display())))?;
    let mut m: ExternalManifest =
        toml::from_str(&text).map_err(|e| PluginError::Config(e.to_string()))?;
    if m.name.is_empty() {
        m.name = m.id.clone();
    }
    if m.description.is_empty() {
        m.description = format!("External plugin `{}`", m.id);
    }
    Ok(m)
}

/// Discover plugin directories under `roots` (each child dir with plugin.toml).
pub fn discover_plugin_dirs(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() && p.join("plugin.toml").is_file() {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Plugin loaded from a directory manifest.
pub struct ExternalPlugin {
    dir: PathBuf,
    manifest: ExternalManifest,
}

impl ExternalPlugin {
    /// Create from directory (loads manifest).
    pub fn from_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        let manifest = load_manifest(&dir)?;
        Ok(Self { dir, manifest })
    }
}

#[async_trait]
impl Plugin for ExternalPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new(
            self.manifest.id.clone(),
            self.manifest.name.clone(),
            self.manifest.version.clone(),
            self.manifest.description.clone(),
        )
    }

    async fn init(&mut self, ctx: &mut PluginContext<'_>) -> Result<()> {
        for def in &self.manifest.tools {
            let tool = Arc::new(ExternalCommandTool {
                def: def.clone(),
                plugin_dir: self.dir.clone(),
                workspace: ctx.workspace.clone(),
            });
            ctx.tools.register_or_replace(tool);
            info!(
                plugin = %self.manifest.id,
                tool = %def.name,
                "registered external plugin tool"
            );
        }
        Ok(())
    }
}

struct ExternalCommandTool {
    def: ExternalToolDef,
    plugin_dir: PathBuf,
    workspace: PathBuf,
}

#[async_trait]
impl Tool for ExternalCommandTool {
    fn name(&self) -> &str {
        &self.def.name
    }

    fn description(&self) -> &str {
        if self.def.description.is_empty() {
            "External plugin tool"
        } else {
            &self.def.description
        }
    }

    fn parameters_schema(&self) -> Value {
        if self.def.parameters.is_null()
            || self
                .def
                .parameters
                .as_object()
                .map(|o| o.is_empty())
                .unwrap_or(true)
        {
            json!({
                "type": "object",
                "properties": {
                    "args": { "type": "string", "description": "Optional free-form args" }
                }
            })
        } else {
            json!({
                "type": "object",
                "properties": self.def.parameters,
            })
        }
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> cortex_tools::Result<String> {
        ctx.check_cancelled()?;
        let args_json =
            serde_json::to_string(&input).map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        let mut argv: Vec<String> = self
            .def
            .command
            .iter()
            .map(|part| expand_placeholders(part, &self.workspace, &args_json, &input))
            .collect();
        if argv.is_empty() {
            return Err(ToolError::Execution("plugin tool has empty command".into()));
        }
        let program = argv.remove(0);
        let cwd = match self.def.cwd.as_deref() {
            None => self.plugin_dir.clone(),
            Some("{workspace}") | Some("workspace") => self.workspace.clone(),
            Some(rel) if rel.starts_with("{workspace}") => {
                // `{workspace}/subdir` style
                let rest = rel
                    .trim_start_matches("{workspace}")
                    .trim_start_matches('/');
                if rest.is_empty() {
                    self.workspace.clone()
                } else {
                    self.workspace.join(rest)
                }
            }
            Some(rel) => self.plugin_dir.join(rel),
        };

        let mut cmd = Command::new(&program);
        cmd.args(&argv)
            .current_dir(&cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .env("CORTEX_WORKSPACE", self.workspace.as_os_str())
            .env("CORTEX_PLUGIN_DIR", self.plugin_dir.as_os_str())
            .env("CORTEX_TOOL_ARGS_JSON", &args_json);

        for key in &ctx.permissions.scrub_env {
            cmd.env_remove(key);
        }

        let timeout_dur = Duration::from_secs(self.def.timeout_secs.max(1));
        let output = tokio::select! {
            _ = ctx.cancel.cancelled() => {
                return Err(ToolError::Cancelled("external plugin tool cancelled".into()));
            }
            res = timeout(timeout_dur, cmd.output()) => {
                match res {
                    Ok(Ok(o)) => o,
                    Ok(Err(e)) => {
                        return Err(ToolError::Execution(format!(
                            "failed to spawn plugin tool `{}`: {e}",
                            self.def.name
                        )));
                    }
                    Err(_) => {
                        return Err(ToolError::Timeout(format!(
                            "plugin tool `{}` timed out after {}s",
                            self.def.name,
                            timeout_dur.as_secs()
                        )));
                    }
                }
            }
        };

        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str("--- stderr ---\n");
            text.push_str(&stderr);
        }
        let code = output.status.code().unwrap_or(-1);
        if !output.status.success() {
            if self.def.allow_nonzero {
                // Analyzers often exit non-zero when findings exist — keep output.
                let header = format!(
                    "[exit {code} — non-zero allowed for tool `{}`]\n",
                    self.def.name
                );
                text = format!("{header}{text}");
            } else {
                return Err(ToolError::Execution(if text.is_empty() {
                    format!("plugin tool `{}` failed (exit {code})", self.def.name)
                } else {
                    text
                }));
            }
        }
        if text.is_empty() {
            text = "(no output)".into();
        }
        Ok(ctx.truncate_output(text))
    }
}

fn expand_placeholders(part: &str, workspace: &Path, args_json: &str, input: &Value) -> String {
    let mut s = part
        .replace("{workspace}", &workspace.to_string_lossy())
        .replace("{args_json}", args_json);
    if let Some(obj) = input.as_object() {
        for (k, v) in obj {
            let needle = format!("{{arg:{k}}}");
            let val = match v {
                Value::String(x) => x.clone(),
                other => other.to_string(),
            };
            s = s.replace(&needle, &val);
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_tools::ToolRegistry;
    use tempfile::tempdir;

    #[test]
    fn allow_nonzero_deserializes() {
        let dir = tempdir().unwrap();
        let plug = dir.path().join("nz");
        std::fs::create_dir_all(&plug).unwrap();
        std::fs::write(
            plug.join("plugin.toml"),
            r#"
id = "nz"
[[tools]]
name = "t"
command = ["false"]
allow_nonzero = true
"#,
        )
        .unwrap();
        let m = load_manifest(&plug).unwrap();
        assert!(m.tools[0].allow_nonzero);
    }

    #[test]
    fn expand_workspace_placeholder() {
        let ws = PathBuf::from("/tmp/proj");
        let s = expand_placeholders("cd {workspace}", &ws, "{}", &json!({}));
        assert_eq!(s, "cd /tmp/proj");
        let s = expand_placeholders("{arg:pattern}", &ws, "{}", &json!({"pattern": "test/*"}));
        assert_eq!(s, "test/*");
    }

    #[test]
    fn load_and_discover() {
        let dir = tempdir().unwrap();
        let plug = dir.path().join("hello_ext");
        std::fs::create_dir_all(&plug).unwrap();
        std::fs::write(
            plug.join("plugin.toml"),
            r#"
id = "hello_ext"
description = "test external"
[[tools]]
name = "ext_echo"
description = "echo via printf"
command = ["printf", "%s", "{arg:message}"]
[tools.parameters]
message = { type = "string" }
"#,
        )
        .unwrap();
        let m = load_manifest(&plug).unwrap();
        assert_eq!(m.id, "hello_ext");
        assert_eq!(m.tools.len(), 1);
        let found = discover_plugin_dirs(&[dir.path().to_path_buf()]);
        assert_eq!(found.len(), 1);
    }

    #[tokio::test]
    async fn external_tool_runs() {
        let dir = tempdir().unwrap();
        let plug = dir.path().join("hello_ext");
        std::fs::create_dir_all(&plug).unwrap();
        std::fs::write(
            plug.join("plugin.toml"),
            r#"
id = "hello_ext"
[[tools]]
name = "ext_echo"
command = ["printf", "%s", "{arg:message}"]
"#,
        )
        .unwrap();
        let mut plugin = ExternalPlugin::from_dir(&plug).unwrap();
        let mut reg = ToolRegistry::new();
        let mut ctx = PluginContext::new(dir.path(), &mut reg, Value::Null);
        plugin.init(&mut ctx).await.unwrap();
        let tool = reg.get("ext_echo").unwrap();
        let out = tool
            .execute(
                &ToolContext::for_tests(dir.path()),
                json!({"message": "hi-ext"}),
            )
            .await
            .unwrap();
        assert!(out.contains("hi-ext"));
    }
}
