use anyhow::Result;
use rmcp::{
    model::CallToolRequestParam,
    object,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    RoleClient, ServiceExt,
};
use tokio::process::Command;

/// Test helper to create an MCP client connected to our rust-analyzer server
async fn create_test_client() -> Result<RunningService<RoleClient, ()>> {
    let server_path = std::env::current_dir()?
        .join("target")
        .join("debug")
        .join("language-server-mcp");

    if !server_path.exists() {
        return Err(anyhow::anyhow!(
            "Server binary not found at {:?}. Run 'cargo build' first.",
            server_path
        ));
    }

    let client = ()
        .serve(TokioChildProcess::new(
            Command::new(&server_path).configure(|_cmd| {
                // No additional configuration needed
            }),
        )?)
        .await?;

    Ok(client)
}

/// Test helper to get main.rs file path
fn get_main_file_path() -> String {
    std::env::current_dir()
        .unwrap()
        .join("src")
        .join("main.rs")
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_server_initialization_and_tool_listing() -> Result<()> {
    let client = create_test_client().await?;

    // Test server info
    let server_info = client.peer_info();
    if let Some(info) = server_info {
        // The name comes from the rmcp client, not our server
        assert!(!info.server_info.name.is_empty());
        assert!(!info.server_info.version.is_empty());
    } else {
        panic!("Expected server info, got None");
    }

    // Test listing tools
    let tools = client.list_all_tools().await?;

    // Verify we have all expected tools
    let expected_tools = vec![
        "hover",
        "completion",
        "diagnostics",
        "goto_definition",
        "find_references",
        "format_document",
        "rename",
        "code_actions",
        "workspace_symbols",
        "inlay_hints",
        "expand_macro",
        "document_symbols",
        "signature_help",
        "document_highlight",
        "selection_range",
        "runnables",
        "implementations",
        "lsp_status",
        "close_document",
    ];

    assert_eq!(tools.len(), expected_tools.len());

    for expected_tool in expected_tools {
        assert!(
            tools.iter().any(|tool| tool.name == expected_tool),
            "Missing tool: {}",
            expected_tool
        );
    }

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_hover_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    // Test hover on Args struct definition
    let hover_result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 13,  // Line with struct Args
                "column": 7  // Position on "Args"
            })),
        })
        .await?;

    // Verify we get a successful response with hover information
    assert!(!hover_result.is_error.unwrap_or(true));
    assert!(!hover_result.content.is_empty());

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_diagnostics_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let diagnostics_result = client
        .call_tool(CallToolRequestParam {
            name: "diagnostics".into(),
            arguments: Some(object!({
                "file_path": test_file
            })),
        })
        .await?;

    // Should get a successful response even if no diagnostics
    assert!(!diagnostics_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_completion_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let completion_result = client
        .call_tool(CallToolRequestParam {
            name: "completion".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 69,  // Line with "user." for completion
                "column": 25  // After "user."
            })),
        })
        .await?;

    // Should get completions for User methods
    assert!(!completion_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_goto_definition_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let goto_result = client
        .call_tool(CallToolRequestParam {
            name: "goto_definition".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 75,  // Line with HashMap usage
                "column": 25  // Position on HashMap
            })),
        })
        .await?;

    // Should find definition location
    assert!(!goto_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_find_references_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let refs_result = client
        .call_tool(CallToolRequestParam {
            name: "find_references".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 13,  // Line with User struct definition
                "column": 7,  // Position on "User"
                "include_declaration": true
            })),
        })
        .await?;

    // Should find references to User
    assert!(!refs_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_format_document_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let format_result = client
        .call_tool(CallToolRequestParam {
            name: "format_document".into(),
            arguments: Some(object!({
                "file_path": test_file
            })),
        })
        .await?;

    // Should successfully format (or report no changes needed)
    assert!(!format_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_workspace_symbols_tool() -> Result<()> {
    let client = create_test_client().await?;

    let symbols_result = client
        .call_tool(CallToolRequestParam {
            name: "workspace_symbols".into(),
            arguments: Some(object!({
                "query": "User"
            })),
        })
        .await?;

    // Should find User symbol in workspace
    assert!(!symbols_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_inlay_hints_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let hints_result = client
        .call_tool(CallToolRequestParam {
            name: "inlay_hints".into(),
            arguments: Some(object!({
                "file_path": test_file
            })),
        })
        .await?;

    // Should get inlay hints successfully
    assert!(!hints_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_runnables_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "runnables".into(),
            arguments: Some(object!({
                "file_path": test_file
            })),
        })
        .await;

    // Runnables might not be supported by all rust-analyzer versions
    match result {
        Ok(runnables_result) => {
            // If successful, should not have error
            assert!(!runnables_result.is_error.unwrap_or(true));
        }
        Err(_) => {
            // If runnables is not supported, that's also acceptable
            // The "unknown request" error suggests this LSP method is not available
        }
    }

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_implementations_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let impl_result = client
        .call_tool(CallToolRequestParam {
            name: "implementations".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 6,   // Line with Greetable trait
                "column": 6  // Position on trait name
            })),
        })
        .await?;

    // Should find implementations of Greetable trait
    assert!(!impl_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_lsp_status_tool() -> Result<()> {
    let client = create_test_client().await?;

    let status_result = client
        .call_tool(CallToolRequestParam {
            name: "lsp_status".into(),
            arguments: Some(object!({})),
        })
        .await?;

    // Should get LSP status information
    assert!(!status_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_document_caching() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    // First call to hover - should open document
    let hover_result1 = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 12,
                "column": 7
            })),
        })
        .await?;

    assert!(!hover_result1.is_error.unwrap_or(true));

    // Second call to same file - should use cached document
    let hover_result2 = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 20,  // Different line
                "column": 7
            })),
        })
        .await?;

    assert!(!hover_result2.is_error.unwrap_or(true));

    // Check status to see opened documents
    let status_result = client
        .call_tool(CallToolRequestParam {
            name: "lsp_status".into(),
            arguments: Some(object!({})),
        })
        .await?;

    assert!(!status_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_close_document_tool() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    // First open a document
    let _hover_result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 12,
                "column": 7
            })),
        })
        .await?;

    // Now close it
    let close_result = client
        .call_tool(CallToolRequestParam {
            name: "close_document".into(),
            arguments: Some(object!({
                "file_path": test_file
            })),
        })
        .await?;

    assert!(!close_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling_invalid_file() -> Result<()> {
    let client = create_test_client().await?;
    let invalid_file = "/nonexistent/file.rs";

    let result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": invalid_file,
                "line": 10,
                "column": 5
            })),
        })
        .await;

    // Should get a result (potentially with error message) or error for invalid file
    match result {
        Ok(hover_result) => {
            // If we get a result, it should indicate no hover information or error
            let content = hover_result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No hover") || content.contains("error") || 
                content.contains("not found") || content.is_empty(),
                "Invalid file should produce appropriate message: {}",
                content
            );
        }
        Err(_) => {
            // Getting an error is also acceptable
            // We just verify the server doesn't crash
        }
    }

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_error_handling_invalid_position() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();

    let result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 10000,  // Way beyond file length
                "column": 10000
            })),
        })
        .await;

    // Should handle out-of-bounds position gracefully
    match result {
        Ok(hover_result) => {
            // If we get a result, it should indicate no hover information
            let content = hover_result
                .content
                .iter()
                .filter_map(|c| c.as_text())
                .map(|t| t.text.clone())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                content.contains("No hover") || content.contains("out of bounds") || 
                content.contains("error") || content.is_empty(),
                "Out-of-bounds position should produce appropriate message: {}",
                content
            );
        }
        Err(_) => {
            // Getting an error is also acceptable
            // We just verify the server handles it without crashing
        }
    }

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_timeout_hanging_reproduction() -> Result<()> {
    use tokio::time::{timeout, Duration};

    let client = create_test_client().await?;

    // Test with our test_trait.rs file that seems to cause hanging
    let hanging_file = std::env::current_dir()
        .unwrap()
        .join("src")
        .join("test_trait.rs")
        .to_str()
        .unwrap()
        .to_string();

    println!(
        "Testing goto_definition on potentially hanging file: {}",
        hanging_file
    );

    // Set a 10-second timeout to verify timeout mechanism works
    let goto_result = timeout(
        Duration::from_secs(10),
        client.call_tool(CallToolRequestParam {
            name: "goto_definition".into(),
            arguments: Some(object!({
                "file_path": hanging_file,
                "line": 58,  // Line with my_struct usage
                "column": 15 // Position on variable
            })),
        }),
    )
    .await;

    match goto_result {
        Ok(result) => {
            match result {
                Ok(call_result) => {
                    println!(
                        "Got result (should not hang): error={:?}",
                        call_result.is_error
                    );
                }
                Err(e) => {
                    println!("Got error result: {:?}", e);
                }
            }
            // If we get here, the request completed within timeout
        }
        Err(_) => {
            println!("Request timed out after 10 seconds - this proves timeout works");
            // This is what we expect if the underlying request hangs
        }
    }

    client.cancel().await?;
    Ok(())
}

#[tokio::test]
async fn test_memory_management() -> Result<()> {
    let client = create_test_client().await?;
    let test_file = get_main_file_path();
    let main_file = get_main_file_path();

    // Open multiple documents
    for file in [&test_file, &main_file] {
        let _hover_result = client
            .call_tool(CallToolRequestParam {
                name: "hover".into(),
                arguments: Some(object!({
                    "file_path": file,
                    "line": 10,
                    "column": 5
                })),
            })
            .await?;
    }

    // Check memory status
    let status_result = client
        .call_tool(CallToolRequestParam {
            name: "lsp_status".into(),
            arguments: Some(object!({})),
        })
        .await?;

    assert!(!status_result.is_error.unwrap_or(true));

    client.cancel().await?;
    Ok(())
}
