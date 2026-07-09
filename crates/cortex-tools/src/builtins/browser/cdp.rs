//! Minimal async CDP session over WebSocket (Obscura / Chrome compatible).

use crate::error::{Result, ToolError};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::time::timeout;
use tokio_tungstenite::{connect_async, tungstenite::Message};

struct SharedCdp {
    timeout_secs: u64,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    write_tx: mpsc::UnboundedSender<Message>,
    session_id: Mutex<Option<String>>,
    target_id: Mutex<Option<String>>,
    _reader: tokio::task::JoinHandle<()>,
}

impl SharedCdp {
    async fn call(&self, method: &str, params: Value, session_id: Option<String>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut msg = json!({
            "id": id,
            "method": method,
            "params": params,
        });
        if let Some(sid) = session_id {
            msg.as_object_mut()
                .unwrap()
                .insert("sessionId".into(), Value::String(sid));
        }
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        let text = serde_json::to_string(&msg).map_err(|e| ToolError::Execution(e.to_string()))?;
        self.write_tx
            .send(Message::Text(text.into()))
            .map_err(|e| ToolError::Execution(format!("CDP write failed: {e}")))?;

        let resp = timeout(Duration::from_secs(self.timeout_secs.max(1)), rx)
            .await
            .map_err(|_| ToolError::Timeout(format!("CDP {method} timed out")))?
            .map_err(|_| ToolError::Execution(format!("CDP {method} channel closed")))?;

        if let Some(err) = resp.get("error") {
            return Err(ToolError::Execution(format!("CDP {method} error: {err}")));
        }
        Ok(resp)
    }

    async fn evaluate_raw(&self, expression: &str, session_id: Option<String>) -> Result<String> {
        let resp = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                session_id,
            )
            .await?;
        if let Some(ex) = resp.pointer("/result/exceptionDetails") {
            return Err(ToolError::Execution(format!("JS exception: {ex}")));
        }
        let value = resp
            .pointer("/result/result/value")
            .cloned()
            .unwrap_or(Value::Null);
        Ok(match value {
            Value::String(s) => s,
            other => other.to_string(),
        })
    }
}

/// Active CDP connection with a page session.
pub struct CdpSession {
    inner: Arc<SharedCdp>,
}

impl CdpSession {
    /// Connect to a browser-level CDP WebSocket URL and open a blank page.
    pub async fn connect(ws_url: &str, timeout_secs: u64) -> Result<Self> {
        let (ws, _) = connect_async(ws_url)
            .await
            .map_err(|e| ToolError::Execution(format!("CDP connect failed ({ws_url}): {e}")))?;

        let (write, mut read) = ws.split();
        let write = Arc::new(Mutex::new(write));
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Message>();
        {
            let write = Arc::clone(&write);
            tokio::spawn(async move {
                while let Some(msg) = write_rx.recv().await {
                    let mut g = write.lock().await;
                    if g.send(msg).await.is_err() {
                        break;
                    }
                }
            });
        }

        let shared_pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_r = Arc::clone(&shared_pending);
        let reader = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        if let Ok(val) = serde_json::from_str::<Value>(text.as_ref()) {
                            if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                                let mut map = pending_r.lock().await;
                                if let Some(tx) = map.remove(&id) {
                                    let _ = tx.send(val);
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
        });

        let inner = Arc::new(SharedCdp {
            timeout_secs: timeout_secs.max(1),
            next_id: AtomicU64::new(1),
            pending: shared_pending,
            write_tx,
            session_id: Mutex::new(None),
            target_id: Mutex::new(None),
            _reader: reader,
        });

        let create = inner
            .call("Target.createTarget", json!({ "url": "about:blank" }), None)
            .await?;
        let target_id = create
            .pointer("/result/targetId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::Execution(format!("createTarget failed: {create}")))?
            .to_string();

        let attach = inner
            .call(
                "Target.attachToTarget",
                json!({ "targetId": target_id, "flatten": true }),
                None,
            )
            .await?;
        let session_id = attach
            .pointer("/result/sessionId")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::Execution(format!("attachToTarget failed: {attach}")))?
            .to_string();

        *inner.session_id.lock().await = Some(session_id.clone());
        *inner.target_id.lock().await = Some(target_id);

        let _ = inner
            .call("Page.enable", json!({}), Some(session_id.clone()))
            .await;
        let _ = inner
            .call("Runtime.enable", json!({}), Some(session_id))
            .await;

        Ok(Self { inner })
    }

    /// Navigate and wait for readyState.
    pub async fn navigate(&mut self, url: &str, wait_until: &str) -> Result<String> {
        let sid = self.inner.session_id.lock().await.clone();
        let _ = self
            .inner
            .call("Page.navigate", json!({ "url": url }), sid.clone())
            .await?;

        let deadline = Duration::from_secs(self.inner.timeout_secs);
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > deadline {
                return Err(ToolError::Timeout(format!(
                    "navigation timeout after {}s",
                    self.inner.timeout_secs
                )));
            }
            let state = self
                .inner
                .call(
                    "Runtime.evaluate",
                    json!({
                        "expression": "document.readyState",
                        "returnByValue": true
                    }),
                    sid.clone(),
                )
                .await?;
            let ready = state
                .pointer("/result/result/value")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ok = match wait_until {
                "domcontentloaded" => ready == "interactive" || ready == "complete",
                "networkidle" | "networkidle0" => ready == "complete",
                _ => ready == "complete",
            };
            if ok {
                let title = self
                    .inner
                    .evaluate_raw("document.title", sid)
                    .await
                    .unwrap_or_default();
                return Ok(format!("navigated to {url} (title={title})"));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Evaluate JS expression.
    pub async fn evaluate(&mut self, expression: &str) -> Result<String> {
        let sid = self.inner.session_id.lock().await.clone();
        self.inner.evaluate_raw(expression, sid).await
    }

    /// Snapshot url/title/text.
    pub async fn snapshot(&mut self) -> Result<String> {
        let sid = self.inner.session_id.lock().await.clone();
        let expr = r#"(function(){
          return JSON.stringify({
            url: location.href,
            title: document.title,
            text: document.body ? document.body.innerText.slice(0, 20000) : ''
          });
        })()"#;
        self.inner.evaluate_raw(expr, sid).await
    }

    /// Close target best-effort.
    pub async fn close(self) -> Result<()> {
        let sid = self.inner.session_id.lock().await.clone();
        let tid = self.inner.target_id.lock().await.clone();
        if let Some(target_id) = tid {
            let _ = self
                .inner
                .call("Target.closeTarget", json!({ "targetId": target_id }), sid)
                .await;
        }
        Ok(())
    }
}
