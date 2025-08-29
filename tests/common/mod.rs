use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

// MCP server testing infrastructure - now primary testing approach
// Direct MCP server testing provides better coverage than LSP client alone

// Shared MCP server instance for tests that can reuse it
#[allow(dead_code)]
pub static TEST_MCP_SERVER: Lazy<Arc<Mutex<Option<language_server_mcp::server::RustAnalyzerMCP>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

// Legacy LSP client for tests that specifically need direct LSP access
#[allow(dead_code)]
pub static TEST_LSP_CLIENT: Lazy<Arc<Mutex<Option<language_server_mcp::lsp_client::LspClient>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

// Test timeout with automatic hang detection
pub async fn with_timeout<T, F>(name: &str, duration: Duration, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => Ok(result),
        Err(_) => {
            eprintln!(
                "⚠️  Test '{}' timed out after {:?} - possible hang detected",
                name, duration
            );
            Err(format!("Test '{}' timed out", name))
        }
    }
}

// Standard timeouts for different operation types
#[allow(dead_code)]
pub const STANDARD_TIMEOUT: Duration = Duration::from_secs(5);
#[allow(dead_code)]
pub const FAST_TIMEOUT: Duration = Duration::from_secs(2);
#[allow(dead_code)]
pub const RAPID_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);

// Test file constants
#[allow(dead_code)]
pub const TEST_TRAIT_RS: &str = "tests/test_trait.rs";

// Helper to check if MCP result has content
#[allow(dead_code)]
pub fn has_mcp_content(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

// Helper for MCP timeout operations with better error reporting
#[allow(dead_code)]
pub async fn with_mcp_timeout<F, T>(name: &str, duration: Duration, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => Ok(result),
        Err(_) => {
            eprintln!(
                "⚠️  MCP operation '{}' timed out after {:?}",
                name, duration
            );
            Err(format!("MCP operation '{}' timed out", name))
        }
    }
}

// Get or initialize the shared MCP server (primary testing approach)
#[allow(dead_code)]
pub async fn get_test_mcp_server(
) -> Arc<Mutex<Option<language_server_mcp::server::RustAnalyzerMCP>>> {
    let server_option = TEST_MCP_SERVER.clone();
    let mut server_guard = server_option.lock().await;

    if server_guard.is_none() {
        // Initialize the MCP server with Option A pre-warming
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        match language_server_mcp::server::RustAnalyzerMCP::new(workspace_root).await {
            Ok(server) => {
                println!("✅ MCP server initialized for testing with Option A pre-warming");
                *server_guard = Some(server);
            }
            Err(e) => {
                if format!("{}", e).contains("rust-analyzer not found") {
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    eprintln!("⚠️  Skipping integration tests...\n");
                    std::process::exit(0); // Exit gracefully
                }
                panic!("Failed to initialize MCP server: {}", e);
            }
        }
    }

    // Return the shared server wrapped in Arc<Mutex<>>
    drop(server_guard);
    server_option
}

// Create a fresh MCP server instance (for tests that need isolation)
#[allow(dead_code)]
pub async fn create_fresh_mcp_server() -> language_server_mcp::server::RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    language_server_mcp::server::RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create fresh MCP server")
}

// Get or initialize the test LSP client (legacy, for direct LSP testing)
#[allow(dead_code)]
pub async fn get_test_client() -> Arc<Mutex<Option<language_server_mcp::lsp_client::LspClient>>> {
    let client_option = TEST_LSP_CLIENT.clone();
    let mut client_guard = client_option.lock().await;

    if client_guard.is_none() {
        // Initialize the client
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        match language_server_mcp::lsp_client::LspClient::new(&workspace_root).await {
            Ok(client) => {
                println!("✅ rust-analyzer LSP client initialized for testing");
                *client_guard = Some(client);
            }
            Err(e) => {
                if format!("{}", e).contains("rust-analyzer not found") {
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    eprintln!("⚠️  Skipping integration tests...\n");
                    std::process::exit(0); // Exit gracefully
                }
                panic!("Failed to initialize LSP client: {}", e);
            }
        }
    }

    // Return the shared client wrapped in Arc<Mutex<>>
    drop(client_guard);
    client_option
}

// Common test file paths in this project
#[allow(dead_code)]
pub mod test_files {
    #[allow(dead_code)]
    pub const MAIN_RS: &str = "src/main.rs";
    #[allow(dead_code)]
    pub const LSP_CLIENT_RS: &str = "src/lsp_client.rs";
    #[allow(dead_code)]
    pub const SERVER_RS: &str = "src/server.rs";
    #[allow(dead_code)]
    pub const LIB_RS: &str = "src/lib.rs";
    #[allow(dead_code)]
    pub const MODELS_RS: &str = "src/models.rs";
    #[allow(dead_code)]
    pub const ERRORS_RS: &str = "src/errors.rs";
}

// Common test positions for consistent testing
#[allow(dead_code)]
pub mod test_positions {
    pub struct TestPosition {
        pub file: &'static str,
        pub line: u32,
        pub column: u32,
        pub description: &'static str,
    }

    #[allow(dead_code)]
    pub const MAIN_STRUCT_POS: TestPosition = TestPosition {
        file: "src/main.rs",
        line: 13,
        column: 7,
        description: "Args struct in main.rs",
    };

    #[allow(dead_code)]
    pub const LSP_CLIENT_STRUCT_POS: TestPosition = TestPosition {
        file: "src/lsp_client.rs",
        line: 35,
        column: 15,
        description: "LspClient struct definition",
    };

    #[allow(dead_code)]
    pub const IMPORT_POS: TestPosition = TestPosition {
        file: "src/main.rs",
        line: 4,
        column: 35,
        description: "Import statement",
    };
}
