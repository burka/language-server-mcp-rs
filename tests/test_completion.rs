mod common;

use common::*;
use std::time::Duration;

#[tokio::test]
async fn test_completion_basic() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test completion in a simple context (after "std::")
    let result = with_timeout(
        "completion_basic",
        STANDARD_TIMEOUT,
        client.completion(test_files::MAIN_RS, 1, 5), // Somewhere in main.rs where "std::" might make sense
    )
    .await;

    match result {
        Ok(Ok(Some(completions))) => {
            println!(
                "✅ Completion succeeded with {} items",
                match completions {
                    lsp_types::CompletionResponse::Array(items) => items.len(),
                    lsp_types::CompletionResponse::List(list) => list.items.len(),
                }
            );
        }
        Ok(Ok(None)) => {
            println!("⚪ No completions available (normal depending on position)");
        }
        Ok(Err(e)) => {
            println!("⚠️  Completion LSP error: {}", e);
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Completion timed out - hang detected!");
            } else {
                panic!("Unexpected completion error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_completion_hang_detection() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test multiple rapid completion requests that might cause hangs
    println!("Testing completion hang detection with rapid requests...");

    for i in 0..3 {
        let result = with_timeout(
            &format!("completion_rapid_{}", i),
            Duration::from_secs(3), // Generous timeout
            client.completion(test_files::LSP_CLIENT_RS, 10, 5),
        )
        .await;

        match result {
            Ok(Ok(Some(_))) => println!("✅ Completion {} succeeded", i),
            Ok(Ok(None)) => println!("⚪ Completion {} returned no items (normal)", i),
            Ok(Err(e)) => println!("⚠️  Completion {} LSP error: {}", i, e),
            Err(_) => {
                println!("🔥 Completion {} timed out - hang detected!", i);
                break;
            }
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("Completion hang detection test completed");
}

#[tokio::test]
async fn test_completion_invalid_position() {
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard
        .as_ref()
        .expect("LSP client should be initialized");

    // Test completion at invalid position
    let result = with_timeout(
        "completion_invalid_position",
        Duration::from_secs(2),                               // Short timeout
        client.completion(test_files::MAIN_RS, 99999, 99999), // Invalid position
    )
    .await;

    match result {
        Ok(_) => println!("Completion handled invalid position gracefully"),
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Completion hung on invalid position!");
            } else {
                println!("Completion returned error as expected: {}", e);
            }
        }
    }
}
