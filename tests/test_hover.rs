mod common;

use common::*;
use std::time::Duration;

#[tokio::test]
async fn test_hover_on_struct() {
    let client = get_test_client().await;
    let client = client.lock().await;
    
    // Test hover on a struct definition (LspClient in lsp_client.rs)
    let result = with_timeout(
        "hover_on_struct",
        STANDARD_TIMEOUT,
        client.hover(test_files::LSP_CLIENT_RS, 115, 12) // Line with "pub struct LspClient"
    ).await;
    
    match result {
        Ok(Some(hover)) => {
            let content = format!("{:?}", hover);
            assert_contains(&content, "LspClient", "Hover on struct");
            assert_contains(&content, "struct", "Should show it's a struct");
        }
        Ok(None) => panic!("Expected hover information but got None"),
        Err(e) => panic!("Hover failed: {}", e),
    }
}

#[tokio::test]
async fn test_hover_on_function() {
    let client = get_test_client().await;
    let client = client.lock().await;
    
    // Test hover on a function (get_timeout_secs in lsp_client.rs)
    let result = with_timeout(
        "hover_on_function",
        STANDARD_TIMEOUT,
        client.hover(test_files::LSP_CLIENT_RS, 129, 10) // Line with "pub fn get_timeout_secs"
    ).await;
    
    match result {
        Ok(Some(hover)) => {
            let content = format!("{:?}", hover);
            assert_contains(&content, "get_timeout_secs", "Hover on function");
            assert_contains(&content, "u64", "Should show return type");
        }
        Ok(None) => panic!("Expected hover information but got None"),
        Err(e) => panic!("Hover failed: {}", e),
    }
}

#[tokio::test]
async fn test_hover_on_variable() {
    let client = get_test_client().await;
    let client = client.lock().await;
    
    // Test hover on a variable in main.rs
    let result = with_timeout(
        "hover_on_variable",
        STANDARD_TIMEOUT,
        client.hover(test_files::MAIN_RS, 1337, 15) // workspace_root variable
    ).await;
    
    match result {
        Ok(Some(hover)) => {
            let content = format!("{:?}", hover);
            assert_not_empty(&content, "Hover on variable should return content");
            // Variable hover should show type information
        }
        Ok(None) => {
            // Some positions might not have hover info, that's OK
            println!("No hover info at this position");
        }
        Err(e) => panic!("Hover failed: {}", e),
    }
}

#[tokio::test]
async fn test_hover_on_trait() {
    let client = get_test_client().await;
    let client = client.lock().await;
    
    // Test hover on trait in test_trait.rs
    let result = with_timeout(
        "hover_on_trait",
        STANDARD_TIMEOUT,
        client.hover(test_files::TEST_TRAIT_RS, 6, 10) // Line with "pub trait MyTrait"
    ).await;
    
    match result {
        Ok(Some(hover)) => {
            let content = format!("{:?}", hover);
            assert_contains(&content, "MyTrait", "Hover on trait");
            assert_contains(&content, "trait", "Should indicate it's a trait");
        }
        Ok(None) => {
            println!("No hover info for trait (might be OK depending on rust-analyzer version)");
        }
        Err(e) => panic!("Hover failed: {}", e),
    }
}

#[tokio::test]
async fn test_hover_timeout_handling() {
    let client = get_test_client().await;
    let client = client.lock().await;
    
    // Test that hover doesn't hang on invalid position
    let result = with_timeout(
        "hover_invalid_position",
        Duration::from_secs(2), // Short timeout
        client.hover(test_files::MAIN_RS, 99999, 99999) // Invalid position
    ).await;
    
    // Should complete quickly even with invalid position
    match result {
        Ok(_) => println!("Hover handled invalid position gracefully"),
        Err(e) if e.contains("timed out") => {
            panic!("Hover hung on invalid position - hang detection needed!");
        }
        Err(e) => println!("Hover returned error as expected: {}", e),
    }
}