//! Auto-start a local CDP browser when nothing is listening.

use crate::error::{Result, ToolError};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::{sleep, Instant};
use tracing::{info, warn};

use super::config::{BrowserBackend, BrowserConfig};

/// State for a browser process we launched.
#[derive(Default)]
pub(crate) struct ManagedBrowser {
    child: Option<Child>,
}

impl ManagedBrowser {
    pub(crate) fn take_child(&mut self) -> Option<Child> {
        self.child.take()
    }

    pub(crate) fn set_child(&mut self, child: Child) {
        // Prefer not to kill a previous child we started — leave it for reuse.
        if let Some(mut old) = self.child.replace(child) {
            let _ = old.try_wait();
        }
    }
}

/// Whether this error looks like "nothing is listening".
pub(crate) fn is_connection_failure(err: &ToolError) -> bool {
    let s = err.to_string().to_lowercase();
    s.contains("connection refused")
        || s.contains("os error 111")
        || s.contains("actively refused")
        || s.contains("connect failed")
        || s.contains("failed to connect")
        || s.contains("connection reset")
        || s.contains("timed out") && s.contains("connect")
        || s.contains("cdp discovery failed")
        || s.contains("error connecting")
}

/// Host is loopback (safe to auto-start a local binary).
pub(crate) fn is_loopback_host(host: &str) -> bool {
    let h = host.trim().trim_matches(|c| c == '[' || c == ']');
    matches!(h, "127.0.0.1" | "localhost" | "::1" | "0.0.0.0" | "") || h.starts_with("127.")
}

impl BrowserConfig {
    /// Auto-start is allowed only for local obscura/chrome backends.
    pub fn should_auto_start(&self) -> bool {
        if !self.auto_start || !self.enabled {
            return false;
        }
        if !matches!(
            self.backend,
            BrowserBackend::Obscura | BrowserBackend::Chrome
        ) {
            return false;
        }
        if !is_loopback_host(&self.host) {
            return false;
        }
        if !self.cdp_url.trim().is_empty() {
            if let Some(host) = host_from_ws_url(&self.cdp_url) {
                if !is_loopback_host(&host) {
                    return false;
                }
            }
        }
        true
    }
}

fn host_from_ws_url(url: &str) -> Option<String> {
    // ws://host:port/... or wss://
    let rest = url
        .strip_prefix("ws://")
        .or_else(|| url.strip_prefix("wss://"))?;
    let hostport = rest.split('/').next()?;
    let host = if hostport.starts_with('[') {
        hostport
            .trim_start_matches('[')
            .split(']')
            .next()?
            .to_string()
    } else {
        hostport.split(':').next()?.to_string()
    };
    Some(host)
}

/// Resolve binary + args for the configured backend.
pub(crate) fn start_command(cfg: &BrowserConfig) -> Result<(PathBuf, Vec<String>)> {
    match cfg.backend {
        BrowserBackend::Obscura => {
            let bin = find_in_path(&["obscura"]).ok_or_else(|| {
                ToolError::Execution(
                    "auto-start failed: `obscura` not found on PATH. \
                     Install Obscura or start CDP manually (see docs/browser.md)."
                        .into(),
                )
            })?;
            let args = vec![
                "serve".into(),
                "--host".into(),
                cfg.host.clone(),
                "--port".into(),
                cfg.port.to_string(),
            ];
            Ok((bin, args))
        }
        BrowserBackend::Chrome => {
            let bin = find_in_path(&[
                "chromium",
                "chromium-browser",
                "google-chrome",
                "google-chrome-stable",
                "chrome",
            ])
            .ok_or_else(|| {
                ToolError::Execution(
                    "auto-start failed: Chrome/Chromium not found on PATH. \
                     Install Chromium or start CDP manually (see docs/browser.md)."
                        .into(),
                )
            })?;
            let args = vec![
                "--headless=new".into(),
                "--disable-gpu".into(),
                "--no-first-run".into(),
                "--no-default-browser-check".into(),
                format!("--remote-debugging-port={}", cfg.port),
                format!("--remote-debugging-address={}", cfg.host),
                "about:blank".into(),
            ];
            Ok((bin, args))
        }
        BrowserBackend::Custom => Err(ToolError::Execution(
            "auto-start is not supported for backend=custom; set cdp_url or start the browser yourself"
                .into(),
        )),
    }
}

