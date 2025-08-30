// Quick test to analyze the exact timing pattern of content modified errors
use futures::future::join_all;
use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_timing_analysis() {
    println!("🔍 TIMING ANALYSIS: When do 'content modified' errors occur?");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create server");

    println!("✅ Server created, starting timing tests...");

    // Test 1: Immediate hover request (should potentially work)
    println!("\n1️⃣ Immediate hover request:");
    let start1 = Instant::now();
    let result1 = server
        .hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 10,
            column: 10,
        }))
        .await;
    let time1 = start1.elapsed();

    match result1 {
        Ok(_) => println!("✅ Immediate request: SUCCESS in {:?}", time1),
        Err(e) => {
            if e.to_string().contains("content modified") {
                println!("❌ Immediate request: CONTENT MODIFIED in {:?}", time1);
            } else {
                println!("⚠️  Immediate request: OTHER ERROR in {:?}: {}", time1, e);
            }
        }
    }

    // Test 2: Wait 2 seconds, then try
    println!("\n2️⃣ After 2 second delay:");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let start2 = Instant::now();
    let result2 = server
        .hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20,
            column: 10,
        }))
        .await;
    let time2 = start2.elapsed();

    match result2 {
        Ok(_) => println!("✅ After 2s: SUCCESS in {:?}", time2),
        Err(e) => {
            if e.to_string().contains("content modified") {
                println!("❌ After 2s: CONTENT MODIFIED in {:?}", time2);
            } else {
                println!("⚠️  After 2s: OTHER ERROR in {:?}: {}", time2, e);
            }
        }
    }

    // Test 3: Rapid sequence to trigger race condition
    println!("\n3️⃣ Rapid sequence (3 requests immediately):");
    let start3 = Instant::now();
    let mut futures = Vec::new();

    for i in 0..3 {
        let future = server.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 30 + i,
            column: 10,
        }));
        futures.push(future);
    }

    let results3 = join_all(futures).await;
    let time3 = start3.elapsed();

    let mut success_count = 0;
    let mut content_modified_count = 0;
    let mut other_error_count = 0;

    for (i, result) in results3.iter().enumerate() {
        match result {
            Ok(_) => {
                success_count += 1;
                println!("  Request {}: SUCCESS", i + 1);
            }
            Err(e) => {
                if e.to_string().contains("content modified") {
                    content_modified_count += 1;
                    println!("  Request {}: CONTENT MODIFIED", i + 1);
                } else {
                    other_error_count += 1;
                    println!("  Request {}: OTHER ERROR: {}", i + 1, e);
                }
            }
        }
    }

    println!("📊 Rapid sequence results ({:?} total):", time3);
    println!("  ✅ Success: {}", success_count);
    println!("  ❌ Content Modified: {}", content_modified_count);
    println!("  ⚠️  Other Errors: {}", other_error_count);

    // Test 4: Does it recover?
    println!("\n4️⃣ Recovery test (after 5 second wait):");
    tokio::time::sleep(Duration::from_secs(5)).await;
    let start4 = Instant::now();
    let result4 = server
        .hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 40,
            column: 10,
        }))
        .await;
    let time4 = start4.elapsed();

    match result4 {
        Ok(_) => println!("✅ Recovery: SUCCESS in {:?}", time4),
        Err(e) => {
            if e.to_string().contains("content modified") {
                println!("❌ Recovery: STILL CONTENT MODIFIED in {:?}", time4);
            } else {
                println!("⚠️  Recovery: OTHER ERROR in {:?}: {}", time4, e);
            }
        }
    }

    println!("\n💡 ANALYSIS COMPLETE - Check patterns above!");
}
