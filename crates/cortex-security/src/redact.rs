//! Secrets hygiene: redact sensitive values in logs and tool output.

use regex::Regex;
use std::sync::OnceLock;

fn re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid secret regex")
}

fn patterns() -> &'static [Regex] {
    static RE: OnceLock<Vec<Regex>> = OnceLock::new();
    RE.get_or_init(|| {
        vec![
            re(r"(?i)(api[_-]?key|token|secret|password|passwd)\s*[:=]\s*\S+"),
            re(r"sk-[A-Za-z0-9]{20,}"),
            re(r"ghp_[A-Za-z0-9]{20,}"),
            re(r"github_pat_[A-Za-z0-9_]{20,}"),
            re(r"xox[baprs]-[A-Za-z0-9-]{10,}"),
            re(r"(?i)bearer\s+[A-Za-z0-9\-._~+/]+=*"),
            re(r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----"),
        ]
    })
}

/// Redact secrets in free-form text.
pub fn redact_text(input: &str) -> String {
    let mut out = input.to_string();
    for re in patterns() {
        out = re.replace_all(&out, "[REDACTED]").into_owned();
    }
    out
}

/// Redact a JSON value in place (string leaves + sensitive keys).
pub fn redact_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(s) => {
            *s = redact_text(s);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json(item);
            }
        }
        serde_json::Value::Object(map) => {
            let sensitive = [
                "password",
                "secret",
                "token",
                "api_key",
                "apikey",
                "authorization",
            ];
            let keys: Vec<String> = map.keys().cloned().collect();
            for k in keys {
                if sensitive.iter().any(|s| k.to_ascii_lowercase().contains(s)) {
                    map.insert(k, serde_json::Value::String("[REDACTED]".into()));
                } else if let Some(v) = map.get_mut(&k) {
                    redact_json(v);
                }
            }
        }
        _ => {}
    }
}

/// Clone JSON with secrets redacted.
pub fn redacted_json(value: &serde_json::Value) -> serde_json::Value {
    let mut v = value.clone();
    redact_json(&mut v);
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_openai_key() {
        let s = "token sk-abcdefghijklmnopqrstuvwxyz0123456789";
        let out = redact_text(s);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("sk-abcdefghijklmnopqrstuvwxyz0123456789"));
    }

    #[test]
    fn redacts_json_secret_key() {
        let mut v = json!({"api_key": "supersecretvalue", "path": "a.rs"});
        redact_json(&mut v);
        assert_eq!(v["api_key"], "[REDACTED]");
        assert_eq!(v["path"], "a.rs");
    }
}
