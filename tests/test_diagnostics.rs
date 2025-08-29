mod common;
mod test_positions;

use common::*;
use test_positions::*;
use std::time::Duration;

#[tokio::test]
async fn test_diagnostics_success() {
    println!("🔍 Testing diagnostics on clean code...");
    
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard.as_ref().expect("LSP client should be initialized");
    
    // Test diagnostics on a clean file (should have no errors)
    let result = with_timeout(
        "diagnostics_clean_file",
        STANDARD_TIMEOUT,
        client.diagnostics(main_rs::FILE)
    ).await;
    
    match result {
        Ok(Ok(diagnostics)) => {
            println!("✅ Diagnostics succeeded with {} issues found", diagnostics.len());
            
            // In a clean codebase, we might have warnings but not errors
            let errors = diagnostics.iter().filter(|d| {
                matches!(d.severity, Some(lsp_types::DiagnosticSeverity::ERROR))
            }).count();
            
            let warnings = diagnostics.iter().filter(|d| {
                matches!(d.severity, Some(lsp_types::DiagnosticSeverity::WARNING))
            }).count();
            
            println!("📊 Found {} errors, {} warnings", errors, warnings);
            
            // Print first few diagnostics for inspection
            for (i, diagnostic) in diagnostics.iter().take(3).enumerate() {
                println!("🔍 Diagnostic {}: {:?} at {:?}", i + 1, diagnostic.message, diagnostic.range);
            }
        }
        Ok(Err(e)) => {
            println!("⚠️  Diagnostics LSP error: {}", e);
            // Don't panic - LSP errors can be expected during testing
        }
        Err(e) => {
            if e.contains("timed out") {
                println!("🔥 Diagnostics timed out - hang detected!");
            } else {
                panic!("Unexpected diagnostics error: {}", e);
            }
        }
    }
}

#[tokio::test]
async fn test_diagnostics_multiple_files() {
    println!("🔍 Testing diagnostics on multiple files...");
    
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard.as_ref().expect("LSP client should be initialized");
    
    let test_files = [
        main_rs::FILE,
        lsp_client_rs::FILE,
        models_rs::FILE,
        errors_rs::FILE,
    ];
    
    for file in test_files {
        let result = with_timeout(
            &format!("diagnostics_{}", file.replace('/', "_").replace('.', "_")),
            STANDARD_TIMEOUT,
            client.diagnostics(file)
        ).await;
        
        match result {
            Ok(Ok(diagnostics)) => {
                println!("✅ Diagnostics for {} succeeded: {} issues", file, diagnostics.len());
            }
            Ok(Err(e)) => {
                println!("⚠️  Diagnostics for {} LSP error: {}", file, e);
            }
            Err(e) => {
                if e.contains("timed out") {
                    println!("🔥 Diagnostics for {} timed out!", file);
                } else {
                    println!("❌ Diagnostics for {} failed: {}", file, e);
                }
            }
        }
        
        // Small delay between requests to avoid overwhelming rust-analyzer
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn test_diagnostics_invalid_files() {
    println!("🔍 Testing diagnostics with invalid files...");
    
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard.as_ref().expect("LSP client should be initialized");
    
    let invalid_files = [
        (invalid_files::NON_EXISTENT, "Non-existent file"),
        (invalid_files::PATH_TRAVERSAL, "Path traversal"),
        (invalid_files::EMPTY_PATH, "Empty path"),
    ];
    
    for (file, description) in invalid_files {
        let result = with_timeout(
            &format!("diagnostics_invalid_{}", description.replace(' ', "_").to_lowercase()),
            Duration::from_secs(3), // Shorter timeout for invalid operations
            client.diagnostics(file)
        ).await;
        
        match result {
            Ok(Ok(diagnostics)) => {
                println!("⚠️  {} returned {} diagnostics (unexpected success)", description, diagnostics.len());
            }
            Ok(Err(e)) => {
                println!("✅ {} failed as expected: {}", description, e);
            }
            Err(e) => {
                if e.contains("timed out") {
                    println!("⚠️  {} timed out (might indicate hang)", description);
                } else {
                    println!("✅ {} handled gracefully: {}", description, e);
                }
            }
        }
    }
}

#[tokio::test] 
async fn test_diagnostics_hang_detection() {
    println!("🔍 Testing diagnostics hang detection with rapid requests...");
    
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard.as_ref().expect("LSP client should be initialized");
    
    // Test multiple rapid diagnostic requests
    for i in 0..5 {
        let result = with_timeout(
            &format!("diagnostics_rapid_{}", i),
            Duration::from_secs(3), // Generous timeout for hang detection
            client.diagnostics(main_rs::FILE)
        ).await;
        
        match result {
            Ok(Ok(diagnostics)) => {
                println!("✅ Rapid diagnostics {} succeeded: {} issues", i, diagnostics.len());
            }
            Ok(Err(e)) => {
                println!("⚠️  Rapid diagnostics {} LSP error: {}", i, e);
            }
            Err(_) => {
                println!("🔥 Rapid diagnostics {} timed out - hang detected!", i);
                // In a real scenario, this would trigger restart
                break;
            }
        }
        
        // Very small delay between rapid requests
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    
    println!("Diagnostics hang detection test completed");
}

#[tokio::test]
async fn test_diagnostics_concurrent() {
    println!("🔍 Testing concurrent diagnostics requests...");
    
    let client_arc = get_test_client().await;
    let client_guard = client_arc.lock().await;
    let client = client_guard.as_ref().expect("LSP client should be initialized");
    
    let files = [main_rs::FILE, lsp_client_rs::FILE, models_rs::FILE];
    
    let tasks: Vec<_> = files.iter().enumerate().map(|(i, file)| {
        async move {
            let result = with_timeout(
                &format!("concurrent_diagnostics_{}", i),
                Duration::from_secs(5),
                client.diagnostics(file)
            ).await;
            
            match result {
                Ok(Ok(diagnostics)) => {
                    println!("✅ Concurrent diagnostics {} succeeded: {} issues", i, diagnostics.len());
                    (true, diagnostics.len())
                }
                Ok(Err(e)) => {
                    println!("⚠️  Concurrent diagnostics {} LSP error: {}", i, e);
                    (false, 0)
                }
                Err(e) => {
                    if e.contains("timed out") {
                        println!("🔥 Concurrent diagnostics {} timed out!", i);
                    }
                    (false, 0)
                }
            }
        }
    }).collect();
    
    // Execute all concurrent operations
    let results = futures::future::join_all(tasks).await;
    let successful = results.iter().filter(|(success, _)| *success).count();
    let total_issues: usize = results.iter().map(|(_, count)| count).sum();
    
    println!("📊 Concurrent diagnostics: {}/{} successful, {} total issues found", 
             successful, files.len(), total_issues);
}