// Central LSP operation handler - ONE place for all LSP concerns
// This eliminates scattered retry/logging/throttling across the codebase

use crate::lsp_client::LspClient;
use async_throttle::MultiRateLimiter;
use rmcp::ErrorData as McpError;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::{info, warn};

/// Central handler for ALL LSP operations with unified error handling,
/// retry logic, throttling, and logging
pub struct LspCallHandler {
    client: Arc<Mutex<LspClient>>,
    throttle: &'static MultiRateLimiter<&'static str>,
    test_start_time: Instant,
}

impl LspCallHandler {
    pub fn new(client: Arc<Mutex<LspClient>>, test_start_time: Instant) -> Self {
        Self {
            client,
            throttle: get_lsp_throttle(),
            test_start_time,
        }
    }

    /// Execute ANY LSP operation with unified error handling, retry, throttling, logging
    /// This is the SINGLE point where all LSP concerns are handled
    pub async fn execute_lsp_operation<T, F, Fut>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> Result<T, McpError>
    where
        F: Fn(Arc<Mutex<LspClient>>) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let start_time = Instant::now();

        // 1. THROTTLING (if enabled)
        if is_throttle_enabled() {
            self.throttle
                .throttle("rust-analyzer", || async {
                    self.execute_with_retry(operation_name, &operation, start_time)
                        .await
                })
                .await
        } else {
            self.execute_with_retry(operation_name, &operation, start_time)
                .await
        }
    }

    /// Internal retry logic - part of the centralized handler
    async fn execute_with_retry<T, F, Fut>(
        &self,
        operation_name: &str,
        operation: &F,
        start_time: Instant,
    ) -> Result<T, McpError>
    where
        F: Fn(Arc<Mutex<LspClient>>) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_millis(300);
        const SEMANTIC_RETRY_DELAY: Duration = Duration::from_millis(1500);

        for attempt in 1..=MAX_RETRIES {
            match operation(Arc::clone(&self.client)).await {
                Ok(result) => {
                    // SUCCESS: Log and return
                    self.log_timing(operation_name, start_time, "ok");
                    return Ok(result);
                }
                Err(error) => {
                    // Check for readiness issues
                    if Self::is_readiness_issue(&error) && attempt < MAX_RETRIES {
                        let delay = if Self::is_semantic_operation(operation_name) {
                            warn!(
                                "Operation '{}' may need semantic indexing (attempt {}). Waiting {:?} for rust-analyzer readiness...",
                                operation_name, attempt, SEMANTIC_RETRY_DELAY
                            );
                            SEMANTIC_RETRY_DELAY
                        } else {
                            warn!(
                                "Operation '{}' got readiness issue (attempt {}). Retrying in {:?}...",
                                operation_name, attempt, RETRY_DELAY
                            );
                            RETRY_DELAY
                        };
                        sleep(delay).await;
                        continue;
                    }

                    // Check for initialization errors
                    if error.contains("not initialized") && attempt < MAX_RETRIES {
                        warn!(
                            "Operation '{}' failed due to initialization (attempt {}). Retrying in {:?}...",
                            operation_name, attempt, RETRY_DELAY
                        );
                        sleep(RETRY_DELAY).await;
                        continue;
                    }

                    // FINAL ERROR: Log and return appropriate error type
                    let error_type = if error.contains("content modified") {
                        "content_modified_error"
                    } else if Self::is_readiness_issue(&error) {
                        "readiness_issue"
                    } else {
                        "error"
                    };

                    self.log_timing(operation_name, start_time, error_type);

                    // Convert readiness issues to successful responses with helpful message
                    if Self::is_readiness_issue(&error) {
                        // This becomes a successful MCP response, not an error
                        return Err(McpError::internal_error(
                            format!("readiness_response:{}", error),
                            None,
                        ));
                    }

                    return Err(McpError::internal_error(error, None));
                }
            }
        }

        unreachable!("Loop should have returned")
    }

    /// Log timing information - centralized logging
    fn log_timing(&self, operation_name: &str, start_time: Instant, result: &str) {
        let elapsed = start_time.elapsed();
        let since_test_start = self.test_start_time.elapsed();
        info!(
            "🕐 TIMING: +{:?} {}ms {}: {}",
            since_test_start,
            elapsed.as_millis(),
            operation_name,
            result
        );
    }

    /// Check if this is a semantic operation that depends on indexing
    fn is_semantic_operation(operation_name: &str) -> bool {
        matches!(
            operation_name,
            "hover"
                | "completion"
                | "goto_definition"
                | "find_references"
                | "rename"
                | "code_actions"
                | "signature_help"
        )
    }

    /// Check if the error suggests rust-analyzer isn't ready yet
    fn is_readiness_issue(error: &str) -> bool {
        error.contains("No hover information available")
            || error.contains("No completions available")
            || error.contains("not available")
            || error.contains("analysis not ready")
            || error.contains("indexing")
            || error.contains("not ready")
            || error.contains("still loading")
            || error.contains("No ") // Generic "No X available" pattern
    }
}

// Throttling utilities - centralized
static LSP_THROTTLE: std::sync::OnceLock<MultiRateLimiter<&'static str>> =
    std::sync::OnceLock::new();

pub fn get_lsp_throttle() -> &'static MultiRateLimiter<&'static str> {
    LSP_THROTTLE.get_or_init(|| {
        let delay_ms = get_throttle_delay_ms();
        MultiRateLimiter::new(Duration::from_millis(delay_ms))
    })
}

pub fn is_throttle_enabled() -> bool {
    std::env::var("RUST_ANALYZER_MCP_THROTTLE").is_ok()
}

pub fn get_throttle_delay_ms() -> u64 {
    std::env::var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100)
}
