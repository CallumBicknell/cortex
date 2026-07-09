//! History compression for long sessions.

use crate::token::estimate_tokens;
use cortex_models::{Message, Role};

/// Compress conversation history to fit a token budget.
///
/// Strategy:
/// 1. Always keep the most recent `keep_recent` messages.
/// 2. Drop middle tool-result noise first (role=tool).
/// 3. Drop older assistant/user messages until under budget.
/// 4. Optionally insert a placeholder summary note for dropped content.
pub fn compress_history(
    history: &[Message],
    keep_recent: usize,
    max_tokens: usize,
) -> Vec<Message> {
    if history.is_empty() {
        return Vec::new();
    }
    if max_tokens == 0 {
        return history.to_vec();
    }

    let keep_recent = keep_recent.max(1);
    if history.len() <= keep_recent && total_tokens(history) <= max_tokens {
        return history.to_vec();
    }

    let split = history.len().saturating_sub(keep_recent);
    let head = &history[..split];
    let tail = &history[split..];

    // Prefer dropping tool messages from the head first.
    let mut kept_head: Vec<Message> = head
        .iter()
        .filter(|m| m.role != Role::Tool)
        .cloned()
        .collect();

    // If still too large with tail, drop from the front of kept_head.
    let mut out = Vec::new();
    let mut dropped = head.len().saturating_sub(kept_head.len());

    loop {
        out.clear();
        out.extend(kept_head.iter().cloned());
        out.extend(tail.iter().cloned());
        if total_tokens(&out) <= max_tokens || kept_head.is_empty() {
            break;
        }
        kept_head.remove(0);
        dropped += 1;
    }

    // If tail alone exceeds budget, truncate oldest within tail (keep last).
    if total_tokens(&out) > max_tokens {
        let mut tail_only: Vec<Message> = tail.to_vec();
        while tail_only.len() > 1 && total_tokens(&tail_only) > max_tokens {
            tail_only.remove(0);
            dropped += 1;
        }
        out = tail_only;
    }

    if dropped > 0 {
        let note = Message::system(format!(
            "[context] {dropped} earlier messages were omitted to fit the token budget."
        ));
        let mut with_note = vec![note];
        with_note.extend(out.iter().cloned());
        // Re-check budget for note overhead; drop note if needed.
        if total_tokens(&with_note) <= max_tokens + 32 {
            return with_note;
        }
    }

    out
}

fn total_tokens(msgs: &[Message]) -> usize {
    msgs.iter()
        .map(|m| {
            estimate_tokens(&m.content)
                + m.tool_calls
                    .iter()
                    .map(|t| estimate_tokens(&t.name) + estimate_tokens(&t.arguments.to_string()))
                    .sum::<usize>()
                + 4
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_models::ToolCall;

    #[test]
    fn keeps_recent_and_drops_old_tools() {
        let mut history = Vec::new();
        for i in 0..5 {
            history.push(Message::user(format!("u{i}")));
            let call = ToolCall::new("shell", serde_json::json!({}));
            history.push(Message::tool_result(
                call.id,
                "shell",
                format!("tool-output-{i}-{}", "x".repeat(200)),
            ));
        }
        history.push(Message::user("latest"));
        let compressed = compress_history(&history, 3, 80);
        assert!(compressed.iter().any(|m| m.content == "latest"));
        // Should be smaller than original.
        assert!(compressed.len() < history.len());
    }

    #[test]
    fn empty_ok() {
        assert!(compress_history(&[], 4, 100).is_empty());
    }

    #[test]
    fn with_tool_calls_tokens() {
        let msg = Message::assistant_with_tools(
            "x",
            vec![ToolCall::new(
                "read_file",
                serde_json::json!({"path": "a.rs"}),
            )],
        );
        assert!(total_tokens(&[msg]) > 0);
    }
}
