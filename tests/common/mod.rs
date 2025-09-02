use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use std::path::PathBuf;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::time::Duration;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::timeout;
use std::collections::HashMap;
use tokio::task::JoinHandle;

// TEST ISOLATION INFRASTRUCTURE
// Provides proper isolation between tests to prevent Tokio runtime interference
// and resource conflicts. Each test can run in isolation or coordinate with others.

// Test isolation strategy:
// 1. Per-test resource isolation with unique identifiers
// 2. Controlled concurrency through semaphores
// 3. Proper cleanup hooks for all resources
// 4. Runtime separation to prevent conflicts
// 5. Optional sequential coordination for sensitive tests

// RESOURCE ISOLATION PRIMITIVES

// Semaphore to limit concurrent server instances to prevent resource exhaustion
#[allow(dead_code)]
pub static CONCURRENT_SERVER_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(3));

// Global test counter for unique test identifiers
#[allow(dead_code)]
static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

// Registry for tracking active test resources
#[allow(dead_code)]
pub static ACTIVE_TEST_RESOURCES: Lazy<Arc<Mutex<HashMap<u64, TestResourceHandle>>>> = 
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

// Handle for managing test resources with proper cleanup
pub struct TestResourceHandle {
    pub test_id: u64,
    pub server_handle: Option<Arc<Mutex<language_server_mcp::server::RustAnalyzerMCP>>>,
    pub lsp_handle: Option<Arc<Mutex<language_server_mcp::lsp_client::LspClient>>>,
    pub cleanup_tasks: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for TestResourceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TestResourceHandle")
            .field("test_id", &self.test_id)
            .field("server_handle", &self.server_handle.is_some())
            .field("lsp_handle", &self.lsp_handle.is_some())
            .field("cleanup_tasks", &self.cleanup_tasks.len())
            .finish()
    }
}

impl TestResourceHandle {
    pub fn new(test_id: u64) -> Self {
        Self {
            test_id,
            server_handle: None,
            lsp_handle: None,
            cleanup_tasks: Vec::new(),
        }
    }
    
    pub async fn cleanup(mut self) {
        // Wait for all cleanup tasks to complete
        for task in self.cleanup_tasks {
            let _ = task.await;
        }
        
        // Clean up server resources
        if let Some(server) = self.server_handle.take() {
            // Server cleanup happens via Drop trait
            drop(server);
        }
        
        // Clean up LSP resources  
        if let Some(lsp) = self.lsp_handle.take() {
            // LSP cleanup happens via Drop trait
            drop(lsp);
        }
    }
}

// Test timeout with automatic hang detection
#[allow(dead_code)]
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

