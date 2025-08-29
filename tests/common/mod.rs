use once_cell::sync::Lazy;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::timeout;

// Future: Direct MCP server testing infrastructure
// Currently we test through the LspClient directly for simplicity

// Since direct MCP server usage is complex due to macros, let's use the LspClient directly
// We'll initialize it lazily on first use within the test runtime
#[allow(dead_code)]
pub static TEST_LSP_CLIENT: Lazy<Arc<Mutex<Option<language_server_mcp::lsp_client::LspClient>>>> = Lazy::new(|| {
    Arc::new(Mutex::new(None))
});

// Test timeout with automatic hang detection
pub async fn with_timeout<T, F>(name: &str, duration: Duration, future: F) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match timeout(duration, future).await {
        Ok(result) => Ok(result),
        Err(_) => {
            eprintln!("⚠️  Test '{}' timed out after {:?} - possible hang detected", name, duration);
            Err(format!("Test '{}' timed out", name))
        }
    }
}

// Standard timeout for most operations  
#[allow(dead_code)]
pub const STANDARD_TIMEOUT: Duration = Duration::from_secs(5);
#[allow(dead_code)]
pub const TEST_TRAIT_RS: &str = "tests/test_trait.rs";

// Get or initialize the test LSP client
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
}