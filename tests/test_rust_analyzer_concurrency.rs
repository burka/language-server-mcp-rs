// Test rust-analyzer's ability to handle concurrent requests
// This will determine if we can use a shared instance architecture

use futures::future::join_all;
use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Create a single test server instance
async fn create_test_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server for concurrency test")
}

#[tokio::test]
async fn test_rust_analyzer_sequential_baseline() {
    println!("=== Baseline: Sequential Request Performance ===");
    let server = create_test_server().await;

    let requests = vec![
        ("hover", "src/main.rs", 18, 15),
        ("hover", "src/server.rs", 50, 10),
        ("hover", "src/lsp_client.rs", 100, 20),
        ("hover", "src/models.rs", 20, 10),
        ("hover", "src/tool_handlers.rs", 15, 5),
    ];

    let mut total_time = Duration::new(0, 0);
    let mut individual_times = Vec::new();

    for (i, (method, file, line, column)) in requests.into_iter().enumerate() {
        let start = Instant::now();

        let result = server
            .hover(Parameters(HoverRequest {
                file_path: file.to_string(),
                line,
                column,
            }))
            .await;

        let duration = start.elapsed();
        individual_times.push(duration);
        total_time += duration;

        match result {
            Ok(_) => println!(
                "✅ Request {}: {} completed in {:?}",
                i + 1,
                method,
                duration
            ),
            Err(e) => println!(
                "⚠️  Request {}: {} error in {:?}: {}",
                i + 1,
                method,
                duration,
                e
            ),
        }
    }

    println!("\n📊 Sequential Results:");
    println!("  Individual times: {:?}", individual_times);
    println!("  Total time: {:?}", total_time);
    println!(
        "  Average per request: {:?}",
        total_time / individual_times.len() as u32
    );
}

#[tokio::test]
async fn test_rust_analyzer_concurrent_requests() {
    println!("=== Testing Concurrent Request Handling ===");
    let server = create_test_server().await;

    // Create 5 hover requests to fire simultaneously (same type for simplicity)
    let futures = vec![
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 18,
            column: 15,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/server.rs".to_string(),
            line: 50,
            column: 10,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/lsp_client.rs".to_string(),
            line: 100,
            column: 20,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/models.rs".to_string(),
            line: 20,
            column: 10,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/tool_handlers.rs".to_string(),
            line: 15,
            column: 5,
        })),
    ];

    println!("🚀 Firing {} concurrent requests...", futures.len());
    let start = Instant::now();

    // Execute all requests concurrently
    let results = join_all(futures).await;

    let total_time = start.elapsed();

    // Analyze results
    let mut successful = 0;
    let mut errors = 0;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(_) => {
                successful += 1;
                println!("✅ Concurrent request {} succeeded", i + 1);
            }
            Err(e) => {
                errors += 1;
                println!("⚠️  Concurrent request {} error: {}", i + 1, e);
            }
        }
    }

    println!("\n📊 Concurrent Results:");
    println!("  Total time: {:?}", total_time);
    println!("  Successful: {}/{}", successful, results.len());
    println!("  Errors: {}", errors);

    // Analysis: If truly concurrent, total time should be close to the slowest individual request
    // If serialized, total time would be close to sum of individual request times

    if total_time < Duration::from_secs(3) {
        println!("🎉 CONCURRENT: Total time suggests parallel processing!");
        println!("   → Shared rust-analyzer instance architecture is viable");
    } else {
        println!("⚠️  SERIALIZED: Total time suggests sequential processing");
        println!("   → Need pooled rust-analyzer instances or different approach");
    }
}

