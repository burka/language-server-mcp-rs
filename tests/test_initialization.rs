// Tests for rust-analyzer initialization behavior and pre-warming
// Explores what happens when we send requests to a not-yet-initialized server

use futures::future::join_all;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Create a fresh MCP server and immediately send requests before it's fully warmed up
#[tokio::test]
async fn test_immediate_requests_to_fresh_server() {
    println!("🚀 Testing immediate requests to fresh rust-analyzer server...");

    let start_time = Instant::now();

    // Create a fresh server (this starts rust-analyzer subprocess)
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    let server_arc = Arc::new(Mutex::new(server));

    let creation_time = start_time.elapsed();
    println!("🔧 Server created in {:?}", creation_time);

    // Immediately fire requests without any warm-up delay
    let server = server_arc.lock().await;

    // Immediately fire requests without any warm-up delay
    println!("🏃 Firing immediate requests to fresh server (no warm-up delay)...");

    // Test 1: LSP Status
    {
        let req = language_server_mcp::models::LspClientStatusRequest {};
        let start = Instant::now();
        let result = server.lsp_status(Parameters(req)).await;
        let duration = start.elapsed();
        let status = if result.is_ok() { "✅" } else { "❌" };
        let summary = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("{:?}", e).chars().take(50).collect(),
        };
        println!("  {} LSP Status: {:?} - {}", status, duration, summary);
    }

    // Test 2: Diagnostics
    {
        let req = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/main.rs".to_string(),
        };
        let start = Instant::now();
        let result = server.diagnostics(Parameters(req)).await;
        let duration = start.elapsed();
        let status = if result.is_ok() { "✅" } else { "❌" };
        let summary = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("{:?}", e).chars().take(50).collect(),
        };
        println!("  {} Diagnostics: {:?} - {}", status, duration, summary);
    }

    // Test 3: Hover
    {
        let req = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 5,
        };
        let start = Instant::now();
        let result = server.hover(Parameters(req)).await;
        let duration = start.elapsed();
        let status = if result.is_ok() { "✅" } else { "❌" };
        let summary = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("{:?}", e).chars().take(50).collect(),
        };
        println!("  {} Hover: {:?} - {}", status, duration, summary);
    }

    // Test 4: Workspace Symbols
    {
        let req = language_server_mcp::models::WorkspaceSymbolsRequest {
            query: "main".to_string(),
        };
        let start = Instant::now();
        let result = server.workspace_symbols(Parameters(req)).await;
        let duration = start.elapsed();
        let status = if result.is_ok() { "✅" } else { "❌" };
        let summary = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("{:?}", e).chars().take(50).collect(),
        };
        println!(
            "  {} Workspace Symbols: {:?} - {}",
            status, duration, summary
        );
    }

    // Test 5: Document Symbols
    {
        let req = language_server_mcp::models::DocumentSymbolsRequest {
            file_path: "src/main.rs".to_string(),
            page: 0,
            page_size: 50,
        };
        let start = Instant::now();
        let result = server.document_symbols(Parameters(req)).await;
        let duration = start.elapsed();
        let status = if result.is_ok() { "✅" } else { "❌" };
        let summary = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => format!("{:?}", e).chars().take(50).collect(),
        };
        println!(
            "  {} Document Symbols: {:?} - {}",
            status, duration, summary
        );
    }

    let total_time = start_time.elapsed();
    println!("🏁 Total test time: {:?}", total_time);
}

