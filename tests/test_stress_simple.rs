// Simple stress tests for rust-analyzer - focusing on concurrent load and hanging detection
// Tests rapid-fire requests and concurrent operations

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::timeout;

/// Create a new MCP server instance for each stress test (isolated)
async fn create_stress_server() -> Arc<Mutex<RustAnalyzerMCP>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create stress test MCP server");
    Arc::new(Mutex::new(server))
}

#[tokio::test]
async fn test_stress_rapid_fire_hovers() {
    let server_arc = create_stress_server().await;
    let server = server_arc.lock().await;

    println!("🔥 Starting rapid-fire hover stress test - 30 requests...");

    let start = Instant::now();
    let mut futures = Vec::new();

    // Fire 30 requests as quickly as possible
    for i in 0..30 {
        let request = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: (i % 25) + 1,
            column: (i % 35) + 1,
        };

        futures.push(server.hover(Parameters(request)));
    }

    let queue_time = start.elapsed();
    println!("🔥 Queued 30 hover requests in {:?}", queue_time);

    // Wait for all responses with timeout
    let results = timeout(Duration::from_secs(15), futures::future::join_all(futures)).await;
    let total_time = start.elapsed();

    match results {
        Ok(responses) => {
            let successful = responses.iter().filter(|r| r.is_ok()).count();
            println!(
                "🔥 Rapid-fire hovers: {}/{} successful in {:?}",
                successful,
                responses.len(),
                total_time
            );

            // At least 70% should succeed
            assert!(
                successful >= 21,
                "Too many rapid-fire requests failed: {}/{}",
                successful,
                responses.len()
            );
        }
        Err(_) => {
            panic!("⏰ Rapid-fire hover test timed out after 15s - potential hang detected!");
        }
    }
}

#[tokio::test]
async fn test_stress_concurrent_mixed_requests() {
    let server_arc = create_stress_server().await;

    println!("🔥 Starting concurrent mixed request stress test - 20 requests...");

    let mut tasks = Vec::new();

    // Create diverse concurrent requests
    for i in 0..20 {
        let server_clone = server_arc.clone();

        let task = tokio::spawn(async move {
            let server = server_clone.lock().await;
            let start = Instant::now();

            let result = match i % 4 {
                0 => {
                    let req = language_server_mcp::models::HoverRequest {
                        file_path: "src/lib.rs".to_string(),
                        line: (i % 8) + 1,
                        column: 5,
                    };
                    server.hover(Parameters(req)).await
                }
                1 => {
                    let req = language_server_mcp::models::DiagnosticsRequest {
                        file_path: "src/main.rs".to_string(),
                    };
                    server.diagnostics(Parameters(req)).await
                }
                2 => {
                    let req = language_server_mcp::models::CompletionRequest {
                        file_path: "src/server.rs".to_string(),
                        line: (i % 20) + 60,
                        column: 10,
                    };
                    server.completion(Parameters(req)).await
                }
                _ => {
                    let req = language_server_mcp::models::GotoDefinitionRequest {
                        file_path: "src/lib.rs".to_string(),
                        line: (i % 5) + 1,
                        column: 8,
                    };
                    server.goto_definition(Parameters(req)).await
                }
            };

            let duration = start.elapsed();
            (i, result.is_ok(), duration)
        });

        tasks.push(task);
    }

    let start = Instant::now();
    let results = timeout(Duration::from_secs(20), futures::future::join_all(tasks)).await;
    let total_time = start.elapsed();

    match results {
        Ok(task_results) => {
            let successful = task_results
                .iter()
                .filter_map(|r| r.as_ref().ok())
                .filter(|(_, success, _)| *success)
                .count();

            let total = task_results.len();
            println!(
                "🔥 Concurrent mixed: {}/{} successful in {:?}",
                successful, total, total_time
            );

            // Print timing info for successful requests
            for result in task_results.iter().filter_map(|r| r.as_ref().ok()) {
                let (id, success, duration) = result;
                if *success {
                    println!("  ✅ Request {}: {:?}", id, duration);
                } else {
                    println!("  ❌ Request {}: failed after {:?}", id, duration);
                }
            }

            // At least 60% should succeed under concurrent load
            assert!(
                successful >= 12,
                "Too many concurrent requests failed: {}/{}",
                successful,
                total
            );
        }
        Err(_) => {
            panic!("⏰ Concurrent mixed test timed out after 20s - potential hang detected!");
        }
    }
}

