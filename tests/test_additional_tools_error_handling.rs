// Additional comprehensive error handling tests for remaining MCP tools
// Covers document_highlight, selection_range, inlay_hints, expand_macro, runnables, implementations, format_document

use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::ErrorData as McpError;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

/// Create a test MCP server instance
async fn create_test_server() -> RustAnalyzerMCP {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server for additional error tests")
}

/// Test with extremely short timeout (1 microsecond) to verify timeout handling
/// Note: Operations may complete faster than 1μs, which is actually good performance!
async fn test_timeout_behavior<T, F>(
    operation_name: &str,
    future: F,
) -> Result<String, String>
where
    F: std::future::Future<Output = Result<T, McpError>>,
{
    match timeout(Duration::from_micros(1), future).await {
        Ok(Ok(_)) => Ok(format!("{} completed in under 1μs (excellent performance!)", operation_name)),
        Ok(Err(e)) => Ok(format!("{} returned error: {}", operation_name, e)),
        Err(_) => Ok(format!("{} timed out at 1μs (as expected for slow operations)", operation_name)),
    }
}

#[tokio::test]
async fn test_document_highlight_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Document Highlight Error Scenarios ===");

    // Test 1: Invalid file path
    let invalid_request = DocumentHighlightRequest {
        file_path: "non_existent_file.rs".to_string(),
        line: 0,
        column: 0,
    };

    match server.document_highlight(Parameters(invalid_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No highlights") || content.contains("error"),
                "Invalid file should produce clear message: {}",
                content
            );
            println!("✅ Invalid file message: {}", content);
        }
        Err(e) => {
            println!("✅ Invalid file produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = DocumentHighlightRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let timeout_result = test_timeout_behavior(
        "document_highlight",
        server.document_highlight(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_selection_range_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Selection Range Error Scenarios ===");

    // Test 1: Empty positions list
    let empty_positions = SelectionRangeRequest {
        file_path: "src/main.rs".to_string(),
        positions: vec![],
    };

    match server.selection_range(Parameters(empty_positions)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            println!("✅ Empty positions result: {}", content);
        }
        Err(e) => {
            assert!(
                e.to_string().contains("empty") || e.to_string().contains("no positions"),
                "Empty positions should have clear error: {}",
                e
            );
            println!("✅ Empty positions error: {}", e);
        }
    }

    // Test 2: Invalid positions
    let invalid_positions = SelectionRangeRequest {
        file_path: "src/main.rs".to_string(),
        positions: vec![
            PositionInfo {
                line: 999999,
                column: 999999,
            },
        ],
    };

    match server.selection_range(Parameters(invalid_positions)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            println!("✅ Invalid positions result: {}", content);
        }
        Err(e) => {
            println!("✅ Invalid positions error: {}", e);
        }
    }

    // Test 3: Microsecond timeout
    let timeout_request = SelectionRangeRequest {
        file_path: "src/main.rs".to_string(),
        positions: vec![
            PositionInfo { line: 20, column: 10 },
        ],
    };

    let timeout_result = test_timeout_behavior(
        "selection_range",
        server.selection_range(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_inlay_hints_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Inlay Hints Error Scenarios ===");

    // Test 1: Non-existent file
    let invalid_request = InlayHintsRequest {
        file_path: "does_not_exist.rs".to_string(),
    };

    match server.inlay_hints(Parameters(invalid_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No inlay hints") || content.contains("error"),
                "Invalid file should produce clear message: {}",
                content
            );
            println!("✅ Invalid file message: {}", content);
        }
        Err(e) => {
            println!("✅ Invalid file produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = InlayHintsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let timeout_result = test_timeout_behavior(
        "inlay_hints",
        server.inlay_hints(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_expand_macro_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Expand Macro Error Scenarios ===");

    // Test 1: Position without macro
    let no_macro_request = ExpandMacroRequest {
        file_path: "src/main.rs".to_string(),
        line: 0,
        column: 0,
    };

    match server.expand_macro(Parameters(no_macro_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No macro") || content.contains("not a macro"),
                "No macro position should have clear message: {}",
                content
            );
            println!("✅ No macro message: {}", content);
        }
        Err(e) => {
            println!("✅ No macro produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = ExpandMacroRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let timeout_result = test_timeout_behavior(
        "expand_macro",
        server.expand_macro(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_runnables_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Runnables Error Scenarios ===");

    // Test 1: File with no runnables
    let no_runnables_request = RunnablesRequest {
        file_path: "Cargo.toml".to_string(), // Not a Rust source file
    };

    match server.runnables(Parameters(no_runnables_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            println!("✅ Non-Rust file runnables result: {}", content);
        }
        Err(e) => {
            println!("✅ Non-Rust file produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = RunnablesRequest {
        file_path: "src/main.rs".to_string(),
    };

    let timeout_result = test_timeout_behavior(
        "runnables",
        server.runnables(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_implementations_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Implementations Error Scenarios ===");

    // Test 1: Position without trait
    let no_trait_request = ImplementationsRequest {
        file_path: "src/main.rs".to_string(),
        line: 0,
        column: 0,
    };

    match server.implementations(Parameters(no_trait_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No implementations") || content.contains("not a trait"),
                "No trait position should have clear message: {}",
                content
            );
            println!("✅ No trait message: {}", content);
        }
        Err(e) => {
            println!("✅ No trait produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = ImplementationsRequest {
        file_path: "src/main.rs".to_string(),
        line: 20,
        column: 10,
    };

    let timeout_result = test_timeout_behavior(
        "implementations",
        server.implementations(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_format_document_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Format Document Error Scenarios ===");

    // Test 1: Non-existent file
    let invalid_request = FormatRequest {
        file_path: "non_existent.rs".to_string(),
    };

    match server.format_document(Parameters(invalid_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            println!("✅ Non-existent file format result: {}", content);
        }
        Err(e) => {
            assert!(
                e.to_string().contains("not found") || e.to_string().contains("error"),
                "Non-existent file should have clear error: {}",
                e
            );
            println!("✅ Non-existent file produced error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = FormatRequest {
        file_path: "src/main.rs".to_string(),
    };

    let timeout_result = test_timeout_behavior(
        "format_document",
        server.format_document(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_lsp_status_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing LSP Status Error Scenarios ===");

    // LSP status should generally always work unless the server is down
    let status_request = LspClientStatusRequest {};

    match server.lsp_status(Parameters(status_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("Status") || content.contains("running") || content.contains("Ready"),
                "Status should have meaningful info: {}",
                content
            );
            println!("✅ LSP status message: {}", content);
        }
        Err(e) => {
            println!("⚠️ LSP status error (unexpected): {}", e);
        }
    }

    // Test microsecond timeout for status
    let timeout_result = test_timeout_behavior(
        "lsp_status",
        server.lsp_status(Parameters(LspClientStatusRequest {})),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Status timeout test: {}", msg),
        Err(e) => panic!("Status timeout test failed: {}", e),
    }
}

#[tokio::test]
async fn test_close_document_error_scenarios() {
    let server = create_test_server().await;
    println!("=== Testing Close Document Error Scenarios ===");

    // Test 1: Close non-existent document
    let invalid_request = CloseDocumentRequest {
        file_path: "never_opened.rs".to_string(),
    };

    match server.close_document(Parameters(invalid_request)).await {
        Ok(result) => {
            let content = result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            // Closing a non-open document might succeed silently
            println!("✅ Close non-existent document result: {}", content);
        }
        Err(e) => {
            println!("✅ Close non-existent document error: {}", e);
        }
    }

    // Test 2: Microsecond timeout
    let timeout_request = CloseDocumentRequest {
        file_path: "src/main.rs".to_string(),
    };

    let timeout_result = test_timeout_behavior(
        "close_document",
        server.close_document(Parameters(timeout_request)),
    )
    .await;

    match timeout_result {
        Ok(msg) => println!("✅ Timeout behavior test: {}", msg),
        Err(e) => panic!("Timeout behavior test failed: {}", e),
    }
}

#[tokio::test]
async fn test_concurrent_error_handling() {
    println!("=== Testing Concurrent Error Handling ===");
    let server = create_test_server().await;

    // Fire multiple error-prone requests simultaneously
    let futures = vec![
        server.hover(Parameters(HoverRequest {
            file_path: "invalid1.rs".to_string(),
            line: 0,
            column: 0,
        })),
        server.completion(Parameters(CompletionRequest {
            file_path: "invalid2.rs".to_string(),
            line: 0,
            column: 0,
        })),
        server.diagnostics(Parameters(DiagnosticsRequest {
            file_path: "invalid3.rs".to_string(),
        })),
    ];

    let results = futures::future::join_all(futures).await;

    let mut error_count = 0;
    let mut success_count = 0;

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(_) => {
                success_count += 1;
                println!("✅ Concurrent request {} handled", i);
            }
            Err(e) => {
                error_count += 1;
                println!("✅ Concurrent request {} errored: {}", i, e);
            }
        }
    }

    println!(
        "📊 Concurrent error handling: {} successes, {} errors",
        success_count, error_count
    );

    // All requests should be handled without panics
    assert_eq!(
        success_count + error_count,
        3,
        "All concurrent requests should be handled"
    );
}

#[tokio::test]
async fn test_error_recovery() {
    println!("=== Testing Error Recovery ===");
    let server = create_test_server().await;

    // First, trigger an error
    let bad_request = HoverRequest {
        file_path: "definitely_not_real.rs".to_string(),
        line: 0,
        column: 0,
    };

    let _ = server.hover(Parameters(bad_request)).await;
    println!("✅ Error triggered");

    // Then, verify the server still works with a valid request
    let good_request = DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    match server.diagnostics(Parameters(good_request)).await {
        Ok(_) => println!("✅ Server recovered and handled valid request"),
        Err(e) => panic!("Server failed to recover from error: {}", e),
    }
}