#[tokio::test]
async fn test_initialization_state_progression() {
    println!("🔄 Testing rust-analyzer initialization state progression...");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    let server_arc = Arc::new(Mutex::new(server));
    let server = server_arc.lock().await;

    // Test how responses change over time as rust-analyzer initializes
    let time_intervals = vec![0, 100, 500, 1000, 2000]; // milliseconds

    for (i, delay_ms) in time_intervals.iter().enumerate() {
        if *delay_ms > 0 {
            println!("⏳ Waiting {}ms for further initialization...", delay_ms);
            tokio::time::sleep(Duration::from_millis(*delay_ms)).await;
        }

        println!("🔍 Test round {} (after {}ms):", i + 1, delay_ms);

        // Test LSP status to see initialization progress
        let status_req = language_server_mcp::models::LspClientStatusRequest {};
        let start = Instant::now();
        let status_result = server.lsp_status(Parameters(status_req)).await;
        let status_duration = start.elapsed();

        match status_result {
            Ok(result) => {
                // Try to extract first few lines of text content for summary
                let summary = if let Some(_content) = result.content.first() {
                    // Extract text from content - the exact field structure may vary
                    format!("Content available ({})", result.content.len())
                } else {
                    "No content".to_string()
                };
                println!("  📊 Status ({:?}): {}", status_duration, summary);
            }
            Err(e) => println!("  ❌ Status error: {:?}", e),
        }

        // Test a simple operation (diagnostics)
        let diag_req = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/main.rs".to_string(),
        };
        let start = Instant::now();
        let diag_result = server.diagnostics(Parameters(diag_req)).await;
        let diag_duration = start.elapsed();

        match diag_result {
            Ok(_) => println!("  ✅ Diagnostics ({:?}): Success", diag_duration),
            Err(e) => println!("  ❌ Diagnostics ({:?}): {:?}", diag_duration, e),
        }

        // Test a more complex operation (hover)
        let hover_req = language_server_mcp::models::HoverRequest {
            file_path: "src/lib.rs".to_string(),
            line: 1,
            column: 10,
        };
        let start = Instant::now();
        let hover_result =
            timeout(Duration::from_secs(3), server.hover(Parameters(hover_req))).await;
        let hover_duration = start.elapsed();

        match hover_result {
            Ok(Ok(_)) => println!("  ✅ Hover ({:?}): Success", hover_duration),
            Ok(Err(e)) => println!("  ❌ Hover ({:?}): {:?}", hover_duration, e),
            Err(_) => println!("  ⏰ Hover: Timed out after 3s"),
        }
    }
}

#[tokio::test]
async fn test_parallel_initialization_requests() {
    println!("⚡ Testing parallel requests during initialization...");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    let server_arc = Arc::new(Mutex::new(server));

    println!("🚀 Launching 5 parallel requests...");
    let start_time = Instant::now();

    let mut tasks = Vec::new();

    // Task 1: Status 1
    {
        let server_clone = server_arc.clone();
        let task = tokio::spawn(async move {
            let server = server_clone.lock().await;
            let req = language_server_mcp::models::LspClientStatusRequest {};
            let start = Instant::now();
            let result = server.lsp_status(Parameters(req)).await;
            ("Status 1".to_string(), start.elapsed(), result.is_ok())
        });
        tasks.push(task);
    }

    // Task 2: Diagnostics 1
    {
        let server_clone = server_arc.clone();
        let task = tokio::spawn(async move {
            let server = server_clone.lock().await;
            let req = language_server_mcp::models::DiagnosticsRequest {
                file_path: "src/main.rs".to_string(),
            };
            let start = Instant::now();
            let result = server.diagnostics(Parameters(req)).await;
            ("Diagnostics 1".to_string(), start.elapsed(), result.is_ok())
        });
        tasks.push(task);
    }

    // Task 3: Diagnostics 2
    {
        let server_clone = server_arc.clone();
        let task = tokio::spawn(async move {
            let server = server_clone.lock().await;
            let req = language_server_mcp::models::DiagnosticsRequest {
                file_path: "src/lib.rs".to_string(),
            };
            let start = Instant::now();
            let result = server.diagnostics(Parameters(req)).await;
            ("Diagnostics 2".to_string(), start.elapsed(), result.is_ok())
        });
        tasks.push(task);
    }

    // Task 4: Hover 1
    {
        let server_clone = server_arc.clone();
        let task = tokio::spawn(async move {
            let server = server_clone.lock().await;
            let req = language_server_mcp::models::HoverRequest {
                file_path: "src/main.rs".to_string(),
                line: 10,
                column: 5,
            };
            let start = Instant::now();
            let result = timeout(Duration::from_secs(5), server.hover(Parameters(req))).await;
            let duration = start.elapsed();
            (
                "Hover 1".to_string(),
                duration,
                result.is_ok() && result.unwrap().is_ok(),
            )
        });
        tasks.push(task);
    }

    // Task 5: Status 2 (with delay)
    {
        let server_clone = server_arc.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await; // Slight delay
            let server = server_clone.lock().await;
            let req = language_server_mcp::models::LspClientStatusRequest {};
            let start = Instant::now();
            let result = server.lsp_status(Parameters(req)).await;
            ("Status 2".to_string(), start.elapsed(), result.is_ok())
        });
        tasks.push(task);
    }

    let results = join_all(tasks).await;
    let total_time = start_time.elapsed();

    println!(
        "📊 Parallel request results (total time: {:?}):",
        total_time
    );
    for result in results {
        match result {
            Ok((name, duration, success)) => {
                let status = if success { "✅" } else { "❌" };
                println!("  {} {}: {:?}", status, name, duration);
            }
            Err(e) => println!("  💥 Task failed: {:?}", e),
        }
    }
}

