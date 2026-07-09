//! Rolling conversation summarization for long sessions.

use cortex_context::{
    extractive_summary, format_for_summary, needs_summary, split_for_summary,
    summary_system_prompt, summary_user_prompt, DEFAULT_SUMMARY_KEEP_RECENT,
    DEFAULT_SUMMARY_MESSAGE_THRESHOLD,
};
use cortex_llm::{ChatRequest, Provider};
use cortex_models::Message;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// When / how to fold long histories into a rolling summary.
#[derive(Debug, Clone)]
pub struct SummarizeConfig {
    /// Enable automatic summarization.
    pub enabled: bool,
    /// Summarize when message count reaches this (0 = ignore count).
    pub message_threshold: usize,
    /// Summarize when estimated tokens reach this (0 = ignore).
    pub token_threshold: usize,
    /// Keep this many newest messages verbatim.
    pub keep_recent: usize,
    /// Prefer LLM summarization when true; fall back to extractive on failure.
    pub use_llm: bool,
    /// Max chars for extractive fallback.
    pub extractive_max_chars: usize,
}

impl Default for SummarizeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            message_threshold: DEFAULT_SUMMARY_MESSAGE_THRESHOLD,
            token_threshold: 8_000,
            keep_recent: DEFAULT_SUMMARY_KEEP_RECENT,
            use_llm: true,
            extractive_max_chars: 2_500,
        }
    }
}

/// Result of a summarize pass.
#[derive(Debug, Clone)]
pub struct SummarizeOutcome {
    /// New rolling summary text (may replace previous).
    pub summary: String,
    /// Whether an LLM was used.
    pub used_llm: bool,
    /// How many messages were folded into the summary.
    pub folded_messages: usize,
}

/// If history exceeds thresholds, produce a summary of the older portion.
pub async fn maybe_summarize(
    provider: &Arc<dyn Provider>,
    model: &str,
    history: &[Message],
    previous_summary: Option<&str>,
    cfg: &SummarizeConfig,
) -> Option<SummarizeOutcome> {
    if !cfg.enabled {
        return None;
    }
    if !needs_summary(history, cfg.message_threshold, cfg.token_threshold) {
        return None;
    }

    let (old, _recent) = split_for_summary(history, cfg.keep_recent);
    if old.is_empty() {
        return None;
    }

    let mut transcript = String::new();
    if let Some(prev) = previous_summary {
        if !prev.trim().is_empty() {
            transcript.push_str("Previous summary:\n");
            transcript.push_str(prev.trim());
            transcript.push_str("\n\nNew messages since then:\n");
        }
    }
    transcript.push_str(&format_for_summary(&old));

    let (summary, used_llm) = if cfg.use_llm {
        match llm_summarize(provider, model, &transcript).await {
            Ok(s) if !s.trim().is_empty() => (s, true),
            Ok(_) => {
                warn!("LLM returned empty summary; using extractive");
                (extractive_summary(&old, cfg.extractive_max_chars), false)
            }
            Err(e) => {
                warn!(error = %e, "LLM summarize failed; using extractive");
                (extractive_summary(&old, cfg.extractive_max_chars), false)
            }
        }
    } else {
        (extractive_summary(&old, cfg.extractive_max_chars), false)
    };

    info!(
        folded = old.len(),
        used_llm,
        summary_chars = summary.len(),
        "conversation summarized"
    );

    Some(SummarizeOutcome {
        summary,
        used_llm,
        folded_messages: old.len(),
    })
}

async fn llm_summarize(
    provider: &Arc<dyn Provider>,
    model: &str,
    transcript: &str,
) -> Result<String, String> {
    let messages = vec![
        Message::system(summary_system_prompt()),
        Message::user(summary_user_prompt(transcript)),
    ];
    let req = ChatRequest::new(model, messages).with_max_tokens(800);
    debug!(model, "requesting conversation summary");
    let resp = provider.chat(req).await.map_err(|e| e.to_string())?;
    Ok(resp.message.content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_llm::{MockProvider, MockResponse};

    #[tokio::test]
    async fn extractive_when_below_threshold_is_none() {
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::echo("unused"));
        let hist = vec![Message::user("hi")];
        let out =
            maybe_summarize(&provider, "mock", &hist, None, &SummarizeConfig::default()).await;
        assert!(out.is_none());
    }

    #[tokio::test]
    async fn llm_summary_when_long() {
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::new(vec![MockResponse::text(
            "mock",
            "- fixed the auth bug\n- added tests",
        )]));
        let hist: Vec<_> = (0..40)
            .map(|i| Message::user(format!("message {i} about auth")))
            .collect();
        let out = maybe_summarize(
            &provider,
            "mock",
            &hist,
            None,
            &SummarizeConfig {
                message_threshold: 20,
                ..SummarizeConfig::default()
            },
        )
        .await
        .expect("summary");
        assert!(out.used_llm);
        assert!(out.summary.contains("auth") || out.summary.contains("tests"));
        assert!(out.folded_messages >= 20);
    }

    #[tokio::test]
    async fn extractive_fallback() {
        let provider: Arc<dyn Provider> = Arc::new(MockProvider::empty());
        let hist: Vec<_> = (0..40)
            .map(|i| Message::user(format!("task step {i}")))
            .collect();
        let out = maybe_summarize(
            &provider,
            "mock",
            &hist,
            None,
            &SummarizeConfig {
                message_threshold: 10,
                use_llm: true,
                ..SummarizeConfig::default()
            },
        )
        .await
        .expect("summary");
        assert!(!out.used_llm);
        assert!(out.summary.contains("task step") || out.summary.contains("user"));
    }
}
