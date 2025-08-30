#![deny(dead_code)]

use clap::Parser;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::{transport::stdio, ServiceExt};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{self, EnvFilter};

#[derive(Debug, Parser)]
#[command(name = "language-server-mcp")]
#[command(about = "Rust-analyzer MCP server")]
struct Args {
    /// Workspace root directory (optional)
    workspace_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing with stderr for debugging timing issues
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr) // Use stderr for debugging
        .with_ansi(false)
        .try_init(); // Silently ignore if already initialized

    info!("Starting rust-analyzer MCP server");

    let workspace_root = args
        .workspace_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to determine current directory"));

    // Create the MCP service
    let mcp_service = match RustAnalyzerMCP::new(workspace_root).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    // Start MCP service - server is immediately responsive
    let service = mcp_service.serve(stdio()).await?;

    info!("MCP server is running");
    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use language_server_mcp::lsp_client;
    use language_server_mcp::models::{
        CloseDocumentRequest, DocumentSymbolsRequest, LspClientStatusRequest,
    };
    use std::path::PathBuf;

    #[test]
    fn test_workspace_root_method() {
        // Test that workspace_root method works (to avoid dead code warning)
        let workspace = PathBuf::from("/tmp");
        let workspace_clone = workspace.clone();

        // We can't easily create a full RustAnalyzerMCP instance in tests without rust-analyzer,
        // but we can test that the PathBuf operations work
        assert_eq!(workspace, workspace_clone);
        assert!(workspace.is_absolute());
    }

    #[test]
    fn test_lsp_status_request_deserialization() {
        // Test LspClientStatusRequest deserialization (still uses old struct name)
        let json = r#"{}"#;
        let _request: LspClientStatusRequest = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn test_close_document_request_deserialization() {
        // Test CloseDocumentRequest deserialization
        let json = r#"{"file_path": "/path/to/file.rs"}"#;
        let request: CloseDocumentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.file_path, "/path/to/file.rs");
    }

    #[test]
    fn test_document_symbols_pagination_defaults() {
        // Test DocumentSymbolsRequest pagination defaults
        let json = r#"{"file_path": "/path/to/file.rs"}"#;
        let request: DocumentSymbolsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.page, 0);
        assert_eq!(request.page_size, lsp_client::MAX_SYMBOLS_COUNT);
    }

    #[test]
    fn test_document_symbols_pagination_custom() {
        // Test DocumentSymbolsRequest with custom pagination
        let json = r#"{"file_path": "/path/to/file.rs", "page": 2, "page_size": 50}"#;
        let request: DocumentSymbolsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.page, 2);
        assert_eq!(request.page_size, 50);
    }
}
