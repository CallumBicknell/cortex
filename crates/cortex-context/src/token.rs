//! Approximate token counting (chars/4 fallback; no tiktoken dependency yet).

/// Estimate token count for text.
///
/// Uses a simple heuristic of `ceil(chars / 4)` which is good enough for
/// budget enforcement without shipping tokenizer model files.
pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    chars.div_ceil(4).max(if chars == 0 { 0 } else { 1 })
}

/// Estimate tokens across many strings.
pub fn estimate_tokens_many<'a>(parts: impl IntoIterator<Item = &'a str>) -> usize {
    parts.into_iter().map(estimate_tokens).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_zero() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn scales_with_length() {
        let short = estimate_tokens("abcd"); // 4 chars -> 1
        let long = estimate_tokens(&"a".repeat(40)); // 10
        assert_eq!(short, 1);
        assert_eq!(long, 10);
        assert!(long > short);
    }
}