#[tokio::test]
async fn test_mixed_concurrent_operations() {
    println!("=== Testing Mixed Concurrent Operations ===");
    let server = create_test_server().await;

    // Test different operations sequentially for now (mixed types are complex)
    let start = Instant::now();

    println!("🔀 Testing hover requests on different files...");
    let hover_futures = vec![
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20,
            column: 10,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/server.rs".to_string(),
            line: 50,
            column: 15,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/lsp_client.rs".to_string(),
            line: 100,
            column: 20,
        })),
    ];

    let hover_results = join_all(hover_futures).await;

    println!("🔍 Testing diagnostics requests...");
    let diag_futures = vec![
        server.diagnostics(Parameters(DiagnosticsRequest {
            file_path: "src/models.rs".to_string(),
        })),
        server.diagnostics(Parameters(DiagnosticsRequest {
            file_path: "src/tool_handlers.rs".to_string(),
        })),
    ];

    let diag_results = join_all(diag_futures).await;

    // Combine results for analysis
    let mut results = Vec::new();
    results.extend(hover_results);
    results.extend(diag_results);

    let total_time = start.elapsed();

    let mut operation_results = Vec::new();
    let operations = ["hover1", "hover2", "hover3", "diag1", "diag2"];

    for (i, result) in results.iter().enumerate() {
        let op_name = operations.get(i).unwrap_or(&"unknown");
        match result {
            Ok(_) => {
                operation_results.push(format!("✅ {}", op_name));
            }
            Err(e) => {
                operation_results.push(format!("❌ {}: {}", op_name, e));
            }
        }
    }

    println!("\n📊 Mixed Operations Results:");
    println!("  Total time: {:?}", total_time);
    for result in operation_results {
        println!("  {}", result);
    }

    // Test passes if we get reasonable performance with mixed operations
    assert!(
        total_time < Duration::from_secs(8),
        "Mixed operations should complete in reasonable time: {:?}",
        total_time
    );
}

#[tokio::test]
async fn test_concurrent_same_file_requests() {
    println!("=== Testing Concurrent Requests on Same File ===");
    let server = create_test_server().await;

    // Multiple hover requests on the same file to test file-level locking
    let futures = vec![
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 5,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 15,
            column: 10,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20,
            column: 15,
        })),
        server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 25,
            column: 20,
        })),
    ];

    println!(
        "📄 Testing {} concurrent requests on same file...",
        futures.len()
    );
    let start = Instant::now();

    let results = join_all(futures).await;
    let total_time = start.elapsed();

    let successful = results.iter().filter(|r| r.is_ok()).count();

    println!("\n📊 Same File Results:");
    println!("  Total time: {:?}", total_time);
    println!("  Successful: {}/{}", successful, results.len());

    if total_time < Duration::from_secs(2) {
        println!("🎉 Same file requests processed concurrently!");
    } else {
        println!("⚠️  Same file requests may have file-level serialization");
    }
}

#[tokio::test]
async fn test_rapid_fire_requests() {
    println!("=== Testing Rapid Fire Request Pattern ===");
    let server = create_test_server().await;

    // Simulate the kind of rapid requests we see in parallel test execution
    let request_count = 10;
    let mut futures = Vec::new();

    for i in 0..request_count {
        let future = server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 18 + (i % 5) as u32, // Vary positions slightly
            column: 10 + (i % 3) as u32,
        }));
        futures.push(future);
    }

    println!("⚡ Firing {} rapid requests...", request_count);
    let start = Instant::now();

    let results = join_all(futures).await;
    let total_time = start.elapsed();

    let successful = results.iter().filter(|r| r.is_ok()).count();
    let avg_time = total_time / request_count as u32;

    println!("\n📊 Rapid Fire Results:");
    println!("  Total time: {:?}", total_time);
    println!("  Average per request: {:?}", avg_time);
    println!("  Successful: {}/{}", successful, request_count);

    // If truly concurrent, average time should be much less than individual request time
    if avg_time < Duration::from_millis(200) {
        println!("🚀 EXCELLENT: High concurrency achieved!");
        println!("   → rust-analyzer handles rapid parallel requests very well");
    } else if avg_time < Duration::from_millis(500) {
        println!("✅ GOOD: Decent concurrency with some serialization");
        println!("   → Shared instance still viable but with some queuing");
    } else {
        println!("⚠️  LIMITED: Significant serialization detected");
        println!("   → May need pooled instances instead of single shared instance");
    }
}
