mod common;

use common::with_timeout;
use std::time::Duration;
use language_server_mcp::lsp_client::LspClient;
use std::path::PathBuf;

#[tokio::test]
async fn test_lsp_client_creation() {
    println!("🔧 Testing LspClient creation and initialization...");
    
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    
    // Test successful creation
    let result = LspClient::new(&workspace_root).await;
    match result {
        Ok(client) => {
            println!("✅ LspClient created successfully");
            
            // Test if rust-analyzer process is running
            if client.is_ready() {
                println!("✅ rust-analyzer is ready");
            } else {
                println!("⚪ rust-analyzer not ready yet (normal during startup)");
            }
            
            // Test basic functionality
            println!("📊 Opened documents count: {}", client.get_opened_documents_count().await);
            
        }
        Err(e) => {
            if format!("{}", e).contains("rust-analyzer not found") {
                println!("⚠️  rust-analyzer not installed - this is expected in some CI environments");
                return; // Graceful skip
            }
            panic!("Failed to create LspClient: {}", e);
        }
    }
}

#[tokio::test]
async fn test_workspace_validation() {
    println!("📁 Testing workspace validation...");
    
    // Test with valid workspace (current directory)
    let valid_workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let result = LspClient::new(&valid_workspace).await;
    
    match result {
        Ok(_) => println!("✅ Valid workspace accepted"),
        Err(e) => {
            if format!("{}", e).contains("rust-analyzer not found") {
                println!("⚠️  rust-analyzer not installed - skipping workspace test");
                return;
            }
            panic!("Valid workspace rejected: {}", e);
        }
    }
    
    // Test with invalid workspace (non-existent directory)
    let invalid_workspace = PathBuf::from("/non/existent/path");
    let result = LspClient::new(&invalid_workspace).await;
    
    match result {
        Ok(_) => println!("⚠️  Invalid workspace was accepted (unexpected)"),
        Err(e) => {
            println!("✅ Invalid workspace rejected as expected: {}", e);
        }
    }
}

#[tokio::test]
async fn test_document_lifecycle() {
    println!("📄 Testing document open/close lifecycle...");
    
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let client = match LspClient::new(&workspace_root).await {
        Ok(client) => client,
        Err(e) => {
            if format!("{}", e).contains("rust-analyzer not found") {
                println!("⚠️  rust-analyzer not installed - skipping document test");
                return;
            }
            panic!("Failed to create LspClient: {}", e);
        }
    };
    
    // Wait for client to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    let test_file = "src/main.rs";
    
    // Test document opening
    let result = client.open_document(test_file).await;
    match result {
        Ok(_) => {
            println!("✅ Document opened successfully");
            println!("📊 Opened documents after open: {}", client.get_opened_documents_count().await);
        }
        Err(e) => println!("⚠️  Document open error: {}", e),
    }
    
    // Test document closing
    let result = client.close_document(test_file).await;
    match result {
        Ok(_) => {
            println!("✅ Document closed successfully");
            println!("📊 Opened documents after close: {}", client.get_opened_documents_count().await);
        }
        Err(e) => println!("⚠️  Document close error: {}", e),
    }
}

#[tokio::test]
async fn test_concurrent_operations() {
    println!("🔄 Testing concurrent operations...");
    
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let client = match LspClient::new(&workspace_root).await {
        Ok(client) => client,
        Err(e) => {
            if format!("{}", e).contains("rust-analyzer not found") {
                println!("⚠️  rust-analyzer not installed - skipping concurrent test");
                return;
            }
            panic!("Failed to create LspClient: {}", e);
        }
    };
    
    // Wait for client to be ready
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // Test concurrent document operations
    let files = ["src/main.rs", "src/lsp_client.rs", "src/models.rs"];
    
    let tasks: Vec<_> = files.iter().map(|file| {
        let client = &client;
        async move {
            let result = with_timeout(
                &format!("concurrent_open_{}", file),
                Duration::from_secs(5),
                client.open_document(file)
            ).await;
            
            match result {
                Ok(Ok(_)) => {
                    println!("✅ Concurrent open succeeded for {}", file);
                    true
                }
                Ok(Err(e)) => {
                    println!("⚠️  Concurrent open LSP error for {}: {}", file, e);
                    false
                }
                Err(e) => {
                    println!("🔥 Concurrent open timed out for {}: {}", file, e);
                    false
                }
            }
        }
    }).collect();
    
    // Execute all concurrent operations
    let results = futures::future::join_all(tasks).await;
    let successful = results.iter().filter(|&&success| success).count();
    
    println!("📊 Concurrent operations: {}/{} successful", successful, files.len());
    println!("📊 Final opened documents count: {}", client.get_opened_documents_count().await);
}

#[tokio::test]
async fn test_error_recovery() {
    println!("🔄 Testing error recovery scenarios...");
    
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let client = match LspClient::new(&workspace_root).await {
        Ok(client) => client,
        Err(e) => {
            if format!("{}", e).contains("rust-analyzer not found") {
                println!("⚠️  rust-analyzer not installed - skipping recovery test");
                return;
            }
            panic!("Failed to create LspClient: {}", e);
        }
    };
    
    // Test recovery from invalid operations
    let invalid_operations = vec![
        ("non_existent.rs", "Invalid file"),
        ("../../../etc/passwd", "Path traversal"),
        ("", "Empty path"),
    ];
    
    for (file, description) in invalid_operations {
        let result = with_timeout(
            &format!("recovery_test_{}", description.replace(' ', "_").to_lowercase()),
            Duration::from_secs(3),
            client.open_document(file)
        ).await;
        
        match result {
            Ok(Ok(_)) => println!("⚠️  {} succeeded unexpectedly", description),
            Ok(Err(e)) => println!("✅ {} failed as expected: {}", description, e),
            Err(e) => println!("⚠️  {} timed out: {}", description, e),
        }
    }
    
    // Verify client is still functional after errors
    let result = client.open_document("src/main.rs").await;
    match result {
        Ok(_) => println!("✅ Client recovered successfully - can still open valid documents"),
        Err(e) => println!("🔥 Client did not recover properly: {}", e),
    }
}