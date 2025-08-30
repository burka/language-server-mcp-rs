// Fast tests using shared rust-analyzer server
// Demonstrates dramatic performance improvements by reusing one server

mod common;

use common::*;
use futures::future::join_all;
use language_server_mcp::models::*;
use rmcp::handler::server::tool::Parameters;
use std::time::Instant;
// Removed unused import

/// Fast test using shared server - should complete in milliseconds
#[tokio::test]
async fn test_fast_shared_hover() {
    println!("=== Fast Shared Server Test ===");
    
    let start = Instant::now();
    
    // Get shared server (initializes once, reused across all tests)
    let server_container = get_test_mcp_server().await;
    let server_guard = server_container.lock().await;
    let server = server_guard.as_ref().expect("Shared server should be initialized");
    
    // Single hover request - should be fast
    let result = server.hover(Parameters(HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18, // main function
        column: 10,
    })).await;
    
    let elapsed = start.elapsed();
    println!("✅ Shared server hover completed in {:?}", elapsed);
    
    assert!(result.is_ok(), "Hover should succeed");
    assert!(elapsed.as_millis() < 1000, "Should be fast (< 1s)");
}

/// Test multiple requests on shared server
#[tokio::test]
async fn test_fast_shared_multiple_requests() {
    println!("=== Multiple Requests on Shared Server ===");
    
    let start = Instant::now();
    
    let server_container = get_test_mcp_server().await;
    let server_guard = server_container.lock().await;
    let server = server_guard.as_ref().expect("Shared server should be initialized");
    
    // Multiple parallel requests
    let mut futures = Vec::new();
    for i in 0..5 {
        let future = server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 18 + i,
            column: 10,
        }));
        futures.push(future);
    }
    
    let results = join_all(futures).await;
    let elapsed = start.elapsed();
    
    let successful = results.iter().filter(|r| r.is_ok()).count();
    println!("✅ {}/{} requests successful in {:?}", successful, results.len(), elapsed);
    
    assert!(successful >= 3, "Most requests should succeed");
    assert!(elapsed.as_millis() < 2000, "Should be fast (< 2s)");
}

/// Benchmark: Show the difference between fresh vs shared server
#[tokio::test]
async fn test_benchmark_fresh_vs_shared() {
    println!("=== Benchmark: Fresh vs Shared Server ===");
    
    // Test 1: Fresh server (slow)
    println!("🐌 Testing fresh server...");
    let fresh_start = Instant::now();
    let fresh_server = create_fresh_mcp_server().await;
    let fresh_result = fresh_server.hover(Parameters(HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18,
        column: 10,
    })).await;
    let fresh_time = fresh_start.elapsed();
    
    // Test 2: Shared server (fast)
    println!("🚀 Testing shared server...");
    let shared_start = Instant::now();
    let server_container = get_test_mcp_server().await;
    let server_guard = server_container.lock().await;
    let shared_server = server_guard.as_ref().expect("Shared server should be initialized");
    let shared_result = shared_server.hover(Parameters(HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18,
        column: 10,
    })).await;
    let shared_time = shared_start.elapsed();
    
    println!("📊 Performance Comparison:");
    println!("  Fresh server:  {:?} (includes startup)", fresh_time);
    println!("  Shared server: {:?} (no startup)", shared_time);
    
    let speedup = fresh_time.as_millis() as f64 / shared_time.as_millis() as f64;
    println!("  Speedup: {:.1}x faster", speedup);
    
    assert!(fresh_result.is_ok() && shared_result.is_ok(), "Both should succeed");
    assert!(shared_time < fresh_time, "Shared should be faster");
    assert!(speedup > 2.0, "Should be at least 2x faster");
}

/// Test that demonstrates why some tests are slow (rust-analyzer startup)
#[tokio::test]
async fn test_explain_slow_tests() {
    println!("=== Why Some Tests Are Slow ===");
    
    // Simulate what slow tests do: create multiple fresh servers
    let times = vec![];
    let mut times = times;
    
    for i in 0..3 {
        println!("Starting server {}...", i + 1);
        let start = Instant::now();
        let _server = create_fresh_mcp_server().await;
        let elapsed = start.elapsed();
        times.push(elapsed);
        println!("  Server {} startup: {:?}", i + 1, elapsed);
    }
    
    let total: std::time::Duration = times.iter().sum();
    let average = total / times.len() as u32;
    
    println!("📈 Startup Analysis:");
    println!("  Individual startups: {:?}", times);
    println!("  Total time: {:?}", total);
    println!("  Average startup: {:?}", average);
    println!("  💡 Solution: Use shared server to avoid repeated startups!");
}

// Removed problematic helper - use direct access pattern in tests