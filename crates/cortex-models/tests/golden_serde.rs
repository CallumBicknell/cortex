//! Golden JSON fixtures for domain models (stable shape for SDKs / storage).

use cortex_common::{MessageId, SessionId, ToolCallId};
use cortex_models::{Message, Role, Session, SessionStatus, ToolCall, ToolResult, ToolSpec};
use serde_json::{json, Value};

fn strip_volatile(mut value: Value) -> Value {
    // Remove fields that are intentionally non-deterministic across runs.
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                map.remove("created_at");
                map.remove("updated_at");
                map.remove("id");
                map.remove("tool_call_id");
                map.remove("session_id");
                // Recurse into remaining fields.
                for child in map.values_mut() {
                    walk(child);
                }
            }
            Value::Array(items) => {
                for item in items {
                    walk(item);
                }
            }
            _ => {}
        }
    }
    walk(&mut value);
    value
}

#[test]
fn tool_spec_shape() {
    let spec = ToolSpec::new("read_file", "Read a file").with_parameters(json!({
        "type": "object",
        "properties": {
            "path": { "type": "string" }
        },
        "required": ["path"]
    }));
    let value = serde_json::to_value(&spec).unwrap();
    assert_eq!(
        value,
        json!({
            "name": "read_file",
            "description": "Read a file",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" }
                },
                "required": ["path"]
            }
        })
    );
}

#[test]
fn message_with_tool_calls_shape() {
    let tool_id = ToolCallId::from_uuid(
        uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
    );
    let msg_id = MessageId::from_uuid(
        uuid::Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
    );
    let mut msg = Message::assistant_with_tools(
        "reading",
        vec![ToolCall {
            id: tool_id,
            name: "read_file".into(),
            arguments: json!({"path": "Cargo.toml"}),
        }],
    );
    msg.id = msg_id;

    let value = serde_json::to_value(&msg).unwrap();
    let stable = strip_volatile(value);
    assert_eq!(stable["role"], json!("assistant"));
    assert_eq!(stable["content"], json!("reading"));
    assert_eq!(stable["tool_calls"][0]["name"], json!("read_file"));
    assert_eq!(
        stable["tool_calls"][0]["arguments"],
        json!({"path": "Cargo.toml"})
    );
}

#[test]
fn tool_result_error_flag() {
    let id = ToolCallId::new();
    let result = ToolResult::error(id, "shell", "exit 1");
    let value = serde_json::to_value(&result).unwrap();
    assert_eq!(value["is_error"], json!(true));
    assert_eq!(value["name"], json!("shell"));
    assert_eq!(value["output"], json!("exit 1"));
}

#[test]
fn session_status_and_messages() {
    let mut session = Session::new("/tmp/ws", "default");
    session.id = SessionId::from_uuid(
        uuid::Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
    );
    session.push_message(Message::user("hi"));
    assert_eq!(session.status, SessionStatus::Active);
    assert_eq!(session.messages[0].role, Role::User);

    let value = serde_json::to_value(&session).unwrap();
    let stable = strip_volatile(value);
    assert_eq!(stable["workspace"], json!("/tmp/ws"));
    assert_eq!(stable["model"], json!("default"));
    assert_eq!(stable["status"], json!("active"));
    assert_eq!(stable["messages"][0]["role"], json!("user"));
    assert_eq!(stable["messages"][0]["content"], json!("hi"));
}
