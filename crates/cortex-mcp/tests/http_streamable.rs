//! Integration test: mock Streamable HTTP MCP server.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use cortex_mcp::{register_mcp_server_tools, HttpTransport, McpClient};
use cortex_tools::{ToolContext, ToolRegistry};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;

async fn mcp_handler(State(_): State<()>, req: Request<Body>) -> Response {
    let body = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .unwrap_or_default();
    let msg: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
    let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = msg.get("id").cloned().unwrap_or(Value::Null);

    let result = match method {
        "initialize" => json!({
            "protocolVersion": "2025-03-26",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "mock-http", "version": "0.0.1" },
        }),
        "tools/list" => json!({
            "tools": [{
                "name": "ping",
                "description": "Ping",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                }
            }]
        }),
        "tools/call" => {
            let name = msg
                .pointer("/params/name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            json!({
                "content": [{ "type": "text", "text": format!("pong:{name}") }],
                "isError": false,
            })
        }
        _ if id.is_null() => {
            return StatusCode::ACCEPTED.into_response();
        }
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": { "code": -32601, "message": "method not found" }
                })
                .to_string(),
            )
                .into_response();
        }
    };

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
    headers.insert("mcp-session-id", "test-session-1".parse().unwrap());
    (
        StatusCode::OK,
        headers,
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string(),
    )
        .into_response()
}

#[tokio::test]
async fn streamable_http_list_and_call() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/mcp", post(mcp_handler))
        .with_state(());

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    std::env::set_var("CORTEX_MCP_ALLOW_LOCAL", "1");

    let url = format!("http://{addr}/mcp");
    let transport = HttpTransport::new(&url, &Default::default(), 30).unwrap();
    let client = McpClient::from_http_transport("mock", transport, "2025-03-26")
        .await
        .expect("connect");

    let tools = client.list_tools().await.expect("list");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "ping");

    let mut reg = ToolRegistry::new();
    let n = register_mcp_server_tools(Arc::new(client), "mcp_mock_", &mut reg)
        .await
        .unwrap();
    assert_eq!(n, 1);

    let tool = reg.get("mcp_mock_ping").unwrap();
    let ctx = ToolContext::for_tests(std::env::temp_dir());
    let out = tool.execute(&ctx, json!({})).await.expect("call");
    assert!(out.contains("pong:ping"), "{out}");
}
