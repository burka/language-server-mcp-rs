use anyhow::Result;
use rmcp::{
    model::CallToolRequestParam,
    object,
    service::RunningService,
    transport::{ConfigureCommandExt, TokioChildProcess},
    RoleClient, ServiceExt,
};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use std::env;

async fn create_client() -> Result<RunningService<RoleClient, ()>> {
    let server_path = std::env::current_dir()?
        .join("target")
        .join("release")
        .join("language-server-mcp");

    let workspace = env::args().nth(1).unwrap_or_else(|| ".".to_string());

    let mut cmd = Command::new(&server_path);
    cmd.arg(&workspace);
    
    let client = ()
        .serve(TokioChildProcess::new(cmd.configure(|_cmd| {}))?)
        .await?;

    Ok(client)
}

#[tokio::main]
async fn main() -> Result<()> {
    let timeout_secs: u64 = env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    println!("Starting MCP client with {}s timeout...", timeout_secs);
    let client = create_client().await?;
    println!("Client connected successfully!");

    let test_file = std::env::current_dir()?
        .join("src")
        .join("test_trait.rs")
        .to_str()
        .unwrap()
        .to_string();

    println!("Testing goto_definition on {} with {}s timeout...", test_file, timeout_secs);
    
    let start = std::time::Instant::now();
    let result = timeout(
        Duration::from_secs(timeout_secs),
        client.call_tool(CallToolRequestParam {
            name: "goto_definition".into(),
            arguments: Some(object!({
                "file_path": test_file,
                "line": 58,
                "column": 15
            })),
        })
    ).await;

    let elapsed = start.elapsed();

    match result {
        Ok(tool_result) => {
            println!("SUCCESS: Tool completed in {:.2}s", elapsed.as_secs_f64());
            match tool_result {
                Ok(call_result) => {
                    println!("Result: error={:?}", call_result.is_error);
                    println!("Content items: {}", call_result.content.len());
                }
                Err(e) => println!("Tool error: {:?}", e),
            }
        }
        Err(_timeout_err) => {
            println!("TIMEOUT: Tool did not complete within {}s (actual: {:.2}s)", 
                timeout_secs, elapsed.as_secs_f64());
        }
    }

    client.cancel().await?;
    Ok(())
}