// Wrap test function with standard timeout to prevent infinite hangs
#[allow(dead_code)]
pub async fn run_test_with_timeout<F, T>(
    test_name: &str, 
    timeout_duration: Duration, 
    test_fn: F
) -> Result<T, String>
where
    F: std::future::Future<Output = T>,
{
    match timeout(timeout_duration, test_fn).await {
        Ok(result) => Ok(result),
        Err(_) => {
            eprintln!(
                "❌ Test '{}' exceeded timeout of {:?} - likely resource contention or hang",
                test_name, timeout_duration
            );
            Err(format!("Test '{}' timed out after {:?}", test_name, timeout_duration))
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

// CORE ISOLATION FUNCTIONS

/// Create an isolated test environment with unique resources
/// This prevents tests from interfering with each other's state
#[allow(dead_code)]
pub async fn create_isolated_test_env() -> Result<TestEnvironment, String> {
    let test_id = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    
    // Add timeout to semaphore acquisition to prevent infinite hangs
    let permit = timeout(Duration::from_secs(30), CONCURRENT_SERVER_SEMAPHORE.acquire())
        .await
        .map_err(|_| format!("Timed out waiting for resource permit (test_id: {}). Too many concurrent tests competing for {} server slots", test_id, 3))?
        .map_err(|e| format!("Failed to acquire resource permit: {}", e))?;
    
    let handle = TestResourceHandle::new(test_id);
    
    // Register the test environment
    {
        let mut registry = ACTIVE_TEST_RESOURCES.lock().await;
        registry.insert(test_id, handle);
    }
    
    Ok(TestEnvironment {
        test_id,
        _permit: permit,
    })
}

/// Isolated test environment that ensures proper resource cleanup
pub struct TestEnvironment {
    pub test_id: u64,
    _permit: tokio::sync::SemaphorePermit<'static>,
}

impl TestEnvironment {
    /// Create an isolated MCP server for this test environment
    #[allow(dead_code)]
    pub async fn create_mcp_server(&self) -> Result<Arc<Mutex<language_server_mcp::server::RustAnalyzerMCP>>, String> {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        
        let server = language_server_mcp::server::RustAnalyzerMCP::new(workspace_root)
            .await
            .map_err(|e| {
                if format!("{}", e).contains("rust-analyzer not found") {
                    // Graceful exit for missing rust-analyzer
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    std::process::exit(0);
                }
                format!("Failed to create MCP server for test {}: {}", self.test_id, e)
            })?;
        
        let server_handle = Arc::new(Mutex::new(server));
        
        // Store the server handle for cleanup
        {
            let mut registry = ACTIVE_TEST_RESOURCES.lock().await;
            if let Some(handle) = registry.get_mut(&self.test_id) {
                handle.server_handle = Some(server_handle.clone());
            }
        }
        
        Ok(server_handle)
    }
    
    /// Create an isolated LSP client for this test environment  
    #[allow(dead_code)]
    pub async fn create_lsp_client(&self) -> Result<Arc<Mutex<language_server_mcp::lsp_client::LspClient>>, String> {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        
        let client = language_server_mcp::lsp_client::LspClient::new(&workspace_root)
            .await
            .map_err(|e| {
                if format!("{}", e).contains("rust-analyzer not found") {
                    // Graceful exit for missing rust-analyzer
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    std::process::exit(0);
                }
                format!("Failed to create LSP client for test {}: {}", self.test_id, e)
            })?;
            
        let client_handle = Arc::new(Mutex::new(client));
        
        // Store the client handle for cleanup
        {
            let mut registry = ACTIVE_TEST_RESOURCES.lock().await;
            if let Some(handle) = registry.get_mut(&self.test_id) {
                handle.lsp_handle = Some(client_handle.clone());
            }
        }
        
        Ok(client_handle)
    }
}

impl Drop for TestEnvironment {
    fn drop(&mut self) {
        // Spawn cleanup task to run in background, but with faster cleanup
        let test_id = self.test_id;
        tokio::spawn(async move {
            // Remove and cleanup test resources
            let handle = {
                let mut registry = ACTIVE_TEST_RESOURCES.lock().await;
                registry.remove(&test_id)
            };
            
            if let Some(handle) = handle {
                handle.cleanup().await;
            }
            
            // Small delay to ensure cleanup propagates before permit is released
            tokio::time::sleep(Duration::from_millis(100)).await;
        });
    }
}

// SEQUENTIAL COORDINATION FOR SENSITIVE TESTS

/// Semaphore for tests that must run sequentially to avoid conflicts
#[allow(dead_code)]  
pub static SEQUENTIAL_TEST_SEMAPHORE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(1));

/// Run a test function with sequential coordination
/// Use this for tests that are sensitive to concurrent execution
#[allow(dead_code)]
pub async fn run_sequential_test<F, T>(test_name: &str, test_fn: F) -> T
where
    F: std::future::Future<Output = T>,
{
    let _permit = SEQUENTIAL_TEST_SEMAPHORE.acquire().await
        .expect("Failed to acquire sequential test permit");
    
    println!("🔒 Running test '{}' in sequential mode", test_name);
    
    let result = test_fn.await;
    
    // Small delay to ensure proper cleanup between sequential tests
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    result
}

// LEGACY COMPATIBILITY FUNCTIONS (DEPRECATED - Use isolated environments instead)

// Shared legacy server for backwards compatibility
#[allow(dead_code)]
pub static LEGACY_SHARED_SERVER: Lazy<Arc<Mutex<Option<language_server_mcp::server::RustAnalyzerMCP>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

// Get or initialize a shared MCP server (DEPRECATED - use create_isolated_test_env instead)
#[allow(dead_code)]
pub async fn get_test_mcp_server() -> Arc<Mutex<Option<language_server_mcp::server::RustAnalyzerMCP>>> {
    eprintln!("⚠️  get_test_mcp_server() is deprecated. Use create_isolated_test_env().create_mcp_server() instead for better test isolation.");
    
    let server_option = LEGACY_SHARED_SERVER.clone();
    let mut server_guard = server_option.lock().await;

    if server_guard.is_none() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        match language_server_mcp::server::RustAnalyzerMCP::new(workspace_root).await {
            Ok(server) => {
                println!("✅ Legacy MCP server initialized for testing");
                *server_guard = Some(server);
            }
            Err(e) => {
                if format!("{}", e).contains("rust-analyzer not found") {
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    eprintln!("⚠️  Skipping integration tests...\n");
                    std::process::exit(0);
                }
                panic!("Failed to initialize legacy MCP server: {}", e);
            }
        }
    }

    drop(server_guard);
    server_option
}

// Create a fresh MCP server instance (DEPRECATED - use create_isolated_test_env instead)
#[allow(dead_code)]
pub async fn create_fresh_mcp_server() -> language_server_mcp::server::RustAnalyzerMCP {
    eprintln!("⚠️  create_fresh_mcp_server() is deprecated. Use create_isolated_test_env().create_mcp_server() instead for better test isolation.");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    language_server_mcp::server::RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create fresh MCP server")
}

// Shared legacy LSP client for backwards compatibility
#[allow(dead_code)]
pub static LEGACY_SHARED_CLIENT: Lazy<Arc<Mutex<Option<language_server_mcp::lsp_client::LspClient>>>> =
    Lazy::new(|| Arc::new(Mutex::new(None)));

// Get or initialize the test LSP client (DEPRECATED - use create_isolated_test_env instead)
#[allow(dead_code)]
pub async fn get_test_client() -> Arc<Mutex<Option<language_server_mcp::lsp_client::LspClient>>> {
    eprintln!("⚠️  get_test_client() is deprecated. Use create_isolated_test_env().create_lsp_client() instead for better test isolation.");
    
    let client_option = LEGACY_SHARED_CLIENT.clone();
    let mut client_guard = client_option.lock().await;

    if client_guard.is_none() {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        match language_server_mcp::lsp_client::LspClient::new(&workspace_root).await {
            Ok(client) => {
                println!("✅ Legacy rust-analyzer LSP client initialized for testing");
                *client_guard = Some(client);
            }
            Err(e) => {
                if format!("{}", e).contains("rust-analyzer not found") {
                    eprintln!("\n{}\n", e);
                    eprintln!("⚠️  Tests require rust-analyzer to be installed.");
                    eprintln!("⚠️  Run: rustup component add rust-analyzer");
                    eprintln!("⚠️  Skipping integration tests...\n");
                    std::process::exit(0);
                }
                panic!("Failed to initialize legacy LSP client: {}", e);
            }
        }
    }

    drop(client_guard);
    client_option
}

// DETERMINISTIC TIMING UTILITIES

/// Controlled delay with exponential backoff for retry patterns
#[allow(dead_code)]
pub async fn controlled_delay(base_ms: u64, attempt: u32) {
    let delay_ms = base_ms * (2u64.pow(attempt.min(5))); // Cap at 32x base delay
    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
}

/// Wait for a condition with timeout and predictable polling
#[allow(dead_code)]
pub async fn wait_for_condition<F>(
    condition: F,
    timeout_duration: Duration,
    poll_interval: Duration,
    description: &str,
) -> Result<(), String>
where
    F: Fn() -> bool,
{
    let start = std::time::Instant::now();
    
    while start.elapsed() < timeout_duration {
        if condition() {
            return Ok(());
        }
        
        tokio::time::sleep(poll_interval).await;
    }
    
    Err(format!(
        "Condition '{}' not met within {:?}",
        description, timeout_duration
    ))
}

/// Deterministic test timing with consistent behavior across environments
#[allow(dead_code)]
pub struct TestTiming {
    pub fast: Duration,
    pub standard: Duration,
    pub slow: Duration,
    pub polling: Duration,
}

impl Default for TestTiming {
    fn default() -> Self {
        Self {
            fast: Duration::from_millis(100),
            standard: Duration::from_millis(500),
            slow: Duration::from_secs(2),
            polling: Duration::from_millis(50),
        }
    }
}

#[allow(dead_code)]
pub static TEST_TIMING: Lazy<TestTiming> = Lazy::new(TestTiming::default);

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
