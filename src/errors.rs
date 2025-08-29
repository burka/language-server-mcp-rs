#![deny(dead_code)]

use std::path::PathBuf;

// Custom error type for better user experience
#[derive(Debug)]
pub enum LspError {
    RustAnalyzerNotFound,
    WorkspaceNotFound(PathBuf),
    InitializationFailed(String),
    CommunicationError(String),
    TimeoutError(String),
    ProcessTerminated,
    InitializationInProgress(String), // New: for rust-analyzer still initializing
    Other(String),
}

impl std::fmt::Display for LspError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LspError::RustAnalyzerNotFound => write!(
                f,
                "rust-analyzer not found. Please install rust-analyzer:\n\
                • Run: rustup component add rust-analyzer\n\
                • Or install via your package manager\n\
                • Ensure rust-analyzer is in your PATH"
            ),
            LspError::WorkspaceNotFound(path) => write!(
                f,
                "Rust workspace not found at: {}\n\
                Please navigate to a directory containing Cargo.toml or a Rust workspace",
                path.display()
            ),
            LspError::InitializationFailed(msg) => write!(
                f,
                "Failed to initialize rust-analyzer connection.\n\
                This might be due to workspace issues or rust-analyzer version incompatibility.\n\
                Details: {}",
                msg
            ),
            LspError::CommunicationError(msg) => write!(
                f,
                "Communication error with rust-analyzer.\n\
                The language server may have crashed or become unresponsive.\n\
                Details: {}",
                msg
            ),
            LspError::TimeoutError(operation) => write!(
                f,
                "Operation '{}' timed out.\n\
                rust-analyzer may be analyzing a large codebase or having performance issues.\n\
                Try again or check your workspace size.",
                operation
            ),
            LspError::ProcessTerminated => write!(
                f,
                "rust-analyzer process terminated unexpectedly.\n\
                This may be due to memory issues or internal errors in rust-analyzer."
            ),
            LspError::InitializationInProgress(operation) => write!(
                f,
                "rust-analyzer is still initializing, operation '{}' was cancelled.\n\
                This typically takes 0.5-2 seconds. The request will be automatically retried.",
                operation
            ),
            LspError::Other(msg) => write!(f, "rust-analyzer error: {}", msg),
        }
    }
}

impl std::error::Error for LspError {}

impl LspError {
    /// Check if this error indicates rust-analyzer is still initializing
    pub fn is_initialization_error(&self) -> bool {
        match self {
            LspError::InitializationInProgress(_) => true,
            LspError::CommunicationError(msg) | LspError::Other(msg) => {
                // Detect specific initialization error patterns from our tests
                msg.contains("server cancelled the request")
                    || msg.contains("-32802")
                    || msg.contains("retriggerRequest")
                    || (msg.contains("-32603") && msg.contains("LSP error"))
            }
            _ => false,
        }
    }

    /// Convert LSP JSON-RPC errors to initialization errors when appropriate
    pub fn from_lsp_error(msg: String, method: &str) -> Self {
        if msg.contains("server cancelled the request")
            || msg.contains("-32802")
            || msg.contains("retriggerRequest")
        {
            LspError::InitializationInProgress(method.to_string())
        } else if msg.contains("-32603") {
            // Generic LSP error - could be initialization related
            LspError::CommunicationError(format!("LSP error: {}", msg))
        } else {
            LspError::Other(msg)
        }
    }
}

impl From<std::io::Error> for LspError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => LspError::RustAnalyzerNotFound,
            std::io::ErrorKind::PermissionDenied => LspError::Other(
                "Permission denied. Check that rust-analyzer is executable and you have proper permissions.".to_string()
            ),
            _ => LspError::Other(format!("System error: {}", error)),
        }
    }
}

impl From<serde_json::Error> for LspError {
    fn from(error: serde_json::Error) -> Self {
        LspError::CommunicationError(format!("JSON parsing error: {}", error))
    }
}

impl From<std::num::ParseIntError> for LspError {
    fn from(error: std::num::ParseIntError) -> Self {
        LspError::CommunicationError(format!("Failed to parse integer: {}", error))
    }
}