#[tokio::test]
async fn test_pre_warming_strategies() {
    println!("🔥 Testing different pre-warming strategies...");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Strategy 1: No pre-warming (immediate use)
    {
        println!("\n📋 Strategy 1: No pre-warming");
        let start = Instant::now();
        let server = RustAnalyzerMCP::new(workspace.clone())
            .await
            .expect("Failed to create server");
        let server_arc = Arc::new(Mutex::new(server));
        let server = server_arc.lock().await;

        let create_time = start.elapsed();

        let req = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 5,
        };

        let request_start = Instant::now();
        let result = timeout(Duration::from_secs(5), server.hover(Parameters(req))).await;
        let request_time = request_start.elapsed();
        let total_time = start.elapsed();

        println!(
            "  Create: {:?}, First request: {:?}, Total: {:?}, Success: {}",
            create_time,
            request_time,
            total_time,
            result.is_ok() && result.as_ref().unwrap().is_ok()
        );
    }

    // Strategy 2: Pre-warm with status check
    {
        println!("\n📋 Strategy 2: Pre-warm with status check");
        let start = Instant::now();
        let server = RustAnalyzerMCP::new(workspace.clone())
            .await
            .expect("Failed to create server");
        let server_arc = Arc::new(Mutex::new(server));
        let server = server_arc.lock().await;

        let create_time = start.elapsed();

        // Pre-warming: Check status to ensure rust-analyzer is responding
        let status_req = language_server_mcp::models::LspClientStatusRequest {};
        let _ = server.lsp_status(Parameters(status_req)).await;
        let prewarm_time = start.elapsed();

        let req = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 5,
        };

        let request_start = Instant::now();
        let result = timeout(Duration::from_secs(5), server.hover(Parameters(req))).await;
        let request_time = request_start.elapsed();
        let total_time = start.elapsed();

        println!(
            "  Create: {:?}, Pre-warm: {:?}, First request: {:?}, Total: {:?}, Success: {}",
            create_time,
            prewarm_time,
            request_time,
            total_time,
            result.is_ok() && result.as_ref().unwrap().is_ok()
        );
    }

    // Strategy 3: Pre-warm with simple operation
    {
        println!("\n📋 Strategy 3: Pre-warm with simple operation");
        let start = Instant::now();
        let server = RustAnalyzerMCP::new(workspace.clone())
            .await
            .expect("Failed to create server");
        let server_arc = Arc::new(Mutex::new(server));
        let server = server_arc.lock().await;

        let create_time = start.elapsed();

        // Pre-warming: Perform a simple diagnostics check
        let diag_req = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/main.rs".to_string(),
        };
        let _ = server.diagnostics(Parameters(diag_req)).await;
        let prewarm_time = start.elapsed();

        let req = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 5,
        };

        let request_start = Instant::now();
        let result = timeout(Duration::from_secs(5), server.hover(Parameters(req))).await;
        let request_time = request_start.elapsed();
        let total_time = start.elapsed();

        println!(
            "  Create: {:?}, Pre-warm: {:?}, First request: {:?}, Total: {:?}, Success: {}",
            create_time,
            prewarm_time,
            request_time,
            total_time,
            result.is_ok() && result.as_ref().unwrap().is_ok()
        );
    }
}
