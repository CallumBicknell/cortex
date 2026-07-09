//! Streamable HTTP MCP transport (2025-03-26) with legacy HTTP+SSE fallback.

use super::McpTransport;
use crate::error::{McpError, Result};
use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

const SESSION_HEADER: &str = "mcp-session-id";

/// HTTP-based MCP transport (Streamable HTTP primary).
pub struct HttpTransport {
    client: Client,
    /// Endpoint used for POST (and GET for legacy SSE endpoint discovery).
    endpoint: Mutex<String>,
    /// Original URL (for legacy GET discovery).
    base_url: String,
    session_id: Mutex<Option<String>>,
    next_id: AtomicU64,
    extra_headers: HeaderMap,
}

impl HttpTransport {
    /// Create an HTTP transport targeting `url` (Streamable HTTP MCP endpoint).
    pub fn new(url: &str, headers: &HashMap<String, String>, timeout_secs: u64) -> Result<Self> {
        let timeout = Duration::from_secs(timeout_secs.max(5));
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(format!("cortex-mcp/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| McpError::Protocol(format!("http client: {e}")))?;

        let mut extra_headers = HeaderMap::new();
        for (k, v) in headers {
            let name = HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| McpError::Config(format!("bad header name `{k}`: {e}")))?;
            let value = HeaderValue::from_str(v)
                .map_err(|e| McpError::Config(format!("bad header value for `{k}`: {e}")))?;
            extra_headers.insert(name, value);
        }

        let base = url.trim_end_matches('/').to_string();
        Ok(Self {
            client,
            endpoint: Mutex::new(base.clone()),
            base_url: base,
            session_id: Mutex::new(None),
            next_id: AtomicU64::new(1),
            extra_headers,
        })
    }

    /// Legacy HTTP+SSE: GET base URL, wait for `endpoint` event with POST URL.
    pub async fn switch_to_legacy_sse_endpoint(&self) -> Result<()> {
        let builder = self
            .client
            .get(&self.base_url)
            .header(ACCEPT, "text/event-stream")
            .headers(self.extra_headers.clone());
        let resp = builder
            .send()
            .await
            .map_err(|e| McpError::Protocol(format!("legacy SSE GET: {e}")))?;
        if !resp.status().is_success() {
            return Err(McpError::Protocol(format!(
                "legacy SSE GET failed: HTTP {}",
                resp.status()
            )));
        }
        let body = resp
            .text()
            .await
            .map_err(|e| McpError::Protocol(format!("legacy SSE body: {e}")))?;
        let endpoint = parse_sse_endpoint_event(&body).ok_or_else(|| {
            McpError::Protocol(
                "legacy SSE stream did not provide an endpoint event; \
                 use a Streamable HTTP MCP URL"
                    .into(),
            )
        })?;
        let resolved = resolve_endpoint_url(&self.base_url, &endpoint);
        info!(%resolved, "using legacy SSE message endpoint");
        *self.endpoint.lock().await = resolved;
        *self.session_id.lock().await = None;
        Ok(())
    }

    fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    async fn apply_common_headers(
        &self,
        builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        let mut b = builder
            .header(ACCEPT, "application/json, text/event-stream")
            .headers(self.extra_headers.clone());
        if let Some(sid) = self.session_id.lock().await.as_ref() {
            if let Ok(v) = HeaderValue::from_str(sid) {
                b = b.header(SESSION_HEADER, v);
            }
        }
        b
    }

    async fn capture_session(&self, resp: &reqwest::Response) {
        if let Some(v) = resp.headers().get(SESSION_HEADER) {
            if let Ok(s) = v.to_str() {
                if !s.is_empty() {
                    *self.session_id.lock().await = Some(s.to_string());
                    debug!(session_id = %s, "captured MCP session id");
                }
            }
        }
    }

    async fn post_json_rpc(&self, body: &Value) -> Result<reqwest::Response> {
        let url = self.endpoint.lock().await.clone();
        let builder = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .json(body);
        let builder = self.apply_common_headers(builder).await;
        let resp = builder
            .send()
            .await
            .map_err(|e| McpError::Protocol(format!("http post: {e}")))?;
        self.capture_session(&resp).await;
        Ok(resp)
    }

    async fn handle_rpc_response(
        &self,
        expected_id: u64,
        resp: reqwest::Response,
    ) -> Result<Value> {
        let status = resp.status();
        if status == reqwest::StatusCode::ACCEPTED {
            return Err(McpError::Protocol(
                "server returned 202 for a request (expected a result)".into(),
            ));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(McpError::Server(format!("HTTP {status}: {text}")));
        }

        let ctype = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ctype.contains("text/event-stream") {
            let text = resp
                .text()
                .await
                .map_err(|e| McpError::Protocol(format!("sse body: {e}")))?;
            return extract_result_from_sse(&text, expected_id);
        }

        let rpc: JsonRpcResponse = resp
            .json()
            .await
            .map_err(|e| McpError::Protocol(format!("json body: {e}")))?;
        if let Some(err) = rpc.error {
            return Err(McpError::Server(format!("{} ({})", err.message, err.code)));
        }
        rpc.result
            .ok_or_else(|| McpError::Protocol("response missing result".into()))
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id();
        let req = JsonRpcRequest::new(id, method, params);
        let body = serde_json::to_value(&req).map_err(|e| McpError::Protocol(e.to_string()))?;
        let resp = self.post_json_rpc(&body).await?;
        self.handle_rpc_response(id, resp).await
    }

    async fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });
        let resp = self.post_json_rpc(&body).await?;
        let status = resp.status();
        if status == reqwest::StatusCode::ACCEPTED || status.is_success() {
            return Ok(());
        }
        let text = resp.text().await.unwrap_or_default();
        Err(McpError::Server(format!(
            "notification HTTP {status}: {text}"
        )))
    }
}

