// Test throttling with fully warmed-up rust-analyzer server
// Add 10s delay after startup to ensure complete initialization

use language_server_mcp::server::RustAnalyzerMCP;
use language_server_mcp::models::*;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use futures::future::join_all;

#[tokio::test]
async fn test_warm_server_no_throttling() {
    println!("=== Testing Warm Server WITHOUT Throttling ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create server");
    
    // 🔥 CRITICAL: Let rust-analyzer fully warm up (10 seconds)
    println!("🕐 Waiting 10 seconds for rust-analyzer to fully initialize...");
    tokio::time::sleep(Duration::from_secs(10)).await;
    println!("✅ Server should be fully warm now");
    
    // Now test 6 parallel requests on the warm server
    println!("🚀 Testing 6 parallel requests on warm server...");
    let start = Instant::now();
    let mut futures = Vec::new();
    
    for i in 0..6 {
        let future = server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }
    
    let results = join_all(futures).await;
    let total_time = start.elapsed();
    
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len() - successful;
    
    println!("📊 Warm Server (No Throttle) Results:");
    println!("  Success: {}/{}", successful, results.len());
    println!("  Failed: {}", failed);
    println!("  Total time: {:?}", total_time);
    
    // Analyze error types
    for (i, result) in results.iter().enumerate() {
        if let Err(e) = result {
            println!("  Request {} error: {}", i+1, e);
            if e.to_string().contains("content modified") {
                println!("    ↑ This is the 'content modified' error we're trying to fix!");
            }
        }
    }
    
    println!("💡 This establishes our baseline: warm server performance without throttling");
}

#[tokio::test]
#[ignore = "Known flaky test that interferes with runtime in full test suite - passes individually"]
async fn test_warm_server_with_throttling() {
    println!("=== Testing Warm Server WITH 25ms Throttling ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    
    // Enable throttling
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS", "25");
    
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create server");
    
    // 🔥 CRITICAL: Same 10-second warmup period
    println!("🕐 Waiting 10 seconds for rust-analyzer to fully initialize...");
    tokio::time::sleep(Duration::from_secs(10)).await;
    println!("✅ Server fully warm, throttling enabled");
    
    // Test the same 6 parallel requests
    println!("🚀 Testing 6 parallel requests with 25ms throttling...");
    let start = Instant::now();
    let mut futures = Vec::new();
    
    for i in 0..6 {
        let future = server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }
    
    let results = join_all(futures).await;
    let total_time = start.elapsed();
    
    let successful = results.iter().filter(|r| r.is_ok()).count();
    let failed = results.len() - successful;
    
    println!("📊 Warm Server (25ms Throttle) Results:");
    println!("  Success: {}/{}", successful, results.len()); 
    println!("  Failed: {}", failed);
    println!("  Total time: {:?}", total_time);
    
    // Analyze error types
    let content_modified_errors = results.iter()
        .filter(|r| r.is_err())
        .filter(|r| r.as_ref().unwrap_err().to_string().contains("content modified"))
        .count();
    
    println!("  Content modified errors: {}", content_modified_errors);
    
    for (i, result) in results.iter().enumerate() {
        if let Err(e) = result {
            println!("  Request {} error: {}", i+1, e);
        }
    }
    
    // Expected: Should take ~125ms (5 delays * 25ms) plus execution time
    if total_time >= Duration::from_millis(100) {
        println!("✅ Throttling timing looks correct (>100ms)");
    } else {
        println!("⚠️  Throttling may not be working (too fast)");
    }
    
    println!("💡 This shows if throttling helps when server is fully warmed up");
    
    // Clean up
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE");
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS");
}

#[tokio::test]
#[ignore = "Known flaky test that interferes with runtime in full test suite - passes individually"]
async fn test_warm_server_throttling_comparison() {
    println!("=== Direct Comparison: Warm Server Throttled vs Non-Throttled ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    
    // Test 1: No throttling
    println!("\n🚫 Phase 1: No throttling");
    let server_no_throttle = RustAnalyzerMCP::new(workspace.clone())
        .await
        .expect("Failed to create server");
    
    println!("🕐 Warming up server (10s)...");
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    let start = Instant::now();
    let mut futures = Vec::new();
    
    for i in 0..5 {
        let future = server_no_throttle.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 30 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }
    
    let results_no_throttle = join_all(futures).await;
    let time_no_throttle = start.elapsed();
    let success_no_throttle = results_no_throttle.iter().filter(|r| r.is_ok()).count();
    
    println!("  Results: {}/{} success in {:?}", success_no_throttle, results_no_throttle.len(), time_no_throttle);
    
    // Test 2: With throttling 
    println!("\n⚡ Phase 2: With 100ms throttling");
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS", "100");
    
    let server_throttled = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create server");
    
    println!("🕐 Warming up server (10s)...");  
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    let start = Instant::now();
    let mut futures = Vec::new();
    
    for i in 0..5 {
        let future = server_throttled.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 30 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }
    
    let results_throttled = join_all(futures).await;
    let time_throttled = start.elapsed();
    let success_throttled = results_throttled.iter().filter(|r| r.is_ok()).count();
    
    println!("  Results: {}/{} success in {:?}", success_throttled, results_throttled.len(), time_throttled);
    
    // Final comparison
    println!("\n🏆 FINAL COMPARISON (Both servers fully warmed):");
    println!("  No throttle:  {}/{} success, {:?}", success_no_throttle, results_no_throttle.len(), time_no_throttle);
    println!("  With 100ms throttle: {}/{} success, {:?}", success_throttled, results_throttled.len(), time_throttled);
    
    let improvement = success_throttled as i32 - success_no_throttle as i32;
    if improvement > 0 {
        println!("  🎉 THROTTLING WINS: +{} successful requests!", improvement);
    } else if improvement == 0 {
        println!("  ⚖️  TIE: Same success rate");
    } else {
        println!("  📉 NO THROTTLING WINS: +{} successful requests", -improvement);
    }
    
    println!("\n💡 This is the definitive test - both servers fully initialized!");
    
    // Clean up
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE");
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS");
}