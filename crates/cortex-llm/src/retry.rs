//! Retry helpers for transient provider failures.

use crate::error::{ProviderError, Result};
use std::future::Future;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Retry policy for provider calls.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum attempts (including the first try).
    pub max_attempts: u32,
    /// Base delay for exponential backoff.
    pub base_delay: Duration,
    /// Maximum delay between attempts.
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RetryPolicy {
    /// Run `f` with retries on retryable errors.
    pub async fn run<T, F, Fut>(&self, cancel: Option<&CancellationToken>, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 0u32;
        loop {
            if let Some(c) = cancel {
                if c.is_cancelled() {
                    return Err(ProviderError::cancelled("retry aborted"));
                }
            }
            attempt += 1;
            match f().await {
                Ok(v) => return Ok(v),
                Err(err) if err.is_retryable() && attempt < self.max_attempts => {
                    let exp = 1u32 << (attempt.saturating_sub(1).min(8));
                    let delay = self.base_delay.saturating_mul(exp).min(self.max_delay);
                    tracing::warn!(
                        attempt,
                        max = self.max_attempts,
                        ?delay,
                        error = %err,
                        "retrying provider call"
                    );
                    if let Some(c) = cancel {
                        tokio::select! {
                            _ = c.cancelled() => {
                                return Err(ProviderError::cancelled("retry aborted during backoff"));
                            }
                            _ = tokio::time::sleep(delay) => {}
                        }
                    } else {
                        tokio::time::sleep(delay).await;
                    }
                }
                Err(err) => return Err(err),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn retries_then_succeeds() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(5),
        };
        let tries = Arc::new(AtomicU32::new(0));
        let tries2 = Arc::clone(&tries);
        let result = policy
            .run(None, || {
                let tries = Arc::clone(&tries2);
                async move {
                    let n = tries.fetch_add(1, Ordering::SeqCst) + 1;
                    if n < 3 {
                        Err(ProviderError::Transport("flaky".into()))
                    } else {
                        Ok(42)
                    }
                }
            })
            .await
            .unwrap();
        assert_eq!(result, 42);
        assert_eq!(tries.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn does_not_retry_auth() {
        let policy = RetryPolicy {
            max_attempts: 5,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(1),
        };
        let tries = Arc::new(AtomicU32::new(0));
        let tries2 = Arc::clone(&tries);
        let err = policy
            .run(None, || {
                let tries = Arc::clone(&tries2);
                async move {
                    tries.fetch_add(1, Ordering::SeqCst);
                    Err::<(), _>(ProviderError::Auth("bad key".into()))
                }
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::Auth(_)));
        assert_eq!(tries.load(Ordering::SeqCst), 1);
    }
}
