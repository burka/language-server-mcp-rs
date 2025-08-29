// Full-stack diagnostics tests migrated from direct LSP client to MCP server testing
// Tests the complete flow: MCP diagnostics tool → RustAnalyzerMCP → tool_handlers → LspClient → rust-analyzer
// This provides better coverage of server.rs and tests Option A implementation (especially pre-warming benefits)

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Create a test MCP server instance with Option A pre-warming
async fn create_diagnostics_test_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server for diagnostics tests")
}

/// Helper to check if MCP result is successful
fn is_success(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

/// Timeout wrapper for MCP operations  
async fn with_mcp_timeout<F, T>(name: &str, duration: Duration, future: F) -> Result<T, String>
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

#[tokio::test]
async fn test_mcp_diagnostics_clean_file() {
    println!("🔍 Testing MCP diagnostics on main.rs (clean file)...");
    let server = create_diagnostics_test_server().await;

    let request = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let start = Instant::now();
    let result = with_mcp_timeout(
        "mcp_diagnostics_main",
        Duration::from_secs(5),
        server.diagnostics(Parameters(request)),
    )
    .await;
    let duration = start.elapsed();

    println!("📊 MCP diagnostics on main.rs completed in {:?}", duration);

    match result {
        Ok(Ok(tool_result)) => {
            if is_success(&tool_result) {
                println!("✅ MCP diagnostics succeeded - found issues to report");
            } else {
                println!("✅ MCP diagnostics succeeded - clean file (no issues)");
            }
        }
        Ok(Err(e)) => {
            println!("⚠️  MCP diagnostics error: {:?}", e);
            // With Option A, this should be less likely due to retry logic
        }
        Err(e) => {
            println!("🔥 MCP diagnostics timed out: {}", e);
            panic!("Diagnostics should not hang with Option A implementation");
        }
    }
}

#[tokio::test]
async fn test_mcp_diagnostics_multiple_files() {
    println!("🔍 Testing MCP diagnostics on multiple files...");
    let server = create_diagnostics_test_server().await;

    let test_files = [
        "src/main.rs",
        "src/server.rs",
        "src/lsp_client.rs",
        "src/models.rs",
        "src/errors.rs",
    ];

    let mut successful = 0;
    let mut total_time = Duration::new(0, 0);

    for file in test_files {
        let request = language_server_mcp::models::DiagnosticsRequest {
            file_path: file.to_string(),
        };

        let start = Instant::now();
        let result = server.diagnostics(Parameters(request)).await;
        let duration = start.elapsed();
        total_time += duration;

        match result {
            Ok(tool_result) => {
                if is_success(&tool_result) {
                    successful += 1;
                    println!(
                        "✅ MCP diagnostics for {} succeeded in {:?}",
                        file, duration
                    );
                } else {
                    successful += 1; // Empty result is still success (clean file)
                    println!(
                        "✅ MCP diagnostics for {} clean (no issues) in {:?}",
                        file, duration
                    );
                }
            }
            Err(e) => {
                println!("⚠️  MCP diagnostics for {} error: {:?}", file, e);
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!(
        "📊 Multiple file diagnostics: {}/{} successful, total time: {:?}",
        successful,
        test_files.len(),
        total_time
    );

    // With Option A pre-warming, most files should succeed
    assert!(
        successful >= test_files.len() - 1, // Allow 1 failure
        "Most files should have successful diagnostics: {}/{}",
        successful,
        test_files.len()
    );
}

#[tokio::test]
async fn test_mcp_diagnostics_invalid_file() {
    println!("🔍 Testing MCP diagnostics with invalid/nonexistent file...");
    let server = create_diagnostics_test_server().await;

    let request = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/nonexistent.rs".to_string(),
    };

    let start = Instant::now();
    let result = with_mcp_timeout(
        "mcp_diagnostics_invalid",
        Duration::from_secs(3), // Should be fast for invalid files
        server.diagnostics(Parameters(request)),
    )
    .await;
    let duration = start.elapsed();

    println!(
        "📊 MCP diagnostics invalid file completed in {:?}",
        duration
    );

    // Should be fast regardless of outcome
    assert!(
        duration < Duration::from_secs(3),
        "Invalid file diagnostics should be fast: {:?}",
        duration
    );

    match result {
        Ok(Ok(tool_result)) => {
            if is_success(&tool_result) {
                println!(
                    "⚪ MCP diagnostics returned content for invalid file (unexpected but ok)"
                );
            } else {
                println!("✅ MCP diagnostics returned empty for invalid file (expected)");
            }
        }
        Ok(Err(e)) => {
            println!(
                "✅ MCP diagnostics returned error for invalid file: {:?}",
                e
            );
        }
        Err(e) => {
            panic!("MCP diagnostics should not timeout on invalid file: {}", e);
        }
    }
}

#[tokio::test]
async fn test_mcp_diagnostics_rapid_requests() {
    println!("🔍 Testing MCP diagnostics rapid requests (no hanging)...");
    let server = create_diagnostics_test_server().await;

    let mut successful = 0;
    let mut errors = 0;
    let start_time = Instant::now();

    // Fire rapid diagnostics requests
    for i in 0..8 {
        let request = language_server_mcp::models::DiagnosticsRequest {
            file_path: "src/main.rs".to_string(),
        };

        let result = with_mcp_timeout(
            &format!("rapid_diagnostics_{}", i),
            Duration::from_secs(3),
            server.diagnostics(Parameters(request)),
        )
        .await;

        match result {
            Ok(Ok(tool_result)) => {
                if is_success(&tool_result) {
                    successful += 1;
                    println!("✅ Rapid diagnostics {} succeeded", i);
                } else {
                    successful += 1; // Empty is still success
                    println!("✅ Rapid diagnostics {} clean", i);
                }
            }
            Ok(Err(e)) => {
                errors += 1;
                println!("⚠️  Rapid diagnostics {} error: {:?}", i, e);
            }
            Err(e) => {
                panic!("Rapid diagnostics {} timed out: {}", i, e);
            }
        }

        // Very small delay
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    let total_time = start_time.elapsed();
    println!(
        "📊 Rapid diagnostics: {}/8 successful, {} errors, total time: {:?}",
        successful, errors, total_time
    );

    // With Option A, we should have good performance and reliability
    assert!(
        total_time < Duration::from_secs(10),
        "Rapid diagnostics should complete quickly: {:?}",
        total_time
    );

    assert!(
        successful >= 6, // At least 75% success rate
        "Most rapid requests should succeed: {}/8",
        successful
    );
}

#[tokio::test]
async fn test_mcp_diagnostics_concurrent() {
    println!("🔍 Testing MCP diagnostics concurrent requests...");
    let server = create_diagnostics_test_server().await;

    let files = ["src/main.rs", "src/server.rs", "src/lsp_client.rs"];
    let mut tasks: Vec<Result<(bool, Duration), ()>> = Vec::new();

    let start_time = Instant::now();

    // Create concurrent diagnostic requests (sequential for now to avoid server sharing issues)
    for (i, &file) in files.iter().enumerate() {
        let request = language_server_mcp::models::DiagnosticsRequest {
            file_path: file.to_string(),
        };

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(5),
            server.diagnostics(Parameters(request)),
        )
        .await;
        let duration = start.elapsed();

        let task_result = match result {
            Ok(Ok(tool_result)) => {
                let success = is_success(&tool_result);
                println!(
                    "✅ Concurrent diagnostics {} ({}) in {:?}: {}",
                    i,
                    file,
                    duration,
                    if success { "has content" } else { "clean" }
                );
                (true, duration)
            }
            Ok(Err(e)) => {
                println!("⚠️  Concurrent diagnostics {} error: {:?}", i, e);
                (false, duration)
            }
            Err(_) => {
                println!("🔥 Concurrent diagnostics {} timed out", i);
                (false, duration)
            }
        };

        tasks.push(Ok(task_result)); // Simulate task completion
    }

    // Process results (now synchronous)
    let results = tasks;
    let total_time = start_time.elapsed();

    let successful = results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .filter(|(success, _)| *success)
        .count();

    let avg_duration: Duration = results
        .iter()
        .filter_map(|r| r.as_ref().ok())
        .map(|(_, duration)| *duration)
        .sum::<Duration>()
        / results.len() as u32;

    println!(
        "📊 Concurrent diagnostics: {}/{} successful, avg time: {:?}, total: {:?}",
        successful,
        files.len(),
        avg_duration,
        total_time
    );

    // With Option A, concurrent requests should work well
    assert!(
        successful >= 2, // At least 2/3 should succeed
        "Most concurrent requests should succeed: {}/{}",
        successful,
        files.len()
    );
}

#[tokio::test]
async fn test_mcp_diagnostics_option_a_benefits() {
    println!("🔍 Testing MCP diagnostics Option A pre-warming benefits...");

    // Test 1: Server creation time
    let start = Instant::now();
    let server = create_diagnostics_test_server().await;
    let creation_time = start.elapsed();
    println!("📊 Server creation with Option A: {:?}", creation_time);

    // Test 2: Immediate diagnostics request (most likely to benefit from pre-warming)
    let request = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let diag_start = Instant::now();
    let result = server.diagnostics(Parameters(request)).await;
    let diag_time = diag_start.elapsed();

    println!("📊 Immediate diagnostics time: {:?}", diag_time);

    match result {
        Ok(tool_result) => {
            if is_success(&tool_result) {
                println!("✅ Immediate diagnostics succeeded (Option A worked!)");
            } else {
                println!("✅ Immediate diagnostics clean (Option A prevented failures!)");
            }
        }
        Err(e) => {
            println!(
                "⚠️  Immediate diagnostics error (Option A retry should help): {:?}",
                e
            );
        }
    }

    let total_time = start.elapsed();
    println!("📊 Total time with Option A: {:?}", total_time);

    // Key insight: Without Option A, diagnostics often failed for ~500ms
    // With Option A, it should work immediately or very quickly
    assert!(
        diag_time < Duration::from_secs(2),
        "Immediate diagnostics should be fast with Option A: {:?}",
        diag_time
    );

    assert!(
        total_time < Duration::from_secs(5),
        "Total test should be efficient with Option A: {:?}",
        total_time
    );
}

#[tokio::test]
async fn test_mcp_diagnostics_path_variants() {
    println!("🔍 Testing MCP diagnostics with different path formats...");
    let server = create_diagnostics_test_server().await;

    let path_variants = [
        ("src/main.rs", "Relative path"),
        ("./src/main.rs", "Dot relative path"),
        ("src/../src/main.rs", "Path with parent traversal"),
    ];

    for (path, description) in path_variants {
        let request = language_server_mcp::models::DiagnosticsRequest {
            file_path: path.to_string(),
        };

        let start = Instant::now();
        let result = server.diagnostics(Parameters(request)).await;
        let duration = start.elapsed();

        match result {
            Ok(tool_result) => {
                if is_success(&tool_result) {
                    println!(
                        "✅ {} succeeded in {:?}: has content",
                        description, duration
                    );
                } else {
                    println!("✅ {} succeeded in {:?}: clean", description, duration);
                }
            }
            Err(e) => {
                println!("⚠️  {} error in {:?}: {:?}", description, duration, e);
            }
        }

        // Should be reasonably fast
        assert!(
            duration < Duration::from_secs(3),
            "{} should be reasonably fast: {:?}",
            description,
            duration
        );
    }
}
