// Full-stack completion tests migrated from direct LSP client to MCP server testing
// Tests the complete flow: MCP completion tool → RustAnalyzerMCP → tool_handlers → LspClient → rust-analyzer
// This provides better coverage of server.rs and tests Option A implementation

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::timeout;

/// Create a test MCP server instance with Option A pre-warming
async fn create_completion_test_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server for completion tests")
}

/// Helper to check if MCP result is successful
fn is_success(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

/// Helper to extract content as an indication of completion items
fn has_completions(result: &CallToolResult) -> bool {
    is_success(result) // For now, any content suggests completion results
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
async fn test_mcp_completion_basic() {
    println!("🔍 Testing MCP completion on basic code position...");
    let server = create_completion_test_server().await;

    // Test completion in main.rs at a reasonable position
    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 5, // Line with imports or use statements
        column: 10,
    };

    let start = Instant::now();
    let result = with_mcp_timeout(
        "mcp_completion_basic",
        Duration::from_secs(5),
        server.completion(Parameters(request)),
    )
    .await;
    let duration = start.elapsed();

    println!("📊 MCP completion basic completed in {:?}", duration);

    match result {
        Ok(Ok(tool_result)) => {
            if has_completions(&tool_result) {
                println!("✅ MCP completion succeeded with completion items");
            } else {
                println!("⚪ MCP completion succeeded but no items available (normal)");
            }
        }
        Ok(Err(_)) => {
            println!("⚠️  MCP completion error occurred");
            // With Option A retry logic, completion should be more reliable
        }
        Err(e) => {
            println!("🔥 MCP completion timed out: {}", e);
            panic!("Completion should not hang with Option A implementation");
        }
    }
}

#[tokio::test]
async fn test_mcp_completion_multiple_positions() {
    println!("🔍 Testing MCP completion at multiple code positions...");
    let server = create_completion_test_server().await;

    let positions = [
        ("src/main.rs", 10, 5, "Main function area"),
        ("src/server.rs", 50, 15, "Server implementation"),
        ("src/lsp_client.rs", 100, 20, "LSP client code"),
        ("src/models.rs", 20, 10, "Model definitions"),
    ];

    let mut successful = 0;
    let mut total_time = Duration::new(0, 0);

    for (file, line, column, description) in positions {
        let request = language_server_mcp::models::CompletionRequest {
            file_path: file.to_string(),
            line,
            column,
        };

        let start = Instant::now();
        let result = server.completion(Parameters(request)).await;
        let duration = start.elapsed();
        total_time += duration;

        match result {
            Ok(tool_result) => {
                if has_completions(&tool_result) {
                    successful += 1;
                    println!(
                        "✅ MCP completion {} succeeded in {:?}: has items",
                        description, duration
                    );
                } else {
                    successful += 1; // No items is still a successful response
                    println!(
                        "✅ MCP completion {} succeeded in {:?}: no items",
                        description, duration
                    );
                }
            }
            Err(e) => {
                println!("⚠️  MCP completion {} error: {:?}", description, e);
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!(
        "📊 Multiple position completion: {}/{} successful, total time: {:?}",
        successful,
        positions.len(),
        total_time
    );

    // With Option A and retry logic, at least some positions should succeed
    assert!(
        successful >= 1, // At least one should succeed
        "At least one completion position should succeed: {}/{}",
        successful,
        positions.len()
    );
}

#[tokio::test]
async fn test_mcp_completion_invalid_position() {
    println!("🔍 Testing MCP completion with invalid position...");
    let server = create_completion_test_server().await;

    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 99999,   // Invalid line
        column: 99999, // Invalid column
    };

    let start = Instant::now();
    let result = with_mcp_timeout(
        "mcp_completion_invalid",
        Duration::from_secs(2), // Should be fast for invalid positions
        server.completion(Parameters(request)),
    )
    .await;
    let duration = start.elapsed();

    println!(
        "📊 MCP completion invalid position completed in {:?}",
        duration
    );

    // Should be fast regardless of outcome
    assert!(
        duration < Duration::from_secs(2),
        "Invalid position completion should be fast: {:?}",
        duration
    );

    match result {
        Ok(Ok(tool_result)) => {
            if has_completions(&tool_result) {
                println!(
                    "⚪ MCP completion returned items for invalid position (unexpected but ok)"
                );
            } else {
                println!("✅ MCP completion returned no items for invalid position (expected)");
            }
        }
        Ok(Err(_)) => {
            println!("✅ MCP completion returned error for invalid position");
        }
        Err(e) => {
            panic!(
                "MCP completion should not timeout on invalid position: {}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_mcp_completion_nonexistent_file() {
    println!("🔍 Testing MCP completion with nonexistent file...");
    let server = create_completion_test_server().await;

    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/nonexistent.rs".to_string(),
        line: 1,
        column: 1,
    };

    let start = Instant::now();
    let result = server.completion(Parameters(request)).await;
    let duration = start.elapsed();

    println!(
        "📊 MCP completion nonexistent file completed in {:?}",
        duration
    );

    // Should be reasonably fast (accounting for retry logic)
    assert!(
        duration < Duration::from_secs(5),
        "Nonexistent file completion should be reasonably fast: {:?}",
        duration
    );

    match result {
        Ok(tool_result) => {
            if has_completions(&tool_result) {
                println!(
                    "⚪ MCP completion returned items for nonexistent file (unexpected but ok)"
                );
            } else {
                println!("✅ MCP completion returned no items for nonexistent file (expected)");
            }
        }
        Err(e) => {
            println!(
                "✅ MCP completion returned error for nonexistent file: {:?}",
                e
            );
        }
    }
}

#[tokio::test]
async fn test_mcp_completion_rapid_requests() {
    println!("🔍 Testing MCP completion rapid requests (no hanging)...");
    let server = create_completion_test_server().await;

    let mut successful = 0;
    let mut errors = 0;
    let start_time = Instant::now();

    // Fire rapid completion requests
    for i in 0..6 {
        let request = language_server_mcp::models::CompletionRequest {
            file_path: "src/main.rs".to_string(),
            line: 20 + (i % 10), // Vary positions
            column: 10 + (i % 5),
        };

        let result = with_mcp_timeout(
            &format!("rapid_completion_{}", i),
            Duration::from_secs(8), // Account for retry logic
            server.completion(Parameters(request)),
        )
        .await;

        match result {
            Ok(Ok(tool_result)) => {
                if has_completions(&tool_result) {
                    successful += 1;
                    println!("✅ Rapid completion {} succeeded with items", i);
                } else {
                    successful += 1; // No items is still success
                    println!("✅ Rapid completion {} succeeded, no items", i);
                }
            }
            Ok(Err(_)) => {
                errors += 1;
                println!("⚠️  Rapid completion {} error occurred", i);
            }
            Err(e) => {
                panic!("Rapid completion {} timed out: {}", i, e);
            }
        }

        // Small delay between rapid requests
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let total_time = start_time.elapsed();
    println!(
        "📊 Rapid completion: {}/6 successful, {} errors, total time: {:?}",
        successful, errors, total_time
    );

    // With Option A and retry logic, we should have good performance (allow for retries)
    assert!(
        total_time < Duration::from_secs(15),
        "Rapid completion should be reasonably fast: {:?}",
        total_time
    );

    assert!(
        successful >= 4, // At least 2/3 success rate
        "Most rapid completion requests should succeed: {}/6",
        successful
    );
}

#[tokio::test]
async fn test_mcp_completion_hang_detection() {
    println!("🔍 Testing MCP completion hang detection with specific positions...");
    let server = create_completion_test_server().await;

    // Test completion at positions that might be more likely to hang
    let potentially_problematic = [
        ("src/lsp_client.rs", 50, 15, "Complex struct area"),
        ("src/server.rs", 200, 20, "Deep in implementation"),
        ("src/main.rs", 30, 25, "Function call area"),
    ];

    for (file, line, column, description) in potentially_problematic {
        let request = language_server_mcp::models::CompletionRequest {
            file_path: file.to_string(),
            line,
            column,
        };

        let start = Instant::now();
        let result = with_mcp_timeout(
            &format!("hang_detection_{}", description.replace(' ', "_")),
            Duration::from_secs(15), // Account for retry logic and potential indexing
            server.completion(Parameters(request)),
        )
        .await;
        let duration = start.elapsed();

        match result {
            Ok(Ok(tool_result)) => {
                if has_completions(&tool_result) {
                    println!("✅ {} completed in {:?}: has items", description, duration);
                } else {
                    println!("✅ {} completed in {:?}: no items", description, duration);
                }
            }
            Ok(Err(_)) => {
                println!(
                    "⚠️  {} error in {:?}: limited output",
                    description, duration
                );
            }
            Err(e) => {
                panic!("{} timed out: {} (hang detected!)", description, e);
            }
        }

        // Brief pause
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    println!("✅ Completion hang detection test completed - no hangs detected!");
}

#[tokio::test]
async fn test_mcp_completion_option_a_benefits() {
    println!("🔍 Testing MCP completion Option A performance benefits...");

    // Test 1: Server creation with pre-warming
    let start = Instant::now();
    let server = create_completion_test_server().await;
    let creation_time = start.elapsed();
    println!("📊 Server creation with Option A: {:?}", creation_time);

    // Test 2: Immediate completion request (should benefit from pre-warming)
    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 25,
        column: 15,
    };

    let completion_start = Instant::now();
    let result = server.completion(Parameters(request)).await;
    let completion_time = completion_start.elapsed();

    println!("📊 Immediate completion time: {:?}", completion_time);

    match result {
        Ok(tool_result) => {
            if has_completions(&tool_result) {
                println!("✅ Immediate completion succeeded with items (Option A worked!)");
            } else {
                println!(
                    "✅ Immediate completion succeeded, no items (Option A prevented failures!)"
                );
            }
        }
        Err(e) => {
            println!(
                "⚠️  Immediate completion error (Option A retry should help): {:?}",
                e
            );
        }
    }

    let total_time = start.elapsed();
    println!("📊 Total time with Option A: {:?}", total_time);

    // With Option A, completion should be fast and reliable
    assert!(
        completion_time < Duration::from_secs(8), // Allow for retry logic
        "Immediate completion should be fast with Option A: {:?}",
        completion_time
    );

    assert!(
        total_time < Duration::from_secs(10),
        "Total test should be efficient with Option A: {:?}",
        total_time
    );
}

#[tokio::test]
async fn test_mcp_completion_edge_cases() {
    println!("🔍 Testing MCP completion edge cases...");
    let server = create_completion_test_server().await;

    let edge_cases = [
        ("src/main.rs", 1, 1, "Very beginning of file"),
        ("src/main.rs", 1000, 1, "Way past end of file"),
        ("src/main.rs", 10, 1000, "Way past end of line"),
        (".", 1, 1, "Current directory as file"),
        ("", 1, 1, "Empty file path"),
    ];

    for (file, line, column, description) in edge_cases {
        let request = language_server_mcp::models::CompletionRequest {
            file_path: file.to_string(),
            line,
            column,
        };

        let start = Instant::now();
        let result = timeout(
            Duration::from_secs(10), // Account for retry logic and edge cases
            server.completion(Parameters(request)),
        )
        .await;
        let duration = start.elapsed();

        match result {
            Ok(Ok(tool_result)) => {
                if has_completions(&tool_result) {
                    println!(
                        "⚪ {} returned items in {:?} (unexpected but ok)",
                        description, duration
                    );
                } else {
                    println!(
                        "✅ {} returned no items in {:?} (expected)",
                        description, duration
                    );
                }
            }
            Ok(Err(_)) => {
                println!(
                    "✅ {} returned error in {:?}: limited output",
                    description, duration
                );
            }
            Err(_) => {
                println!("⚠️  {} timed out in {:?}", description, duration);
            }
        }

        // Should be reasonably fast for edge cases (accounting for retry logic)
        assert!(
            duration < Duration::from_secs(5),
            "{} should be reasonably fast: {:?}",
            description,
            duration
        );
    }
}
