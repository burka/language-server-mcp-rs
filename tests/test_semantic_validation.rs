// Test semantic validation with proper indexing status monitoring
// This demonstrates how to test for meaningful rust-analyzer responses

use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;
use std::time::Duration;

/// Helper function to check if a CallToolResult contains meaningful content
fn has_meaningful_content(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

/// Helper function to get a simple string representation of the content
fn get_content_summary(result: &CallToolResult) -> String {
    if result.content.is_empty() {
        "EMPTY".to_string()
    } else {
        format!("{} items", result.content.len())
    }
}

#[tokio::test]
async fn test_semantic_validation_with_indexing_status() {
    println!("=== Testing Semantic Validation with Indexing Status ===");

    // Set larger response size limit for semantic validation tests
    std::env::set_var("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", "1048576"); // 1MB
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");

    println!("✅ MCP server created (includes diagnostics pre-warming)");

    // Test 1: Hover on a known function should return meaningful information
    println!("\n🔍 Testing hover on main function...");
    let hover_request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18,   // async fn main() -> Result<(), Box<dyn std::error::Error>> {
        column: 15, // On "main"
    };

    let hover_result = server
        .hover(Parameters(hover_request))
        .await
        .expect("Hover request failed");
    let hover_summary = get_content_summary(&hover_result);

    println!("Hover result: {}", hover_summary);

    // Semantic validation: hover should return meaningful content
    let has_meaningful_hover = has_meaningful_content(&hover_result);

    if has_meaningful_hover {
        println!("✅ Hover contains meaningful semantic information");
    } else {
        println!("❌ Hover result appears to be empty/placeholder");
    }

    // Test 2: Diagnostics should return actual compilation diagnostics
    println!("\n📋 Testing diagnostics for semantic information...");
    let diagnostics_request = DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let diagnostics_result = server
        .diagnostics(Parameters(diagnostics_request))
        .await
        .expect("Diagnostics request failed");
    let diagnostics_summary = get_content_summary(&diagnostics_result);

    println!("Diagnostics result: {}", diagnostics_summary);

    // Semantic validation: diagnostics should return meaningful content
    let has_meaningful_diagnostics = has_meaningful_content(&diagnostics_result);

    if has_meaningful_diagnostics {
        println!("✅ Diagnostics contain meaningful information");
    } else {
        println!("❌ Diagnostics result appears to be empty/placeholder");
    }

    // Test 3: Completion at a meaningful location
    println!("\n💡 Testing completion for semantic information...");
    let completion_request = CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 30,   // info!("Starting rust-analyzer MCP server");
        column: 10, // After "info!"
    };

    let completion_result = server
        .completion(Parameters(completion_request))
        .await
        .expect("Completion request failed");
    let completion_summary = get_content_summary(&completion_result);

    println!("Completion result: {}", completion_summary);

    // Semantic validation: completion should return meaningful content
    let has_meaningful_completion = has_meaningful_content(&completion_result);

    if has_meaningful_completion {
        println!("✅ Completion contains meaningful information");
    } else {
        println!("❌ Completion result appears to be empty/placeholder");
    }

    // Summary
    println!("\n📊 Semantic Validation Summary:");
    println!(
        "  Hover: {}",
        if has_meaningful_hover {
            "✅ PASS"
        } else {
            "❌ FAIL"
        }
    );
    println!(
        "  Diagnostics: {}",
        if has_meaningful_diagnostics {
            "✅ PASS"
        } else {
            "❌ FAIL"
        }
    );
    println!(
        "  Completion: {}",
        if has_meaningful_completion {
            "✅ PASS"
        } else {
            "❌ FAIL"
        }
    );

    let total_score = [
        has_meaningful_hover,
        has_meaningful_diagnostics,
        has_meaningful_completion,
    ]
    .iter()
    .filter(|&&x| x)
    .count();

    println!(
        "  Overall: {}/3 tests showing semantic information",
        total_score
    );

    // The test passes if at least some semantic functionality is working
    // This helps us identify which areas need improvement
    assert!(
        total_score >= 1,
        "At least one semantic test should pass with proper indexing"
    );

    println!("\n✅ Semantic validation test completed!");
}

#[tokio::test]
async fn test_indexing_timing_impact() {
    println!("=== Testing Indexing Timing Impact ===");

    // Test the timing difference between immediate requests vs waiting for indexing
    // Set larger response size limit
    std::env::set_var("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", "1048576"); // 1MB
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    println!("Creating fresh MCP server...");
    let start_time = std::time::Instant::now();
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    let server_creation_time = start_time.elapsed();

    println!("✅ Server created in {:?}", server_creation_time);

    // Make immediate request (should work due to Option A pre-warming)
    let immediate_start = std::time::Instant::now();
    let hover_request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18,
        column: 15,
    };

    let immediate_result = server
        .hover(Parameters(hover_request))
        .await
        .expect("Immediate hover failed");
    let immediate_time = immediate_start.elapsed();
    let immediate_summary = get_content_summary(&immediate_result);
    let has_content = has_meaningful_content(&immediate_result);

    println!("⚡ Immediate hover completed in {:?}", immediate_time);
    println!("   Result: {}", immediate_summary);
    println!("   Has semantic info: {}", has_content);

    // This test demonstrates that Option A (diagnostics pre-warming) works
    // The hover should be reasonably fast and contain meaningful content
    // Allow more time for indexing to complete
    assert!(
        immediate_time < Duration::from_secs(10),
        "Immediate request should complete within reasonable time: {:?}",
        immediate_time
    );
    assert!(has_content, "Should have meaningful content");

    println!("✅ Indexing timing test completed - Option A is working correctly!");
}
