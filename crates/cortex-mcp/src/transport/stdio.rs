//! Stdio MCP transport with Content-Length framing (LSP-style).

use super::McpTransport;
use crate::error::{McpError, Result};
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tracing::{debug, warn};

/// Connected stdio MCP process.
pub struct StdioTransport {
    child: Child,
    stdin: Mutex<ChildStdin>,
    stdout: Mutex<BufReader<ChildStdout>>,
    next_id: AtomicU64,
}

impl StdioTransport {
    /// Spawn `command` with `args` and optional cwd/env.
    pub async fn spawn(
        command: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &std::collections::HashMap<String, String>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(McpError::Io)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Protocol("missing stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Protocol("missing stdout".into()))?;
        Ok(Self {
            child,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(BufReader::new(stdout)),
            next_id: AtomicU64::new(1),
        })
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn write_message(&self, req: &JsonRpcRequest) -> Result<()> {
        let body = serde_json::to_vec(req).map_err(|e| McpError::Protocol(e.to_string()))?;
        self.write_bytes(&body).await
    }

    async fn write_raw(&self, value: &serde_json::Value) -> Result<()> {
        let body = serde_json::to_vec(value).map_err(|e| McpError::Protocol(e.to_string()))?;
        self.write_bytes(&body).await
    }

    async fn write_bytes(&self, body: &[u8]) -> Result<()> {
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(body).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn read_message(&self) -> Result<JsonRpcResponse> {
        let mut stdout = self.stdout.lock().await;
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let n = stdout.read_line(&mut line).await?;
            if n == 0 {
                return Err(McpError::Protocol("MCP server closed stdout".into()));
            }
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                break;
            }
            if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(
                    rest.trim()
                        .parse()
                        .map_err(|e| McpError::Protocol(format!("bad Content-Length: {e}")))?,
                );
            }
        }
        let len = content_length
            .ok_or_else(|| McpError::Protocol("missing Content-Length header".into()))?;
        let mut buf = vec![0u8; len];
        stdout.read_exact(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| McpError::Protocol(format!("invalid JSON: {e}")))
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let id = self.next_id();
        let req = JsonRpcRequest::new(id, method, params);
        self.write_message(&req).await?;
        loop {
            let resp = self.read_message().await?;
            let resp_id = resp.id.as_ref().and_then(|v| v.as_u64());
            if resp_id != Some(id) {
                debug!(?resp_id, expected = id, "skipping non-matching MCP message");
                continue;
            }
            if let Some(err) = resp.error {
                return Err(McpError::Server(format!("{} ({})", err.message, err.code)));
            }
            return resp
                .result
                .ok_or_else(|| McpError::Protocol("response missing result".into()));
        }
    }

    async fn notify(&self, method: &str, params: Option<serde_json::Value>) -> Result<()> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(serde_json::Value::Null),
        });
        self.write_raw(&body).await
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        if let Err(err) = self.child.start_kill() {
            warn!(error = %err, "failed to kill MCP child");
        }
    }
}
