//! HTTP request tool with basic SSRF protections.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Perform an HTTP request (GET/POST/...).
pub struct HttpRequestTool {
    client: Client,
}

impl Default for HttpRequestTool {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpRequestTool {
    /// Create with a default client.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()
            .expect("http client");
        Self { client }
    }
}

#[derive(Deserialize)]
struct HttpInput {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    headers: Option<Value>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_secs: Option<u64>,
}

fn default_method() -> String {
    "GET".into()
}

#[async_trait]
impl Tool for HttpRequestTool {
    fn name(&self) -> &str {
        "http_request"
    }

    fn description(&self) -> &str {
        "Perform an HTTP request. Blocks private/link-local hosts by default."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string" },
                "method": { "type": "string", "default": "GET" },
                "headers": { "type": "object" },
                "body": { "type": "string" },
                "timeout_secs": { "type": "integer" }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: HttpInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid http_request args: {e}")))?;

        let url = reqwest::Url::parse(&args.url)
            .map_err(|e| ToolError::InvalidInput(format!("invalid url: {e}")))?;
        if url.scheme() != "http" && url.scheme() != "https" {
            return Err(ToolError::InvalidInput(
                "only http and https schemes are allowed".into(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| ToolError::InvalidInput("url missing host".into()))?;
        if !ctx.permissions.http_host_allowed(host) {
            return Err(ToolError::PermissionDenied(format!(
                "HTTP host `{host}` is blocked by policy"
            )));
        }

        let method = reqwest::Method::from_bytes(args.method.to_ascii_uppercase().as_bytes())
            .map_err(|_| ToolError::InvalidInput(format!("invalid method {}", args.method)))?;

        let timeout = Duration::from_secs(args.timeout_secs.unwrap_or(30).max(1));
        let mut builder = self.client.request(method, url).timeout(timeout);

        if let Some(headers) = args.headers {
            if let Some(obj) = headers.as_object() {
                for (k, v) in obj {
                    if let Some(val) = v.as_str() {
                        builder = builder.header(k, val);
                    }
                }
            }
        }
        if let Some(body) = args.body {
            builder = builder.body(body);
        }

        let send = builder.send();
        let response = tokio::select! {
            _ = ctx.cancel.cancelled() => {
                return Err(ToolError::Cancelled("http_request cancelled".into()));
            }
            res = send => res.map_err(|e| ToolError::Execution(format!("http error: {e}")))?,
        };

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| ToolError::Execution(format!("read body: {e}")))?;
        let mut out = format!("HTTP {status}\n{body}");
        out = ctx.truncate_output(out);
        if !status.is_success() {
            return Err(ToolError::Execution(out));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::PermissionPolicy;
    use crate::tool::{AlwaysAllow, ToolContext};
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn blocks_localhost() {
        let dir = tempdir().unwrap();
        let ctx = ToolContext::for_tests(dir.path());
        let tool = HttpRequestTool::new();
        let err = tool
            .execute(&ctx, json!({"url": "http://127.0.0.1:9/"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn get_success() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ping"))
            .respond_with(ResponseTemplate::new(200).set_body_string("pong"))
            .mount(&server)
            .await;

        // Allow the wiremock host (127.0.0.1 is blocked by default) by relaxing policy for test.
        let dir = tempdir().unwrap();
        let mut policy = PermissionPolicy::default().allow_all();
        policy.http_block_hosts.clear();
        let ctx = ToolContext {
            workspace_root: dir.path().to_path_buf(),
            session_id: None,
            cancel: tokio_util::sync::CancellationToken::new(),
            permissions: Arc::new(policy),
            approver: Arc::new(AlwaysAllow),
            default_timeout: Duration::from_secs(5),
        };
        let tool = HttpRequestTool::new();
        let url = format!("{}/ping", server.uri());
        let out = tool.execute(&ctx, json!({"url": url})).await.unwrap();
        assert!(out.contains("pong"));
    }
}
