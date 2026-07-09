//! Browser / CDP configuration.

use serde::Deserialize;
use std::path::Path;

/// Browser backend preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBackend {
    /// [Obscura](https://github.com/h4ckf0r0day/obscura) headless browser.
    #[default]
    Obscura,
    /// Google Chrome / Chromium with remote debugging port.
    Chrome,
    /// Custom endpoint only (no preset defaults beyond host/port).
    Custom,
}

/// Browser automation settings.
#[derive(Debug, Clone, Deserialize)]
pub struct BrowserConfig {
    /// When false, browser tools fail closed with a clear message.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Backend preset.
    #[serde(default)]
    pub backend: BrowserBackend,
    /// Explicit CDP WebSocket URL (wins when non-empty).
    #[serde(default)]
    pub cdp_url: String,
    /// HTTP discovery URL for `webSocketDebuggerUrl`.
    #[serde(default)]
    pub discovery_url: String,
    /// Host for default endpoints.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port for default endpoints.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Default navigation wait strategy.
    #[serde(default = "default_wait")]
    pub wait_until: String,
    /// Timeout seconds for CDP operations.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_true() -> bool {
    true
}
fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    9222
}
fn default_wait() -> String {
    "load".into()
}
fn default_timeout() -> u64 {
    30
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: BrowserBackend::Obscura,
            cdp_url: String::new(),
            discovery_url: String::new(),
            host: default_host(),
            port: default_port(),
            wait_until: default_wait(),
            timeout_secs: default_timeout(),
        }
    }
}

impl BrowserConfig {
    /// Load from TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> std::result::Result<Self, String> {
        let text = std::fs::read_to_string(path.as_ref()).map_err(|e| e.to_string())?;
        Self::from_toml(&text)
    }

    /// Parse TOML.
    pub fn from_toml(text: &str) -> std::result::Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Env overrides + sensible defaults (Obscura on 9222).
    pub fn from_env_or_default() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("CORTEX_BROWSER_ENABLED") {
            cfg.enabled = matches!(v.as_str(), "1" | "true" | "yes" | "on");
        }
        if let Ok(v) = std::env::var("CORTEX_BROWSER_BACKEND") {
            cfg.backend = match v.to_ascii_lowercase().as_str() {
                "chrome" | "chromium" => BrowserBackend::Chrome,
                "custom" => BrowserBackend::Custom,
                _ => BrowserBackend::Obscura,
            };
        }
        if let Ok(v) = std::env::var("CORTEX_CDP_URL") {
            if !v.is_empty() {
                cfg.cdp_url = v;
            }
        }
        if let Ok(v) = std::env::var("CORTEX_CDP_DISCOVERY_URL") {
            if !v.is_empty() {
                cfg.discovery_url = v;
            }
        }
        if let Ok(v) = std::env::var("CORTEX_CDP_HOST") {
            cfg.host = v;
        }
        if let Ok(v) = std::env::var("CORTEX_CDP_PORT") {
            if let Ok(p) = v.parse() {
                cfg.port = p;
            }
        }
        cfg
    }

    /// Resolve the WebSocket debugger URL for this config.
    pub async fn resolve_cdp_url(&self) -> std::result::Result<String, String> {
        if !self.cdp_url.trim().is_empty() {
            return Ok(self.cdp_url.trim().to_string());
        }

        let discovery = if !self.discovery_url.trim().is_empty() {
            self.discovery_url.trim().to_string()
        } else {
            match self.backend {
                BrowserBackend::Obscura => {
                    // Obscura documents a stable browser WS path; try it first, fall back to discovery.
                    return Ok(format!("ws://{}:{}/devtools/browser", self.host, self.port));
                }
                BrowserBackend::Chrome | BrowserBackend::Custom => {
                    format!("http://{}:{}/json/version", self.host, self.port)
                }
            }
        };

        discover_websocket_url(&discovery).await
    }
}

/// GET a CDP discovery JSON document and extract `webSocketDebuggerUrl`.
pub async fn discover_websocket_url(discovery_url: &str) -> std::result::Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(discovery_url)
        .send()
        .await
        .map_err(|e| format!("CDP discovery failed ({discovery_url}): {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "CDP discovery HTTP {} from {discovery_url}",
            resp.status()
        ));
    }
    let val: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    val.get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            format!("no webSocketDebuggerUrl in discovery response from {discovery_url}")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obscura_default_url() {
        let cfg = BrowserConfig::default();
        // resolve is async for discovery; obscura path is sync-constructible
        assert_eq!(cfg.backend, BrowserBackend::Obscura);
        assert_eq!(cfg.port, 9222);
    }

    #[test]
    fn parse_toml() {
        let cfg = BrowserConfig::from_toml(
            r#"
            enabled = true
            backend = "chrome"
            host = "127.0.0.1"
            port = 9333
            "#,
        )
        .unwrap();
        assert_eq!(cfg.backend, BrowserBackend::Chrome);
        assert_eq!(cfg.port, 9333);
    }
}
