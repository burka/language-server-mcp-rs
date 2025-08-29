#![deny(dead_code)]

// Library exports for testing
pub mod errors;
pub mod lsp_client;
pub mod models;

// Re-export main types for testing
pub use models::*;
pub use lsp_client::LspClient;

// Note: RustAnalyzerMCP cannot be easily exported due to the #[tool_router] macro
// and its tight coupling with the binary. For testing, we'll test through the 
// actual binary or create a test-specific setup.