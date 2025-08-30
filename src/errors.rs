#![deny(dead_code)]

use rmcp::ErrorData as McpError;
use std::path::PathBuf;
use std::time::Duration;

// Enhanced typed error system following Rust best practices
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("rust-analyzer not found. Please install rust-analyzer:\n• Run: rustup component add rust-analyzer\n• Or install via your package manager\n• Ensure rust-analyzer is in your PATH")]
    RustAnalyzerNotFound,

    #[error("Rust workspace not found at: {path}\nPlease navigate to a directory containing Cargo.toml or a Rust workspace")]
    WorkspaceNotFound { path: PathBuf },

    #[error("Failed to initialize rust-analyzer connection.\nThis might be due to workspace issues or rust-analyzer version incompatibility.\nDetails: {details}")]
    InitializationFailed { details: String },

    #[error("Communication error with rust-analyzer.\nThe language server may have crashed or become unresponsive.\nDetails: {details}")]
    CommunicationError { details: String },

    #[error("Operation '{operation}' timed out after {timeout:?}.\nrust-analyzer may be analyzing a large codebase or having performance issues.\nTry again or check your workspace size.")]
    TimeoutError {
        operation: String,
        timeout: Duration,
    },

    #[error("rust-analyzer process terminated unexpectedly.\nThis may be due to memory issues or internal errors in rust-analyzer.")]
    ProcessTerminated,

    #[error("rust-analyzer is still initializing, operation '{operation}' was cancelled.\nThis typically takes 0.5-2 seconds. The request will be automatically retried.")]
    InitializationInProgress { operation: String },

    #[error("Content was modified during analysis for file '{file_path}'.\nThis is a known rust-analyzer issue with specific files. Try using a file in the src/ directory instead.")]
    ContentModified { file_path: String },

    #[error("File not found or inaccessible: '{file_path}'. Please check the file path.")]
    FileNotFound { file_path: String },

    #[error("Invalid position in '{file_path}' at line {line}, column {column}. Please check the coordinates.")]
    InvalidPosition {
        file_path: String,
        line: u32,
        column: u32,
    },

    #[error("Response too large ({size} bytes) for operation '{operation}'. Maximum allowed: {max_size} bytes")]
    ResponseTooLarge {
        operation: String,
        size: usize,
        max_size: usize,
    },

    #[error("Rust-analyzer is not ready: {reason}. Please wait {suggested_wait:?} and try again.")]
    NotReady {
        reason: String,
        suggested_wait: Duration,
    },

    #[error("rust-analyzer error: {message}")]
    Other { message: String },
}

// Enhanced methods for the typed error system

impl LspError {
    /// Check if this error indicates rust-analyzer is still initializing
    pub fn is_initialization_error(&self) -> bool {
        matches!(
            self,
            LspError::InitializationInProgress { .. }
                | LspError::NotReady { .. }
                | LspError::InitializationFailed { .. }
        )
    }

    /// Check if this error suggests the operation should be retried
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            LspError::InitializationInProgress { .. }
                | LspError::NotReady { .. }
                | LspError::CommunicationError { .. }
                | LspError::TimeoutError { .. }
        )
    }

    /// Check if this is a semantic readiness issue (needs semantic indexing)
    pub fn is_semantic_issue(&self) -> bool {
        matches!(
            self,
            LspError::NotReady { .. } | LspError::InitializationInProgress { .. }
        )
    }

    /// Check if this error should be treated as an informational response
    pub fn is_informational(&self) -> bool {
        matches!(
            self,
            LspError::NotReady { .. }
                | LspError::FileNotFound { .. }
                | LspError::InvalidPosition { .. }
                | LspError::InitializationInProgress { .. }
        )
    }

    /// Convert to MCP error for protocol responses
    pub fn to_mcp_error(self) -> McpError {
        if self.is_informational() {
            // Return as successful response with informational message
            McpError::internal_error(format!("info:{}", self), None)
        } else {
            McpError::internal_error(self.to_string(), None)
        }
    }

    /// Smart constructor from legacy string error with intelligent categorization
    pub fn from_string_error(operation: &str, error: String, file_path: Option<&str>) -> Self {
        if error.contains("content modified") {
            LspError::ContentModified {
                file_path: file_path.unwrap_or("unknown").to_string(),
            }
        } else if error.contains("No hover information available")
            || error.contains("No completions available")
            || error.contains("not available")
            || error.contains("analysis not ready")
            || error.contains("indexing")
            || error.contains("not ready")
            || error.contains("still loading")
        {
            LspError::NotReady {
                reason: error,
                suggested_wait: Duration::from_secs(2),
            }
        } else if error.contains("server cancelled the request")
            || error.contains("-32802")
            || error.contains("retriggerRequest")
        {
            LspError::InitializationInProgress {
                operation: operation.to_string(),
            }
        } else if error.contains("timeout") || error.contains("timed out") {
            LspError::TimeoutError {
                operation: operation.to_string(),
                timeout: Duration::from_secs(30),
            }
        } else if error.contains("file not found") || error.contains("No such file") {
            LspError::FileNotFound {
                file_path: file_path.unwrap_or("unknown").to_string(),
            }
        } else if error.contains("-32603") {
            LspError::CommunicationError {
                details: format!("LSP error: {}", error),
            }
        } else {
            LspError::Other { message: error }
        }
    }

    /// Backwards compatibility: Convert LSP JSON-RPC errors to appropriate types
    pub fn from_lsp_error(msg: String, method: &str) -> Self {
        Self::from_string_error(method, msg, None)
    }
}

impl From<std::io::Error> for LspError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => LspError::RustAnalyzerNotFound,
            std::io::ErrorKind::PermissionDenied => LspError::Other {
                message: "Permission denied. Check that rust-analyzer is executable and you have proper permissions.".to_string()
            },
            _ => LspError::Other { message: format!("System error: {}", error) },
        }
    }
}

impl From<serde_json::Error> for LspError {
    fn from(error: serde_json::Error) -> Self {
        LspError::CommunicationError {
            details: format!("JSON parsing error: {}", error),
        }
    }
}

impl From<std::num::ParseIntError> for LspError {
    fn from(error: std::num::ParseIntError) -> Self {
        LspError::CommunicationError {
            details: format!("Failed to parse integer: {}", error),
        }
    }
}

impl From<String> for LspError {
    fn from(message: String) -> Self {
        LspError::Other { message }
    }
}