/// Spawn the local CDP browser and wait until `host:port` accepts TCP.
pub(crate) async fn spawn_and_wait(
    cfg: &BrowserConfig,
    managed: &mut ManagedBrowser,
) -> Result<String> {
    // Already up? (user may have started it between attempts)
    if port_open(&cfg.host, cfg.port).await {
        return Ok(format!(
            "CDP already listening on {}:{}",
            cfg.host, cfg.port
        ));
    }

    let (bin, args) = start_command(cfg)?;
    info!(
        binary = %bin.display(),
        port = cfg.port,
        backend = ?cfg.backend,
        "auto-starting CDP browser"
    );

    let log_path = browser_log_path();
    let (stdout, stderr) = match open_log_file(&log_path) {
        Ok(f) => {
            let f2 = f.try_clone().ok();
            (
                Stdio::from(f),
                f2.map(Stdio::from).unwrap_or_else(Stdio::null),
            )
        }
        Err(_) => (Stdio::null(), Stdio::null()),
    };

    let mut cmd = Command::new(&bin);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr);

    // Own process group so a terminal Ctrl+C on Cortex does not always kill CDP.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let child = cmd.spawn().map_err(|e| {
        ToolError::Execution(format!(
            "auto-start failed: could not spawn {} {}: {e}",
            bin.display(),
            args.join(" ")
        ))
    })?;

    let pid = child.id();
    managed.set_child(child);

    let wait_secs = cfg.auto_start_timeout_secs.max(1);
    wait_for_port(&cfg.host, cfg.port, Duration::from_secs(wait_secs))
        .await
        .map_err(|e| {
            ToolError::Execution(format!(
                "auto-start launched {bin} (pid {pid}) but CDP did not become ready on {}:{} within {wait_secs}s: {e}. \
                 Check logs at {}.",
                cfg.host,
                cfg.port,
                log_path.display(),
                bin = bin.display()
            ))
        })?;

    // Brief grace period for CDP HTTP/WS to accept after TCP opens.
    sleep(Duration::from_millis(400)).await;

    Ok(format!(
        "auto-started {} on {}:{} (pid {pid})",
        bin.display(),
        cfg.host,
        cfg.port
    ))
}

fn browser_log_path() -> PathBuf {
    if let Ok(home) = std::env::var("CORTEX_HOME") {
        if !home.trim().is_empty() {
            return PathBuf::from(home)
                .join("logs")
                .join("browser-autostart.log");
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".cortex")
            .join("logs")
            .join("browser-autostart.log");
    }
    PathBuf::from("browser-autostart.log")
}

fn open_log_file(path: &Path) -> std::io::Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
}

async fn port_open(host: &str, port: u16) -> bool {
    TcpStream::connect((host, port)).await.is_ok()
}

async fn wait_for_port(host: &str, port: u16, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let mut last_err = String::from("not ready");
    while Instant::now() < deadline {
        match TcpStream::connect((host, port)).await {
            Ok(_) => return Ok(()),
            Err(e) => last_err = e.to_string(),
        }
        sleep(Duration::from_millis(200)).await;
    }
    Err(ToolError::Execution(format!(
        "port {host}:{port} not open ({last_err})"
    )))
}

/// Search PATH for the first existing executable name.
pub(crate) fn find_in_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if is_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        path.metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Soft-stop a managed child (best-effort). Prefer leaving browsers running for reuse.
#[allow(dead_code)]
pub(crate) fn stop_managed(managed: &mut ManagedBrowser) {
    if let Some(mut child) = managed.take_child() {
        match child.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                warn!("stopped auto-started browser process");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_hosts() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("localhost"));
        assert!(is_loopback_host("::1"));
        assert!(!is_loopback_host("10.0.0.1"));
        assert!(!is_loopback_host("example.com"));
    }

    #[test]
    fn should_auto_start_defaults() {
        let cfg = BrowserConfig::default();
        assert!(cfg.auto_start);
        assert!(cfg.should_auto_start());
    }

    #[test]
    fn no_auto_start_remote() {
        let cfg = BrowserConfig {
            host: "10.0.0.5".into(),
            ..BrowserConfig::default()
        };
        assert!(!cfg.should_auto_start());
    }

    #[test]
    fn no_auto_start_when_disabled() {
        let cfg = BrowserConfig {
            auto_start: false,
            ..BrowserConfig::default()
        };
        assert!(!cfg.should_auto_start());
    }

    #[test]
    fn obscura_command_shape() {
        if find_in_path(&["obscura"]).is_none() {
            return;
        }
        let cfg = BrowserConfig::default();
        let (bin, args) = start_command(&cfg).unwrap();
        assert!(bin.to_string_lossy().contains("obscura"));
        assert_eq!(args[0], "serve");
        assert!(args.contains(&"9222".to_string()));
    }

    #[test]
    fn host_from_ws() {
        assert_eq!(
            host_from_ws_url("ws://127.0.0.1:9222/devtools/browser").as_deref(),
            Some("127.0.0.1")
        );
    }
}
