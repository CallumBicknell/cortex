//! Builtin `echo` plugin — registers a `plugin_echo` tool (demo / smoke).

use crate::error::Result;
use crate::plugin::{Plugin, PluginContext, PluginMeta};
use async_trait::async_trait;
use cortex_tools::{Tool, ToolContext};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

/// Demo plugin that contributes one tool.
pub struct EchoPlugin {
    prefix: String,
}

impl EchoPlugin {
    /// Create with default prefix.
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
        }
    }
}

impl Default for EchoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Plugin for EchoPlugin {
    fn meta(&self) -> PluginMeta {
        PluginMeta::new(
            "echo",
            "Echo",
            env!("CARGO_PKG_VERSION"),
            "Demo plugin that registers plugin_echo (echoes a message)",
        )
    }

    async fn init(&mut self, ctx: &mut PluginContext<'_>) -> Result<()> {
        if let Some(p) = ctx.settings.get("prefix").and_then(|v| v.as_str()) {
            self.prefix = p.to_string();
        }
        let tool = Arc::new(EchoTool {
            prefix: self.prefix.clone(),
        });
        ctx.tools.register_or_replace(tool);
        tracing::info!(plugin = "echo", "registered plugin_echo tool");
        Ok(())
    }
}

struct EchoTool {
    prefix: String,
}

#[derive(Deserialize)]
struct EchoInput {
    message: String,
}

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "plugin_echo"
    }

    fn description(&self) -> &str {
        "Echo a message (contributed by the builtin echo plugin)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": { "type": "string" }
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> cortex_tools::Result<String> {
        ctx.check_cancelled()?;
        let args: EchoInput = serde_json::from_value(input).map_err(|e| {
            cortex_tools::ToolError::InvalidInput(format!("invalid plugin_echo args: {e}"))
        })?;
        if self.prefix.is_empty() {
            Ok(args.message)
        } else {
            Ok(format!("{}{}", self.prefix, args.message))
        }
    }
}
