//! CDP browser tools (Obscura, Chrome, Chromium, custom endpoints).

mod cdp;
mod config;

pub use config::{BrowserBackend, BrowserConfig};

use crate::error::{Result, ToolError};
use crate::registry::ToolRegistry;
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use cdp::CdpSession;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared browser handle used by all browser_* tools.
#[derive(Clone)]
pub struct BrowserHandle {
    config: BrowserConfig,
    session: Arc<Mutex<Option<CdpSession>>>,
}

impl BrowserHandle {
    /// Create from config.
    pub fn new(config: BrowserConfig) -> Self {
        Self {
            config,
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// Default Obscura endpoint config (env-aware).
    pub fn from_env_or_default() -> Self {
        Self::new(BrowserConfig::from_env_or_default())
    }

    async fn ensure_session(&self) -> Result<()> {
        if !self.config.enabled {
            return Err(ToolError::Execution(
                "browser tools disabled (set enabled=true in browser.toml or CORTEX_BROWSER_ENABLED=1)"
                    .into(),
            ));
        }
        let mut guard = self.session.lock().await;
        if guard.is_none() {
            let endpoint = self
                .config
                .resolve_cdp_url()
                .await
                .map_err(ToolError::Execution)?;
            let sess = CdpSession::connect(&endpoint, self.config.timeout_secs).await?;
            *guard = Some(sess);
        }
        Ok(())
    }

    async fn navigate(&self, url: &str, wait_until: &str) -> Result<String> {
        self.ensure_session().await?;
        let mut guard = self.session.lock().await;
        let session = guard
            .as_mut()
            .ok_or_else(|| ToolError::Execution("browser session missing".into()))?;
        match session.navigate(url, wait_until).await {
            Ok(v) => Ok(v),
            Err(e) => {
                *guard = None;
                Err(e)
            }
        }
    }

    async fn evaluate(&self, expression: &str) -> Result<String> {
        self.ensure_session().await?;
        let mut guard = self.session.lock().await;
        let session = guard
            .as_mut()
            .ok_or_else(|| ToolError::Execution("browser session missing".into()))?;
        match session.evaluate(expression).await {
            Ok(v) => Ok(v),
            Err(e) => {
                *guard = None;
                Err(e)
            }
        }
    }

    async fn snapshot(&self) -> Result<String> {
        self.ensure_session().await?;
        let mut guard = self.session.lock().await;
        let session = guard
            .as_mut()
            .ok_or_else(|| ToolError::Execution("browser session missing".into()))?;
        match session.snapshot().await {
            Ok(v) => Ok(v),
            Err(e) => {
                *guard = None;
                Err(e)
            }
        }
    }

    async fn close(&self) -> Result<String> {
        let mut guard = self.session.lock().await;
        if let Some(sess) = guard.take() {
            let _ = sess.close().await;
            Ok("browser session closed".into())
        } else {
            Ok("no active browser session".into())
        }
    }
}

// --- Tools ---

/// Navigate the current page to a URL.
pub struct BrowserNavigateTool {
    handle: BrowserHandle,
}

impl BrowserNavigateTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct NavigateInput {
    url: String,
    #[serde(default)]
    wait_until: Option<String>,
}

#[async_trait]
impl Tool for BrowserNavigateTool {
    fn name(&self) -> &str {
        "browser_navigate"
    }

    fn description(&self) -> &str {
        "Navigate the CDP browser (Obscura/Chrome/etc.) to a URL."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" },
                "wait_until": {
                    "type": "string",
                    "description": "load | domcontentloaded | networkidle",
                    "default": "load"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: NavigateInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid browser_navigate args: {e}")))?;
        if args.url.trim().is_empty() {
            return Err(ToolError::InvalidInput("url must not be empty".into()));
        }
        let wait = args
            .wait_until
            .unwrap_or_else(|| self.handle.config.wait_until.clone());
        self.handle.navigate(&args.url, &wait).await
    }
}

/// Evaluate JavaScript in the page.
pub struct BrowserEvaluateTool {
    handle: BrowserHandle,
}

impl BrowserEvaluateTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct EvaluateInput {
    expression: String,
}

#[async_trait]
impl Tool for BrowserEvaluateTool {
    fn name(&self) -> &str {
        "browser_evaluate"
    }

    fn description(&self) -> &str {
        "Evaluate a JavaScript expression in the active browser page and return the result."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string", "description": "JS expression to evaluate" }
            },
            "required": ["expression"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: EvaluateInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid browser_evaluate args: {e}")))?;
        self.handle
            .evaluate(&args.expression)
            .await
            .map(|v| ctx.truncate_output(v))
    }
}

/// Snapshot URL, title, and body text.
pub struct BrowserSnapshotTool {
    handle: BrowserHandle,
}

impl BrowserSnapshotTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl Tool for BrowserSnapshotTool {
    fn name(&self) -> &str {
        "browser_snapshot"
    }

    fn description(&self) -> &str {
        "Return the current page URL, title, and visible body text from the CDP browser."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &ToolContext, _input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        self.handle.snapshot().await.map(|v| ctx.truncate_output(v))
    }
}

/// Get page HTML or text.
pub struct BrowserContentTool {
    handle: BrowserHandle,
}

impl BrowserContentTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct ContentInput {
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "html".into()
}

#[async_trait]
impl Tool for BrowserContentTool {
    fn name(&self) -> &str {
        "browser_content"
    }

    fn description(&self) -> &str {
        "Get page content as html or text from the CDP browser."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "format": { "type": "string", "enum": ["html", "text"], "default": "html" }
            }
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: ContentInput = serde_json::from_value(input).unwrap_or(ContentInput {
            format: "html".into(),
        });
        let expr = if args.format == "text" {
            "document.body ? document.body.innerText : ''"
        } else {
            "document.documentElement ? document.documentElement.outerHTML : ''"
        };
        self.handle
            .evaluate(expr)
            .await
            .map(|v| ctx.truncate_output(v))
    }
}

/// Click an element by CSS selector.
pub struct BrowserClickTool {
    handle: BrowserHandle,
}

impl BrowserClickTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[derive(Deserialize)]
struct ClickInput {
    selector: String,
}

#[async_trait]
impl Tool for BrowserClickTool {
    fn name(&self) -> &str {
        "browser_click"
    }

    fn description(&self) -> &str {
        "Click the first element matching a CSS selector in the CDP browser."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "selector": { "type": "string" }
            },
            "required": ["selector"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: ClickInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid browser_click args: {e}")))?;
        let sel = args.selector.replace('\\', "\\\\").replace('\'', "\\'");
        let expr = format!(
            r#"(function(){{
              const el = document.querySelector('{sel}');
              if (!el) return 'not found';
              el.click();
              return 'clicked';
            }})()"#
        );
        self.handle.evaluate(&expr).await
    }
}

