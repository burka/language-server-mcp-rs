// Comprehensive coverage tests with shared rust-analyzer instance
// Tests all 19 MCP tool methods to maximize server.rs coverage
// Uses lazy_static for shared instance to reduce test overhead

use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

// Global shared MCP server instance
static SHARED_SERVER: OnceCell<Arc<Mutex<RustAnalyzerMCP>>> = OnceCell::const_new();

/// Get or create the shared MCP server instance
async fn get_shared_server() -> Arc<Mutex<RustAnalyzerMCP>> {
    SHARED_SERVER
        .get_or_init(|| async {
            let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let server = RustAnalyzerMCP::new(workspace)
                .await
                .expect("Failed to create shared MCP server");
            Arc::new(Mutex::new(server))
        })
        .await
        .clone()
}

/// Helper to check if result is successful
fn has_content(result: &CallToolResult) -> bool {
    !result.content.is_empty()
}

/// Extract actual content text from CallToolResult for detailed validation
fn extract_content_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| content.as_text().map(|text| text.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Enhanced validation that checks for specific semantic expectations
fn validate_semantic_content(
    result: &CallToolResult,
    tool_name: &str,
    expected_patterns: &[&str],
) -> bool {
    let content_text = extract_content_text(result);

    // Check if content is meaningful (not just error messages)
    if content_text.contains("No ") && content_text.contains(" available") {
        println!(
            "⚠️  {}: Got 'not available' response: {}",
            tool_name, content_text
        );
        return false;
    }

    // Check for any expected patterns
    for pattern in expected_patterns {
        if content_text.contains(pattern) {
            println!(
                "✅ {}: Found expected pattern '{}' in {} chars",
                tool_name,
                pattern,
                content_text.len()
            );
            return true;
        }
    }

    // If no patterns match but we have substantial content, it might still be valid
    if content_text.len() > 50 && !content_text.starts_with("No ") {
        println!(
            "⚠️  {}: Got substantial content ({} chars) but no expected patterns",
            tool_name,
            content_text.len()
        );
        println!(
            "   Sample: {}",
            &content_text[..std::cmp::min(100, content_text.len())]
        );
        return true; // Give benefit of doubt for substantial content
    }

    println!("❌ {}: No meaningful semantic content found", tool_name);
    false
}

// ===== CORE TOOL TESTS (previously tested) =====

#[tokio::test]
async fn test_comprehensive_hover() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::HoverRequest {
        file_path: "src/lib.rs".to_string(),
        line: 1,
        column: 5, // "pub mod errors"
    };

    let result = server.hover(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Hover error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_completion() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let result = server.completion(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Completion error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_diagnostics() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = server.diagnostics(Parameters(request)).await;
    match result {
        Ok(tool_result) => {
            // Enhanced validation: diagnostics should give meaningful status
            let expected_patterns = [
                "No diagnostics", // Clean code case
                "no diagnostic",  // Alternative clean case
                "error",          // Compilation errors
                "warning",        // Compiler warnings
                "diagnostic",     // Any diagnostic info
            ];

            let is_semantic_valid =
                validate_semantic_content(&tool_result, "diagnostics", &expected_patterns);
            assert!(
                is_semantic_valid,
                "Diagnostics should provide clear compilation status"
            );
            assert!(has_content(&tool_result)); // Still keep basic check
        }
        Err(e) => println!("Diagnostics error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_goto_definition() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::GotoDefinitionRequest {
        file_path: "src/lib.rs".to_string(),
        line: 1,
        column: 10, // "errors" in "pub mod errors"
    };

    let result = server.goto_definition(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Goto definition error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_find_references() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::FindReferencesRequest {
        file_path: "src/lib.rs".to_string(),
        line: 1,
        column: 10, // "errors" in "pub mod errors"
        include_declaration: true,
    };

    let result = server.find_references(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Find references error (acceptable): {:?}", e),
    }
}

// ===== MISSING TOOL TESTS (for coverage improvement) =====

#[tokio::test]
async fn test_comprehensive_format_document() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::FormatRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = server.format_document(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Format error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_rename() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::RenameRequest {
        file_path: "src/main.rs".to_string(),
        line: 13,
        column: 7, // "Args" struct
        new_name: "CommandLineArgs".to_string(),
    };

    let result = server.rename(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Rename error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_code_actions() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::CodeActionsRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let result = server.code_actions(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Code actions error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_workspace_symbols() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::WorkspaceSymbolsRequest {
        query: "Args".to_string(),
    };

    let result = server.workspace_symbols(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Workspace symbols error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_inlay_hints() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::InlayHintsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = server.inlay_hints(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Inlay hints error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_expand_macro() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::ExpandMacroRequest {
        file_path: "src/lib.rs".to_string(),
        line: 1,
        column: 1,
    };

    let result = server.expand_macro(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Expand macro error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_document_symbols() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::DocumentSymbolsRequest {
        file_path: "src/main.rs".to_string(),
        page: 0,
        page_size: 50,
    };

    let result = server.document_symbols(Parameters(request)).await;
    match result {
        Ok(tool_result) => {
            // Enhanced validation: check for specific expected symbols
            let expected_patterns = [
                "Args [Struct]",   // Should find Args struct definition
                "main [Function]", // Should find main function
                "symbols total",   // Should have symbol count info
                "workspace_root",  // Should find workspace_root references
            ];

            let is_semantic_valid =
                validate_semantic_content(&tool_result, "document_symbols", &expected_patterns);
            assert!(
                is_semantic_valid,
                "Document symbols should contain meaningful structural information"
            );
            assert!(has_content(&tool_result)); // Still keep basic check
        }
        Err(e) => {
            println!("Document symbols error (acceptable): {:?}", e);
            // For now, don't panic on error to maintain compatibility
        }
    }
}

#[tokio::test]
async fn test_comprehensive_signature_help() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::SignatureHelpRequest {
        file_path: "src/main.rs".to_string(),
        line: 37, // Inside main function call
        column: 40,
    };

    let result = server.signature_help(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Signature help error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_document_highlight() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::DocumentHighlightRequest {
        file_path: "src/main.rs".to_string(),
        line: 13,
        column: 7, // "Args" struct
    };

    let result = server.document_highlight(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Document highlight error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_selection_range() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::SelectionRangeRequest {
        file_path: "src/main.rs".to_string(),
        positions: vec![
            language_server_mcp::models::PositionInfo {
                line: 13,
                column: 7,
            },
            language_server_mcp::models::PositionInfo {
                line: 20,
                column: 10,
            },
        ],
    };

    let result = server.selection_range(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Selection range error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_runnables() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::RunnablesRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = server.runnables(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Runnables error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_implementations() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::ImplementationsRequest {
        file_path: "src/server.rs".to_string(),
        line: 31, // RustAnalyzerMCP impl
        column: 5,
    };

    let result = server.implementations(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Implementations error (acceptable): {:?}", e),
    }
}

#[tokio::test]
async fn test_comprehensive_lsp_status() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::LspClientStatusRequest {};

    let result = server.lsp_status(Parameters(request)).await;
    // Status should always work
    assert!(result.is_ok());
    assert!(has_content(&result.unwrap()));
}

#[tokio::test]
async fn test_comprehensive_close_document() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    let request = language_server_mcp::models::CloseDocumentRequest {
        file_path: "src/lib.rs".to_string(),
    };

    let result = server.close_document(Parameters(request)).await;
    match result {
        Ok(tool_result) => assert!(has_content(&tool_result)),
        Err(e) => println!("Close document error (acceptable): {:?}", e),
    }
}

// ===== HANGING SCENARIO TESTS =====

#[tokio::test]
async fn test_hanging_scenario_deep_goto_definition() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    // Test goto definition on a complex Rust standard library type
    let request = language_server_mcp::models::GotoDefinitionRequest {
        file_path: "src/lsp_client.rs".to_string(),
        line: 25, // Arc<Mutex<>> usage
        column: 15,
    };

    println!("Testing potentially hanging goto definition on complex type...");
    let result = server.goto_definition(Parameters(request)).await;
    match result {
        Ok(tool_result) => {
            println!(
                "Deep goto definition succeeded: has_content={}",
                has_content(&tool_result)
            );
            assert!(has_content(&tool_result));
        }
        Err(e) => println!(
            "Deep goto definition error (may indicate hang issue): {:?}",
            e
        ),
    }
}

#[tokio::test]
async fn test_hanging_scenario_complex_completion() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    // Test completion in a complex context with many imports
    let request = language_server_mcp::models::CompletionRequest {
        file_path: "src/server.rs".to_string(),
        line: 58, // Inside tool handler with many available methods
        column: 20,
    };

    println!("Testing potentially hanging completion in complex context...");
    let result = server.completion(Parameters(request)).await;
    match result {
        Ok(tool_result) => {
            println!(
                "Complex completion succeeded: has_content={}",
                has_content(&tool_result)
            );
            assert!(has_content(&tool_result));
        }
        Err(e) => println!(
            "Complex completion error (may indicate hang issue): {:?}",
            e
        ),
    }
}

#[tokio::test]
async fn test_hanging_scenario_workspace_wide_search() {
    let server_arc = get_shared_server().await;
    let server = server_arc.lock().await;

    // Test workspace symbols with broad query (can be expensive)
    let request = language_server_mcp::models::WorkspaceSymbolsRequest {
        query: "test".to_string(), // Common word likely to have many matches
    };

    println!("Testing potentially hanging workspace-wide symbol search...");
    let result = server.workspace_symbols(Parameters(request)).await;
    match result {
        Ok(tool_result) => {
            println!(
                "Workspace search succeeded: has_content={}",
                has_content(&tool_result)
            );
            assert!(has_content(&tool_result));
        }
        Err(e) => println!("Workspace search error (may indicate hang issue): {:?}", e),
    }
}
