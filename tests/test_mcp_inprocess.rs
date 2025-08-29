// In-process MCP server tests
// Tests the MCP server by calling it directly without going through subprocess

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::ServerHandler;
use std::path::PathBuf;

/// Create a test MCP server instance
async fn create_test_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server")
}

#[tokio::test]
async fn test_server_creation() {
    let server = create_test_server().await;

    // Test that the server was created successfully
    assert_eq!(
        server.workspace_root().file_name().unwrap(),
        "language-server-mcp-rs"
    );
}

#[tokio::test]
async fn test_server_info() {
    let server = create_test_server().await;

    // Get server info
    let info = server.get_info();

    // Verify server info (rmcp is the framework name)
    assert_eq!(info.server_info.name, "rmcp");
    assert_eq!(info.server_info.version, "0.6.0");

    // Verify capabilities
    assert!(info.capabilities.tools.is_some());

    // Verify tools capability
    let tools_cap = info.capabilities.tools.unwrap();
    // list_changed is optional and may be None
    assert!(tools_cap.list_changed.is_none() || tools_cap.list_changed == Some(false));
}

// For actual tool testing, we'll use the tool_handlers module which is already public
// and tested in test_tool_handlers.rs. The main value of this test file is:
// 1. Proving the server can be created in-process (not just as subprocess)
// 2. Testing server metadata and configuration
// 3. Enabling future protocol-level testing when rmcp supports in-process duplex streams

#[tokio::test]
async fn test_server_workspace_root() {
    // Use the current project directory which exists
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace.clone())
        .await
        .expect("Failed to create MCP server");

    assert_eq!(server.workspace_root(), &workspace);
}
