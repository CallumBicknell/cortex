//! Pluggable web search tool.

use crate::error::{Result, ToolError};
use crate::tool::{Tool, ToolContext};
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;

/// Search backend identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchProviderKind {
    /// Tavily API (`TAVILY_API_KEY`).
    Tavily,
    /// Brave Search API (`BRAVE_API_KEY`).
    Brave,
}

/// Web search via configured provider (env-based credentials).
pub struct WebSearchTool {
    client: Client,
    kind: SearchProviderKind,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new(SearchProviderKind::Tavily)
    }
}

impl WebSearchTool {
    /// Create with a preferred provider kind.
    pub fn new(kind: SearchProviderKind) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("http client");
        Self { client, kind }
    }

    /// Pick provider from env (`CORTEX_SEARCH_PROVIDER` = tavily|brave).
    pub fn from_env() -> Self {
        let kind = match std::env::var("CORTEX_SEARCH_PROVIDER")
            .unwrap_or_else(|_| "tavily".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "brave" => SearchProviderKind::Brave,
            _ => SearchProviderKind::Tavily,
        };
        Self::new(kind)
    }
}

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default = "default_limit")]
    limit: u32,
}

fn default_limit() -> u32 {
    5
}

#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web (Tavily or Brave). Requires TAVILY_API_KEY or BRAVE_API_KEY."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "limit": { "type": "integer", "default": 5, "minimum": 1, "maximum": 10 }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, ctx: &ToolContext, input: Value) -> Result<String> {
        ctx.check_cancelled()?;
        let args: SearchInput = serde_json::from_value(input)
            .map_err(|e| ToolError::InvalidInput(format!("invalid web_search args: {e}")))?;
        if args.query.trim().is_empty() {
            return Err(ToolError::InvalidInput("query must not be empty".into()));
        }
        let limit = args.limit.clamp(1, 10);

        let text = match self.kind {
            SearchProviderKind::Tavily => self.search_tavily(&args.query, limit).await?,
            SearchProviderKind::Brave => self.search_brave(&args.query, limit).await?,
        };
        Ok(ctx.truncate_output(text))
    }
}

impl WebSearchTool {
    async fn search_tavily(&self, query: &str, limit: u32) -> Result<String> {
        let key = std::env::var("TAVILY_API_KEY").map_err(|_| {
            ToolError::Execution(
                "TAVILY_API_KEY not set (or set CORTEX_SEARCH_PROVIDER=brave)".into(),
            )
        })?;
        let body = json!({
            "api_key": key,
            "query": query,
            "max_results": limit,
            "include_answer": true,
        });
        let resp = self
            .client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        let status = resp.status();
        let val: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        if !status.is_success() {
            return Err(ToolError::Execution(format!("tavily HTTP {status}: {val}")));
        }
        format_search_results(&val, "results", "title", "url", "content")
    }

    async fn search_brave(&self, query: &str, limit: u32) -> Result<String> {
        let key = std::env::var("BRAVE_API_KEY")
            .map_err(|_| ToolError::Execution("BRAVE_API_KEY not set".into()))?;
        let resp = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query), ("count", &limit.to_string())])
            .header("Accept", "application/json")
            .header("X-Subscription-Token", key)
            .send()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        let status = resp.status();
        let val: Value = resp
            .json()
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;
        if !status.is_success() {
            return Err(ToolError::Execution(format!("brave HTTP {status}: {val}")));
        }
        // Brave nests under web.results
        let results = val
            .pointer("/web/results")
            .cloned()
            .unwrap_or(Value::Array(vec![]));
        let wrapped = json!({ "results": results });
        format_search_results(&wrapped, "results", "title", "url", "description")
    }
}

fn format_search_results(
    val: &Value,
    array_key: &str,
    title_key: &str,
    url_key: &str,
    snippet_key: &str,
) -> Result<String> {
    let mut out = String::new();
    if let Some(answer) = val.get("answer").and_then(|a| a.as_str()) {
        out.push_str("Answer: ");
        out.push_str(answer);
        out.push_str("\n\n");
    }
    let items = val
        .get(array_key)
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        out.push_str("(no results)");
        return Ok(out);
    }
    for (i, item) in items.iter().enumerate() {
        let title = item
            .get(title_key)
            .and_then(|v| v.as_str())
            .unwrap_or("(no title)");
        let url = item.get(url_key).and_then(|v| v.as_str()).unwrap_or("");
        let snippet = item.get(snippet_key).and_then(|v| v.as_str()).unwrap_or("");
        out.push_str(&format!("{}. {title}\n   {url}\n   {snippet}\n", i + 1));
    }
    Ok(out)
}
