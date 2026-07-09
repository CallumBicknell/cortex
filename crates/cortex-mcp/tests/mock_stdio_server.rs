//! Integration test: mock MCP stdio server + client tool call.

use cortex_mcp::{register_mcp_server_tools, McpClient, StdioTransport};
// Stdio path still uses Content-Length framing.
use cortex_tools::{ToolContext, ToolRegistry};
use serde_json::json;
use std::sync::Arc;

/// Minimal MCP server script in Python (stdlib only).
const MOCK_SERVER: &str = r#"
import sys, json

def read_msg():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("utf-8")
        if line in ("\r\n", "\n"):
            break
        if ":" in line:
            k, v = line.split(":", 1)
            headers[k.strip().lower()] = v.strip()
    n = int(headers.get("content-length", "0"))
    body = sys.stdin.buffer.read(n)
    return json.loads(body.decode("utf-8"))

def write_msg(obj):
    data = json.dumps(obj).encode("utf-8")
    sys.stdout.buffer.write(f"Content-Length: {len(data)}\r\n\r\n".encode("ascii"))
    sys.stdout.buffer.write(data)
    sys.stdout.buffer.flush()

while True:
    msg = read_msg()
    if msg is None:
        break
    method = msg.get("method")
    mid = msg.get("id")
    if method == "initialize":
        write_msg({
            "jsonrpc": "2.0",
            "id": mid,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock", "version": "0.0.1"},
            },
        })
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        write_msg({
            "jsonrpc": "2.0",
            "id": mid,
            "result": {
                "tools": [{
                    "name": "echo",
                    "description": "Echo text",
                    "inputSchema": {
                        "type": "object",
                        "properties": {"text": {"type": "string"}},
                        "required": ["text"],
                    },
                }]
            },
        })
    elif method == "tools/call":
        args = (msg.get("params") or {}).get("arguments") or {}
        text = args.get("text", "")
        write_msg({
            "jsonrpc": "2.0",
            "id": mid,
            "result": {
                "content": [{"type": "text", "text": f"echo:{text}"}],
                "isError": False,
            },
        })
    else:
        if mid is not None:
            write_msg({
                "jsonrpc": "2.0",
                "id": mid,
                "error": {"code": -32601, "message": f"unknown method {method}"},
            })
"#;

#[tokio::test]
async fn mock_server_list_and_call() {
    let transport = StdioTransport::spawn(
        "python3",
        &["-u".into(), "-c".into(), MOCK_SERVER.into()],
        None,
        &Default::default(),
    )
    .await
    .expect("spawn transport");

    let client = Arc::new(
        McpClient::from_transport("mock", transport)
            .await
            .expect("initialize"),
    );
    let tools = client.list_tools().await.expect("list");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    let result = client
        .call_tool("echo", json!({"text": "hi"}))
        .await
        .expect("call");
    assert_eq!(result.as_text(), "echo:hi");

    let mut reg = ToolRegistry::new();
    let n = register_mcp_server_tools(client, "mcp_mock_", &mut reg)
        .await
        .expect("register");
    assert_eq!(n, 1);
    assert!(reg.contains("mcp_mock_echo"));

    let tool = reg.get("mcp_mock_echo").unwrap();
    let dir = tempfile::tempdir().unwrap();
    let ctx = ToolContext::for_tests(dir.path());
    let out = tool.execute(&ctx, json!({"text": "cortex"})).await.unwrap();
    assert_eq!(out, "echo:cortex");
}
