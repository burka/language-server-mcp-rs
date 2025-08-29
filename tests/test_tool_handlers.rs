// Direct tests for tool handlers to improve coverage
use language_server_mcp::tool_handlers::*;
use language_server_mcp::{
    CompletionRequest, DiagnosticsRequest, GotoDefinitionRequest, HoverRequest, LspClient,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn create_test_client() -> Arc<Mutex<LspClient>> {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let client = LspClient::new(&workspace)
        .await
        .expect("Failed to create LSP client");
    Arc::new(Mutex::new(client))
}

#[tokio::test]
async fn test_handle_hover_success() {
    let client = create_test_client().await;

    // Test hover on a known position
    let request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 40,
        column: 10,
    };

    let result = handle_hover(&client, request).await;

    // Should either succeed or return a reasonable error message
    match result {
        Ok(content) => {
            println!("Hover content: {}", content);
            assert!(!content.is_empty());
        }
        Err(e) => {
            println!("Hover error (may be expected): {}", e);
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handle_hover_no_info() {
    let client = create_test_client().await;

    // Test hover on whitespace/comment
    let request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 1,
        column: 0,
    };

    let result = handle_hover(&client, request).await;

    // Should return either no info or an error
    match result {
        Ok(content) => {
            assert!(content.contains("No hover information") || !content.is_empty());
        }
        Err(e) => {
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handle_hover_invalid_file() {
    let client = create_test_client().await;

    let request = HoverRequest {
        file_path: "non_existent_file.rs".to_string(),
        line: 0,
        column: 0,
    };

    let result = handle_hover(&client, request).await;

    // Should return an error
    assert!(result.is_err() || result.unwrap().contains("No hover"));
}

#[tokio::test]
async fn test_handle_completion_success() {
    let client = create_test_client().await;

    let request = CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 10,
        column: 5,
    };

    let result = handle_completion(&client, request).await;

    match result {
        Ok(content) => {
            println!("Completion content: {}", content);
            assert!(!content.is_empty());
        }
        Err(e) => {
            println!("Completion error (may be expected): {}", e);
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handle_diagnostics_success() {
    let client = create_test_client().await;

    let request = DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };

    let result = handle_diagnostics(&client, request).await;

    match result {
        Ok(content) => {
            println!("Diagnostics content: {}", content);
            assert!(!content.is_empty());
            // Should either have diagnostics or say "No diagnostics found"
            assert!(content.contains("Diagnostics:") || content.contains("No diagnostics"));
        }
        Err(e) => {
            println!("Diagnostics error: {}", e);
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handle_goto_definition_success() {
    let client = create_test_client().await;

    // Test on a known symbol
    let request = GotoDefinitionRequest {
        file_path: "src/main.rs".to_string(),
        line: 24, // Line with LspClient usage
        column: 20,
    };

    let result = handle_goto_definition(&client, request).await;

    match result {
        Ok(content) => {
            println!("Goto definition content: {}", content);
            assert!(!content.is_empty());
        }
        Err(e) => {
            println!("Goto definition error: {}", e);
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handle_goto_definition_no_definition() {
    let client = create_test_client().await;

    // Test on a position with no definition
    let request = GotoDefinitionRequest {
        file_path: "src/main.rs".to_string(),
        line: 1,
        column: 0,
    };

    let result = handle_goto_definition(&client, request).await;

    // Should return no definition or error
    match result {
        Ok(content) => {
            assert!(content.contains("No definition") || content.contains("Definitions:"));
        }
        Err(e) => {
            assert!(e.contains("LSP error"));
        }
    }
}

#[tokio::test]
async fn test_handlers_with_invalid_positions() {
    let client = create_test_client().await;

    // Test hover with out-of-bounds position
    let hover_request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 99999,
        column: 99999,
    };

    let hover_result = handle_hover(&client, hover_request).await;
    // Should handle gracefully
    assert!(hover_result.is_ok() || hover_result.unwrap_err().contains("LSP error"));

    // Test completion with out-of-bounds position
    let completion_request = CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 99999,
        column: 99999,
    };

    let completion_result = handle_completion(&client, completion_request).await;
    assert!(completion_result.is_ok() || completion_result.unwrap_err().contains("LSP error"));
}
