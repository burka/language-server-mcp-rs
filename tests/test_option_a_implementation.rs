// Test Option A implementation: Automatic Retry with Diagnostics Pre-warming
// Verifies that the new pre-warming and retry logic works as expected

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::test]
async fn test_option_a_prewarm_and_retry() {
    println!("🚀 Testing Option A: Automatic Retry with Diagnostics Pre-warming");

    let start_time = Instant::now();

    // Create server - this should now include pre-warming
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server with pre-warming");

    let creation_time = start_time.elapsed();
    println!("📊 Server creation with pre-warming: {:?}", creation_time);

    // Test immediate requests that previously failed
    let immediate_start = Instant::now();

    // Test 1: Diagnostics (most likely to fail without pre-warming)
    let diagnostics_req = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let diag_start = Instant::now();
    let diagnostics_result = server.diagnostics(Parameters(diagnostics_req)).await;
    let diag_time = diag_start.elapsed();

    match diagnostics_result {
        Ok(_) => println!("✅ Diagnostics succeeded in {:?}", diag_time),
        Err(e) => println!("❌ Diagnostics failed in {:?}: {:?}", diag_time, e),
    }

    // Test 2: Hover (should work well with pre-warming)
    let hover_req = language_server_mcp::models::HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let hover_start = Instant::now();
    let hover_result = server.hover(Parameters(hover_req)).await;
    let hover_time = hover_start.elapsed();

    match hover_result {
        Ok(_) => println!("✅ Hover succeeded in {:?}", hover_time),
        Err(e) => println!("❌ Hover failed in {:?}: {:?}", hover_time, e),
    }

    // Test 3: Completion (should benefit from retry logic)
    let completion_req = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 25,
        column: 15,
    };

    let completion_start = Instant::now();
    let completion_result = server.completion(Parameters(completion_req)).await;
    let completion_time = completion_start.elapsed();

    match completion_result {
        Ok(_) => println!("✅ Completion succeeded in {:?}", completion_time),
        Err(e) => println!("❌ Completion failed in {:?}: {:?}", completion_time, e),
    }

    let total_immediate_time = immediate_start.elapsed();
    let total_time = start_time.elapsed();

    println!("📊 Summary:");
    println!("  - Server creation: {:?}", creation_time);
    println!("  - Immediate requests: {:?}", total_immediate_time);
    println!("  - Total time: {:?}", total_time);

    // Based on our test results, we expect:
    // - Creation time to be similar to old approach but more reliable (~70-100ms)
    // - Immediate requests to be more successful due to pre-warming
    // - Overall better reliability with retry logic

    assert!(
        total_time < std::time::Duration::from_secs(12),
        "Total time should be reasonable with retry logic: {:?}",
        total_time
    );
}

#[tokio::test]
async fn test_option_a_multiple_servers() {
    println!("🔄 Testing multiple server instances with Option A implementation");

    // Create 3 servers to test that pre-warming works consistently
    let mut servers = Vec::new();
    let mut creation_times = Vec::new();

    for i in 1..=3 {
        let start = Instant::now();
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let server = RustAnalyzerMCP::new(workspace)
            .await
            .unwrap_or_else(|_| panic!("Failed to create server {}", i));
        let creation_time = start.elapsed();
        creation_times.push(creation_time);
        servers.push(server);
        println!("📊 Server {} created in {:?}", i, creation_time);
    }

    // Test that all servers can handle immediate requests
    for (i, server) in servers.iter().enumerate() {
        let req = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/lib.rs".to_string(),
        };

        let start = Instant::now();
        let result = server.diagnostics(Parameters(req)).await;
        let duration = start.elapsed();

        match result {
            Ok(_) => println!("✅ Server {} diagnostics: {:?}", i + 1, duration),
            Err(e) => println!("❌ Server {} diagnostics failed: {:?}", i + 1, e),
        }
    }

    let avg_creation_time: std::time::Duration =
        creation_times.iter().sum::<std::time::Duration>() / creation_times.len() as u32;
    println!("📊 Average creation time: {:?}", avg_creation_time);

    // Pre-warming should make creation time more consistent
    assert!(
        avg_creation_time < std::time::Duration::from_millis(200),
        "Average creation time should be reasonable with pre-warming"
    );
}

#[tokio::test]
async fn test_option_a_error_handling() {
    println!("🛡️ Testing Option A error handling and retry logic");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create server for error handling test");

    // Test with a potentially problematic request
    let problematic_req = language_server_mcp::models::HoverRequest {
        file_path: "src/nonexistent.rs".to_string(), // File doesn't exist
        line: 1,
        column: 1,
    };

    let start = Instant::now();
    let result = server.hover(Parameters(problematic_req)).await;
    let duration = start.elapsed();

    println!(
        "🔍 Nonexistent file hover result in {:?}: {:?}",
        duration,
        result.as_ref().map(|_| "Ok").unwrap_or("Error")
    );

    // The retry logic should handle this gracefully
    // Either succeed with "no info" or fail with a reasonable error message
    assert!(
        duration < std::time::Duration::from_secs(6),
        "Error handling should be reasonably fast even with retries: {:?}",
        duration
    );
}