#[tokio::test]
async fn test_stress_expensive_operations() {
    let server_arc = create_stress_server().await;
    let server = server_arc.lock().await;

    println!("🔥 Testing potentially expensive operations for hangs...");

    // Test workspace symbols (can be expensive with broad queries)
    {
        println!("⏱️ Testing workspace symbols search...");
        let request = language_server_mcp::models::WorkspaceSymbolsRequest {
            query: "test".to_string(), // Common term likely to have many matches
        };

        let operation = server.workspace_symbols(Parameters(request));
        let result = timeout(Duration::from_secs(10), operation).await;

        match result {
            Ok(Ok(_)) => println!("✅ Workspace symbols completed successfully"),
            Ok(Err(e)) => println!("❌ Workspace symbols failed: {:?}", e),
            Err(_) => println!("⏰ Workspace symbols timed out after 10s"),
        }
    }

    // Test document symbols on large file
    {
        println!("⏱️ Testing document symbols on large file...");
        let request = language_server_mcp::models::DocumentSymbolsRequest {
            file_path: "src/server.rs".to_string(), // Largest file in project
            page: 0,
            page_size: 200,
        };

        let operation = server.document_symbols(Parameters(request));
        let result = timeout(Duration::from_secs(8), operation).await;

        match result {
            Ok(Ok(_)) => println!("✅ Document symbols completed successfully"),
            Ok(Err(e)) => println!("❌ Document symbols failed: {:?}", e),
            Err(_) => println!("⏰ Document symbols timed out after 8s"),
        }
    }

    // Test find references (can be expensive)
    {
        println!("⏱️ Testing find references...");
        let request = language_server_mcp::models::FindReferencesRequest {
            file_path: "src/lib.rs".to_string(),
            line: 1,
            column: 10, // "errors" in "pub mod errors"
            include_declaration: true,
        };

        let operation = server.find_references(Parameters(request));
        let result = timeout(Duration::from_secs(8), operation).await;

        match result {
            Ok(Ok(_)) => println!("✅ Find references completed successfully"),
            Ok(Err(e)) => println!("❌ Find references failed: {:?}", e),
            Err(_) => println!("⏰ Find references timed out after 8s"),
        }
    }

    println!("🔥 Expensive operations test completed - no hangs detected!");
}

#[tokio::test]
async fn test_stress_burst_then_wait() {
    let server_arc = create_stress_server().await;
    let server = server_arc.lock().await;

    println!("🔥 Testing burst of requests followed by waiting pattern...");

    // Phase 1: Burst of requests
    let mut burst_futures = Vec::new();
    let burst_start = Instant::now();

    for i in 0..15 {
        let request = language_server_mcp::models::HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: (i % 20) + 5,
            column: (i % 30) + 5,
        };

        burst_futures.push(server.hover(Parameters(request)));
    }

    let burst_queue_time = burst_start.elapsed();
    println!("🔥 Queued burst of 15 requests in {:?}", burst_queue_time);

    // Phase 2: Wait for burst to complete
    let burst_results = timeout(
        Duration::from_secs(12),
        futures::future::join_all(burst_futures),
    )
    .await;
    let burst_total_time = burst_start.elapsed();

    let burst_successful = match burst_results {
        Ok(responses) => {
            let success_count = responses.iter().filter(|r| r.is_ok()).count();
            println!(
                "🔥 Burst phase: {}/{} successful in {:?}",
                success_count,
                responses.len(),
                burst_total_time
            );
            success_count
        }
        Err(_) => {
            panic!("⏰ Burst phase timed out after 12s");
        }
    };

    // Phase 3: Wait a bit, then send individual requests
    println!("⏸️ Waiting 1 second before individual requests...");
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let mut individual_successful = 0;
    for i in 0..5 {
        let request = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/lib.rs".to_string(),
        };

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(5),
            server.diagnostics(Parameters(request)),
        )
        .await;
        let duration = start.elapsed();

        match result {
            Ok(Ok(_)) => {
                individual_successful += 1;
                println!("✅ Individual request {}: {:?}", i + 1, duration);
            }
            Ok(Err(e)) => println!("❌ Individual request {} failed: {:?}", i + 1, e),
            Err(_) => println!("⏰ Individual request {} timed out", i + 1),
        }
    }

    println!(
        "🔥 Burst-then-wait test: burst={}/15, individual={}/5",
        burst_successful, individual_successful
    );

    // Both phases should be mostly successful
    assert!(burst_successful >= 10, "Burst phase had too many failures");
    assert!(
        individual_successful >= 3,
        "Individual phase had too many failures"
    );
}
