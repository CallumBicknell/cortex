//! Conversation summarization helpers (provider-agnostic).

use crate::token::estimate_tokens;
use cortex_models::{Message, Role};

/// Default threshold: start summarizing once history grows past this many messages.
pub const DEFAULT_SUMMARY_MESSAGE_THRESHOLD: usize = 32;
/// Messages always kept verbatim after a summary.
pub const DEFAULT_SUMMARY_KEEP_RECENT: usize = 16;

/// Whether history is large enough to warrant a rolling summary.
pub fn needs_summary(
    history: &[Message],
    message_threshold: usize,
    token_threshold: usize,
) -> bool {
    if history.is_empty() {
        return false;
    }
    if message_threshold > 0 && history.len() >= message_threshold {
        return true;
    }
    if token_threshold > 0 {
        let tokens: usize = history.iter().map(|m| estimate_tokens(&m.content)).sum();
        return tokens >= token_threshold;
    }
    false
}

/// Format messages into plain text for an LLM (or extractive) summarizer.
pub fn format_for_summary(messages: &[Message]) -> String {
    let mut out = String::new();
    for (i, m) in messages.iter().enumerate() {
        let role = match m.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };
        let mut line = m.content.clone();
        if line.len() > 1200 {
            line.truncate(1200);
            line.push('…');
        }
        if !m.tool_calls.is_empty() {
            let names: Vec<_> = m.tool_calls.iter().map(|t| t.name.as_str()).collect();
            line.push_str(&format!(" [tools: {}]", names.join(", ")));
        }
        out.push_str(&format!("[{i}] {role}: {line}\n"));
    }
    out
}

/// Prompt used when asking a model to summarize conversation history.
pub fn summary_system_prompt() -> &'static str {
    "You summarize agent conversation history for continuity. \
     Produce a concise bullet-point summary covering: goals, decisions, \
     files changed, tool outcomes, open questions, and constraints. \
     No preamble. Max ~400 words."
}

/// Build the user message for an LLM summarizer.
pub fn summary_user_prompt(transcript: &str) -> String {
    format!("Summarize this conversation transcript:\n\n{transcript}")
}

/// Extractive fallback when no LLM is available: keep first user goals + last N bullets.
pub fn extractive_summary(messages: &[Message], max_chars: usize) -> String {
    let mut bullets: Vec<String> = Vec::new();
    for m in messages {
        match m.role {
            Role::User => {
                let c = m.content.trim();
                if !c.is_empty() {
                    bullets.push(format!("- user: {}", truncate(c, 200)));
                }
            }
            Role::Assistant if !m.content.trim().is_empty() && m.tool_calls.is_empty() => {
                bullets.push(format!("- assistant: {}", truncate(m.content.trim(), 160)));
            }
            Role::Tool => {
                let status = if m.content.len() > 80 {
                    format!("{}…", &m.content[..80])
                } else {
                    m.content.clone()
                };
                bullets.push(format!(
                    "- tool({}): {}",
                    m.name.as_deref().unwrap_or("?"),
                    status
                ));
            }
            _ => {}
        }
    }
    // Prefer head goals + tail recency.
    let mut selected = Vec::new();
    let head = bullets.iter().take(6).cloned();
    let tail = bullets.iter().rev().take(10).cloned().collect::<Vec<_>>();
    selected.extend(head);
    for t in tail.into_iter().rev() {
        if !selected.contains(&t) {
            selected.push(t);
        }
    }
    let mut text = String::from("## Conversation summary (extractive)\n");
    for b in selected {
        if text.len() + b.len() + 1 > max_chars {
            break;
        }
        text.push_str(&b);
        text.push('\n');
    }
    if text.len() < 40 {
        text.push_str("- (no substantial prior content)\n");
    }
    text
}

/// Split history into (to_summarize, keep_recent).
pub fn split_for_summary(history: &[Message], keep_recent: usize) -> (Vec<Message>, Vec<Message>) {
    let keep = keep_recent.max(1);
    if history.len() <= keep {
        return (Vec::new(), history.to_vec());
    }
    let split = history.len() - keep;
    (history[..split].to_vec(), history[split..].to_vec())
}

/// Apply a rolling summary: drop summarized prefix, keep recent + summary system note.
pub fn apply_rolling_summary(
    history: &[Message],
    summary: &str,
    keep_recent: usize,
) -> Vec<Message> {
    let (_old, recent) = split_for_summary(history, keep_recent);
    let mut out = Vec::new();
    if !summary.trim().is_empty() {
        out.push(Message::system(format!(
            "[session summary]\n{}",
            summary.trim()
        )));
    }
    out.extend(recent);
    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_summary_by_count() {
        let hist: Vec<_> = (0..40).map(|i| Message::user(format!("m{i}"))).collect();
        assert!(needs_summary(&hist, 32, 0));
        assert!(!needs_summary(&hist[..10], 32, 0));
    }

    #[test]
    fn extractive_includes_user() {
        let msgs = vec![
            Message::user("fix the bug in auth"),
            Message::assistant("I'll look at auth.rs"),
        ];
        let s = extractive_summary(&msgs, 2000);
        assert!(s.contains("auth"));
    }

    #[test]
    fn apply_keeps_recent() {
        let mut hist = Vec::new();
        for i in 0..20 {
            hist.push(Message::user(format!("u{i}")));
        }
        let out = apply_rolling_summary(&hist, "prior work done", 4);
        assert!(out[0].content.contains("session summary"));
        assert_eq!(out.len(), 5); // 1 summary + 4 recent
        assert!(out.last().unwrap().content == "u19");
    }
}
