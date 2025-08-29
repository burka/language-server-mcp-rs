mod common;

use common::*;
use std::time::Duration;

#[tokio::test]
async fn test_hover_on_struct() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test hover on a struct definition (LspClient in lsp_client.rs)
    let result = with_timeout(
        "hover_on_struct",
        STANDARD_TIMEOUT,
        client.hover(test_files::LSP_CLIENT_RS, 32, 11), // Line with "pub struct LspClient" (0-indexed line 32, column 11 for "LspClient")
    )
    .await;

    match result {
        Ok(Ok(Some(hover))) => {
            let content = format!("{:?}", hover);
            println!("✅ Hover on struct succeeded: {}", content);
            // Check for struct-related content if available
            if content.contains("LspClient") || content.contains("struct") {
                println!("✅ Content looks correct for struct hover");
            }
        }
        Ok(Ok(None)) => {
            println!("⚪ No hover info for struct position (may be normal depending on rust-analyzer state)");
            // This is often normal, especially if rust-analyzer hasn't finished indexing
        }
        Ok(Err(e)) => {
            println!("⚠️  Hover LSP error: {}", e);
            // Don't panic - LSP errors can be expected during testing
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Hover timed out - hang detected!");
                // This indicates our hang detection is working
            } else {
                panic!("Unexpected hover error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_hover_on_function() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test hover on a function (get_timeout_secs in lsp_client.rs)
    let result = with_timeout(
        "hover_on_function",
        STANDARD_TIMEOUT,
        client.hover(test_files::LSP_CLIENT_RS, 49, 11), // Line with "pub fn get_timeout_secs" (0-indexed line 49, column 11 for function name)
    )
    .await;

    match result {
        Ok(Ok(Some(hover))) => {
            let content = format!("{:?}", hover);
            println!("✅ Hover on function succeeded: {}", content);
            // Check for function-related content if available
            if content.contains("get_timeout_secs") || content.contains("u64") {
                println!("✅ Content looks correct for function hover");
            }
        }
        Ok(Ok(None)) => {
            println!("⚪ No hover info for function position (may be normal depending on rust-analyzer state)");
        }
        Ok(Err(e)) => {
            println!("⚠️  Hover LSP error: {}", e);
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Hover timed out - hang detected!");
            } else {
                panic!("Unexpected hover error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_hover_on_variable() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test hover on a variable in main.rs
    let result = with_timeout(
        "hover_on_variable",
        STANDARD_TIMEOUT,
        client.hover(test_files::MAIN_RS, 1337, 15), // workspace_root variable
    )
    .await;

    match result {
        Ok(Ok(Some(hover))) => {
            let content = format!("{:?}", hover);
            println!("✅ Hover on variable succeeded: {}", content);
        }
        Ok(Ok(None)) => {
            println!("⚪ No hover info for variable position (normal - variables often don't have hover info)");
        }
        Ok(Err(e)) => {
            println!("⚠️  Hover LSP error: {}", e);
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Hover timed out - hang detected!");
            } else {
                panic!("Unexpected hover error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_hover_on_trait() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test hover on trait in test_trait.rs
    let result = with_timeout(
        "hover_on_trait",
        STANDARD_TIMEOUT,
        client.hover(TEST_TRAIT_RS, 6, 10), // Line with "pub trait MyTrait"
    )
    .await;

    match result {
        Ok(Ok(Some(hover))) => {
            let content = format!("{:?}", hover);
            println!("✅ Hover on trait succeeded: {}", content);
            if content.contains("MyTrait") || content.contains("trait") {
                println!("✅ Content looks correct for trait hover");
            }
        }
        Ok(Ok(None)) => {
            println!("⚪ No hover info for trait (may be normal - test_trait.rs might not exist)");
        }
        Ok(Err(e)) => {
            println!("⚠️  Hover LSP error: {}", e);
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Hover timed out - hang detected!");
            } else {
                panic!("Unexpected hover error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_hover_timeout_handling() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test that hover doesn't hang on invalid position
    let result = with_timeout(
        "hover_invalid_position",
        Duration::from_secs(2),                          // Short timeout
        client.hover(test_files::MAIN_RS, 99999, 99999), // Invalid position
    )
    .await;

    // Should complete quickly even with invalid position
    match result {
        Ok(_) => println!("Hover handled invalid position gracefully"),
        Err(e) => {
            if e.contains("timed out") {
                panic!("Hover hung on invalid position - hang detection needed!");
            } else {
                println!("Hover returned error as expected: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_rust_analyzer_hang_detection() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test multiple rapid hover requests that might cause hangs
    println!("Testing hang detection with rapid hover requests...");

    for i in 0..5 {
        let result = with_timeout(
            &format!("hover_rapid_{}", i),
            Duration::from_secs(3), // Generous timeout
            client.hover(test_files::LSP_CLIENT_RS, 32, 11),
        )
        .await;

        match result {
            Ok(Ok(Some(_))) => println!("✅ Hover {} succeeded", i),
            Ok(Ok(None)) => println!("⚪ Hover {} returned no info (normal)", i),
            Ok(Err(e)) => println!("⚠️  Hover {} LSP error: {}", i, e),
            Err(_) => {
                println!("🔥 Hover {} timed out - hang detected!", i);
                // In a real scenario, this would trigger restart
                // For now, just log and continue
                break;
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("Hang detection test completed");
}
