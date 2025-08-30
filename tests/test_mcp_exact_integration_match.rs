// Test the exact same parameters as the failing integration test
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

/// Test helper to get test file path - EXACT same as integration test
fn get_test_file_path() -> String {
    std::env::current_dir()
        .unwrap()
        .join("examples")
        .join("test_file.rs")
        .to_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn test_exact_integration_test_parameters() {
    println!("🎯 Testing EXACT same parameters as failing integration test...");

    let client = create_test_client()
        .await
        .expect("Failed to create MCP client");
    let test_file = get_test_file_path();

    println!("📁 Test file: {}", test_file);
    println!("📍 Position: line 12, column 7 (struct User)");

    // Test hover on User struct definition - EXACT same as integration test
    let hover_result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 12,  // Line with struct User
                "column": 7  // Position on "User"
            })),
        })
        .await;

    match hover_result {
        Ok(_) => {
            println!("✅ EXACT integration test parameters: SUCCESS!");
            println!("   💡 This proves the issue is NOT in the core MCP functionality");
        }
        Err(e) => {
            println!("❌ EXACT integration test parameters: FAILED - {}", e);
            if e.to_string().contains("content modified") {
                println!("   🚨 CONFIRMED: Same content modified error with exact parameters!");
                println!("   💭 The issue might be timing or concurrent test execution");
            } else {
                println!("   🤔 Different error than expected: {}", e);
            }
        }
    }
}
