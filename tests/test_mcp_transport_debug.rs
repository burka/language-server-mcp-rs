// Test to isolate MCP transport issues vs rust-analyzer issues
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

#[tokio::test]
async fn test_mcp_transport_simple_operations() {
    println!("🔍 Testing MCP transport with simple operations first...");
    
    let client = create_test_client()
        .await
        .expect("Failed to create MCP client");

    // Test 1: LSP Status (should be very simple, no rust-analyzer semantic analysis)
    println!("\n1️⃣ Testing lsp_status (should be simple):");
    let status_result = client
        .call_tool(CallToolRequestParam {
            name: "lsp_status".into(),
            arguments: Some(object!({})),
        })
        .await;

    match status_result {
        Ok(result) => {
            println!("✅ lsp_status: SUCCESS");
            if let Some(_content) = result.content.first() {
                println!("   Status includes content");
            }
        },
        Err(e) => {
            println!("❌ lsp_status: FAILED - {}", e);
            if e.to_string().contains("content modified") {
                println!("   🚨 Even simple status call has content modified error!");
            }
        }
    }

    // Test 2: Try diagnostics (still relatively simple)
    println!("\n2️⃣ Testing diagnostics:");
    let diag_result = client
        .call_tool(CallToolRequestParam {
            name: "diagnostics".into(),
            arguments: Some(object!({
                "file_path": "src/main.rs"
            })),
        })
        .await;

    match diag_result {
        Ok(_) => println!("✅ diagnostics: SUCCESS"),
        Err(e) => {
            println!("❌ diagnostics: FAILED - {}", e);
            if e.to_string().contains("content modified") {
                println!("   🚨 Diagnostics also has content modified error!");
            }
        }
    }

    // Test 3: Now try hover (the problematic one)
    println!("\n3️⃣ Testing hover (known problematic):");
    let hover_result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": "src/main.rs",
                "line": 10,
                "column": 10
            })),
        })
        .await;

    match hover_result {
        Ok(_) => println!("✅ hover: SUCCESS (unexpected!)"),
        Err(e) => {
            println!("❌ hover: FAILED (expected) - {}", e);
            if e.to_string().contains("content modified") {
                println!("   🎯 Confirmed: hover has content modified error via MCP");
            }
        }
    }

    println!("\n💡 MCP transport analysis complete!");
}

#[tokio::test] 
async fn test_mcp_hover_after_delay() {
    println!("🕐 Testing MCP hover after initial delay...");
    
    let client = create_test_client()
        .await
        .expect("Failed to create MCP client");

    // Wait for rust-analyzer to potentially settle
    println!("⏳ Waiting 6 seconds for rust-analyzer to settle...");
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;

    // Now try hover
    println!("🎯 Trying hover after settling period:");
    let hover_result = client
        .call_tool(CallToolRequestParam {
            name: "hover".into(),
            arguments: Some(object!({
                "file_path": "src/main.rs",
                "line": 20,
                "column": 10
            })),
        })
        .await;

    match hover_result {
        Ok(_) => println!("✅ hover after delay: SUCCESS!"),
        Err(e) => {
            println!("❌ hover after delay: STILL FAILED - {}", e);
            if e.to_string().contains("content modified") {
                println!("   💭 Content modified persists even after delay");
            }
        }
    }
}