/// Close the browser session (disconnect; does not kill Obscura/Chrome process).
pub struct BrowserCloseTool {
    handle: BrowserHandle,
}

impl BrowserCloseTool {
    /// Create tool.
    pub fn new(handle: BrowserHandle) -> Self {
        Self { handle }
    }
}

#[async_trait]
impl Tool for BrowserCloseTool {
    fn name(&self) -> &str {
        "browser_close"
    }

    fn description(&self) -> &str {
        "Close the Cortex CDP browser session (disconnect). Does not stop the Obscura/Chrome process."
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    async fn execute(&self, ctx: &ToolContext, _input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        self.handle.close().await
    }
}

/// Register all browser_* tools sharing one handle.
pub fn register_browser_tools(registry: &mut ToolRegistry, handle: BrowserHandle) {
    registry.register_or_replace(Arc::new(BrowserNavigateTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(BrowserEvaluateTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(BrowserSnapshotTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(BrowserContentTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(BrowserClickTool::new(handle.clone())));
    registry.register_or_replace(Arc::new(BrowserCloseTool::new(handle)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn navigate_rejects_empty_url() {
        let handle = BrowserHandle::new(BrowserConfig {
            enabled: false,
            ..BrowserConfig::default()
        });
        let tool = BrowserNavigateTool::new(handle);
        let err = tool
            .execute(
                &ToolContext::for_tests(std::env::temp_dir()),
                json!({ "url": "   " }),
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[tokio::test]
    async fn disabled_config_fails_closed() {
        let handle = BrowserHandle::new(BrowserConfig {
            enabled: false,
            ..BrowserConfig::default()
        });
        let tool = BrowserSnapshotTool::new(handle);
        let err = tool
            .execute(&ToolContext::for_tests(std::env::temp_dir()), json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("disabled"));
    }

    /// When nothing is listening on the CDP port, navigate fails with a clear fix hint.
    #[tokio::test]
    async fn navigate_fails_clearly_when_cdp_down() {
        let handle = BrowserHandle::new(BrowserConfig {
            enabled: true,
            // Unlikely to be a live browser; forces connection error.
            host: "127.0.0.1".into(),
            port: 1,
            cdp_url: "ws://127.0.0.1:1/devtools/browser".into(),
            ..BrowserConfig::default()
        });
        let tool = BrowserNavigateTool::new(handle);
        let err = tool
            .execute(
                &ToolContext::for_tests(std::env::temp_dir()),
                json!({ "url": "https://example.com" }),
            )
            .await
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("CDP connect failed") || msg.to_lowercase().contains("connect"),
            "unexpected error: {msg}"
        );
        assert!(
            msg.contains("obscura serve") || msg.contains("docs/browser.md"),
            "missing actionable hint: {msg}"
        );
    }

    #[tokio::test]
    async fn close_without_session() {
        let handle = BrowserHandle::from_env_or_default();
        let tool = BrowserCloseTool::new(handle);
        let out = tool
            .execute(&ToolContext::for_tests(std::env::temp_dir()), json!({}))
            .await
            .unwrap();
        assert!(out.contains("no active"));
    }

    /// Live smoke against a running CDP browser (Obscura/Chrome on :9222).
    /// Run with: `CORTEX_LIVE_CDP=1 cargo test -p cortex-tools live_navigate -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn live_navigate_example_com() {
        if std::env::var("CORTEX_LIVE_CDP").ok().as_deref() != Some("1") {
            return;
        }
        let handle = BrowserHandle::from_env_or_default();
        let nav = BrowserNavigateTool::new(handle.clone());
        let snap = BrowserSnapshotTool::new(handle.clone());
        let close = BrowserCloseTool::new(handle);
        let ctx = ToolContext::for_tests(std::env::temp_dir());
        let out = nav
            .execute(&ctx, json!({ "url": "https://example.com" }))
            .await
            .expect("navigate");
        assert!(out.contains("navigated"), "{out}");
        let shot = snap.execute(&ctx, json!({})).await.expect("snapshot");
        assert!(
            shot.to_lowercase().contains("example"),
            "unexpected snapshot: {shot}"
        );
        let _ = close.execute(&ctx, json!({})).await;
    }
}
