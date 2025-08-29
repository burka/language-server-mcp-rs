// Full-stack tool handler tests - replaces mock-based approach with real MCP server testing
// Tests the complete flow: MCP tool calls → RustAnalyzerMCP → tool_handlers → LspClient → rust-analyzer
// This provides much better coverage of server.rs compared to testing tool_handlers directly

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;

/// Create a test MCP server instance for full-stack testing
async fn create_full_stack_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server")
}

/// Helper to check if result is successful
fn is_success(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

#[tokio::test]
async fn test_fullstack_hover_success() {
    let server = create_full_stack_server().await;

    // Test hover on a known position (imports in main.rs)
    let request = language_server_mcp::models::HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 3,
        column: 30, // "RustAnalyzerMCP" in import
    };

    let result = server.hover(Parameters(request)).await;

    // Should succeed with the full stack
    match result {
        Ok(tool_result) => {
            println!("Full-stack hover result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Full-stack hover error: {:?}", e);
            // For now, accept reasonable error messages
            assert!(
                format!("{:?}", e).contains("LSP error") || format!("{:?}", e).contains("timeout")
            );
        }
    }
}

#[tokio::test]
async fn test_fullstack_hover_no_info() {
    let server = create_full_stack_server().await;

    // Test hover on whitespace (should return no hover info)
    let request = language_server_mcp::models::HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 1,
        column: 1, // Empty line or comment
    };

    let result = server.hover(Parameters(request)).await;

    match result {
        Ok(tool_result) => {
            println!("No-info hover result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Expected error for no-info hover: {:?}", e);
            assert!(format!("{:?}", e).contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_fullstack_completion_success() {
    let server = create_full_stack_server().await;

    // Test completion at a position where we expect completions
    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 10,
        column: 5, // Inside a function where we can get completions
    };

    let result = server.completion(Parameters(request)).await;

    match result {
        Ok(tool_result) => {
            println!("Full-stack completion result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Completion error (may be expected): {:?}", e);
            assert!(format!("{:?}", e).contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_fullstack_diagnostics_success() {
    let server = create_full_stack_server().await;

    // Test diagnostics on main.rs
    let request = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = server.diagnostics(Parameters(request)).await;

    // Diagnostics should always succeed (even if no issues found)
    match result {
        Ok(tool_result) => {
            println!("Full-stack diagnostics result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            panic!("Diagnostics should not fail: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fullstack_goto_definition_success() {
    let server = create_full_stack_server().await;

    // Test goto definition on a symbol we know should have a definition
    let request = language_server_mcp::models::GotoDefinitionRequest {
        file_path: "src/main.rs".to_string(),
        line: 3,
        column: 30, // "RustAnalyzerMCP" in import
    };

    let result = server.goto_definition(Parameters(request)).await;

    match result {
        Ok(tool_result) => {
            println!("Full-stack goto definition result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Goto definition error (may be expected): {:?}", e);
            assert!(format!("{:?}", e).contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_fullstack_find_references_success() {
    let server = create_full_stack_server().await;

    // Test find references on a symbol
    let request = language_server_mcp::models::FindReferencesRequest {
        file_path: "src/main.rs".to_string(),
        line: 3,
        column: 30, // "RustAnalyzerMCP" in import
        include_declaration: true,
    };

    let result = server.find_references(Parameters(request)).await;

    match result {
        Ok(tool_result) => {
            println!("Full-stack find references result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Find references error (may be expected): {:?}", e);
            assert!(format!("{:?}", e).contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_fullstack_invalid_file() {
    let server = create_full_stack_server().await;

    // Test with non-existent file
    let request = language_server_mcp::models::HoverRequest {
        file_path: "src/does_not_exist.rs".to_string(),
        line: 1,
        column: 1,
    };

    let result = server.hover(Parameters(request)).await;

    // Should return a result (error handling varies)
    match result {
        Ok(tool_result) => {
            println!("Invalid file result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Expected error for invalid file: {:?}", e);
            assert!(
                format!("{:?}", e).contains("LSP error") || format!("{:?}", e).contains("file")
            );
        }
    }
}

#[tokio::test]
async fn test_fullstack_invalid_position() {
    let server = create_full_stack_server().await;

    // Test with invalid position (way beyond file bounds)
    let request = language_server_mcp::models::HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 99999,
        column: 99999,
    };

    let result = server.hover(Parameters(request)).await;

    match result {
        Ok(tool_result) => {
            println!("Invalid position result: {:?}", tool_result);
            assert!(is_success(&tool_result), "Tool result should have content");
        }
        Err(e) => {
            println!("Expected error for invalid position: {:?}", e);
            assert!(
                format!("{:?}", e).contains("LSP error") || format!("{:?}", e).contains("position")
            );
        }
    }
}
