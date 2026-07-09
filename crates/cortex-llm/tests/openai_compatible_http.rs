//! Integration tests against a mock HTTP server (OpenAI-compatible).

use cortex_llm::{
    ChatRequest, FinishReason, OpenAiCompatibleConfig, OpenAiCompatibleProvider, Provider,
};
use cortex_models::{Message, ToolSpec};
use serde_json::json;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn chat_parses_tool_calls() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "chatcmpl-1",
            "object": "chat.completion",
            "model": "gpt-test",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": {
                            "name": "read_file",
                            "arguments": "{\"path\":\"README.md\"}"
                        }
                    }]
                }
            }],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 8,
                "total_tokens": 20
            }
        })))
        .mount(&server)
        .await;

    let provider = OpenAiCompatibleProvider::new(OpenAiCompatibleConfig {
        id: "test".into(),
        base_url: format!("{}/v1", server.uri()),
        api_key: Some("sk-test".into()),
        timeout: Duration::from_secs(5),
        retry: Default::default(),
        extra_headers: Vec::new(),
    })
    .unwrap();

    let req = ChatRequest::new("gpt-test", vec![Message::user("read readme")])
        .with_tools(vec![ToolSpec::new("read_file", "Read a file")]);
    let resp = provider.chat(req).await.unwrap();

    assert_eq!(resp.model, "gpt-test");
    assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
    assert!(resp.has_tool_calls());
    assert_eq!(resp.tool_calls()[0].name, "read_file");
    assert_eq!(resp.tool_calls()[0].arguments["path"], "README.md");
    assert_eq!(resp.usage.total_tokens, 20);
}

#[tokio::test]
async fn chat_text_response() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "gpt-test",
            "choices": [{
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "hello world"
                }
            }],
            "usage": { "prompt_tokens": 1, "completion_tokens": 2, "total_tokens": 3 }
        })))
        .mount(&server)
        .await;

    let provider = OpenAiCompatibleProvider::new(OpenAiCompatibleConfig {
        id: "test".into(),
        base_url: format!("{}/v1", server.uri()),
        api_key: None,
        timeout: Duration::from_secs(5),
        retry: Default::default(),
        extra_headers: Vec::new(),
    })
    .unwrap();

    let resp = provider
        .chat(ChatRequest::new("gpt-test", vec![Message::user("hi")]))
        .await
        .unwrap();
    assert_eq!(resp.message.content, "hello world");
    assert_eq!(resp.finish_reason, FinishReason::Stop);
}

#[tokio::test]
async fn auth_error_maps_correctly() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
        .mount(&server)
        .await;

    let provider = OpenAiCompatibleProvider::new(OpenAiCompatibleConfig {
        id: "test".into(),
        base_url: format!("{}/v1", server.uri()),
        api_key: Some("bad".into()),
        timeout: Duration::from_secs(5),
        retry: Default::default(),
        extra_headers: Vec::new(),
    })
    .unwrap();

    let err = provider
        .chat(ChatRequest::new("m", vec![Message::user("x")]))
        .await
        .unwrap_err();
    assert!(matches!(err, cortex_llm::ProviderError::Auth(_)));
}