/// Parse first `event: endpoint` data line from an SSE body.
fn parse_sse_endpoint_event(body: &str) -> Option<String> {
    let mut event_name = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event_name = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            let data = rest.trim().to_string();
            if event_name == "endpoint" {
                return Some(data);
            }
            // Some servers send only data with the path as first event.
            if event_name.is_empty() && (data.starts_with('/') || data.starts_with("http")) {
                return Some(data);
            }
        } else if line.is_empty() {
            event_name.clear();
        }
    }
    None
}

fn resolve_endpoint_url(base: &str, endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.to_string();
    }
    if let Ok(base_url) = url::Url::parse(base) {
        if let Ok(joined) = base_url.join(endpoint) {
            return joined.to_string();
        }
    }
    endpoint.to_string()
}

/// Walk SSE events and return the JSON-RPC result for `expected_id`.
fn extract_result_from_sse(body: &str, expected_id: u64) -> Result<Value> {
    let mut data_buf = String::new();
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            let chunk = rest.trim_start();
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(chunk);
        } else if line.is_empty() {
            if data_buf.is_empty() {
                continue;
            }
            let payload = std::mem::take(&mut data_buf);
            if let Ok(rpc) = serde_json::from_str::<JsonRpcResponse>(&payload) {
                let resp_id = rpc.id.as_ref().and_then(|v| v.as_u64());
                if resp_id.is_some() && resp_id != Some(expected_id) {
                    debug!(?resp_id, expected_id, "skip SSE event for other request");
                    continue;
                }
                if let Some(err) = rpc.error {
                    return Err(McpError::Server(format!("{} ({})", err.message, err.code)));
                }
                if let Some(result) = rpc.result {
                    return Ok(result);
                }
            } else {
                warn!(%payload, "non-JSON-RPC SSE data event");
            }
        }
    }
    if !data_buf.is_empty() {
        if let Ok(rpc) = serde_json::from_str::<JsonRpcResponse>(&data_buf) {
            if let Some(err) = rpc.error {
                return Err(McpError::Server(format!("{} ({})", err.message, err.code)));
            }
            if let Some(result) = rpc.result {
                return Ok(result);
            }
        }
    }
    Err(McpError::Protocol(format!(
        "SSE stream ended without result for id {expected_id}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_endpoint_event() {
        let body = "event: endpoint\ndata: /messages?session=abc\n\n";
        assert_eq!(
            parse_sse_endpoint_event(body).as_deref(),
            Some("/messages?session=abc")
        );
    }

    #[test]
    fn resolve_relative() {
        let u = resolve_endpoint_url("https://example.com/sse", "/messages");
        assert_eq!(u, "https://example.com/messages");
    }

    #[test]
    fn extract_json_result_from_sse() {
        let body = concat!(
            "event: message\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n",
            "\n"
        );
        let v = extract_result_from_sse(body, 1).unwrap();
        assert_eq!(v["ok"], true);
    }
}
