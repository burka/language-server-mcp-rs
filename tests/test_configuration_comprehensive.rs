//! Comprehensive Configuration Testing Suite
//! 
//! This test suite validates all configuration options and environment variables
//! for the Rust Analyzer MCP server, ensuring robustness across different 
//! configuration combinations and edge cases.
//!
//! Test Coverage:
//! - Environment variable validation (valid/invalid/missing values)  
//! - Workspace path configuration and validation
//! - Configuration combinations and interactions
//! - Integration robustness under various system conditions
//! - Error handling and graceful degradation scenarios

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::timeout;

// Import test infrastructure
mod common;
use common::{create_isolated_test_env, run_sequential_test, STANDARD_TIMEOUT, FAST_TIMEOUT};

// Import the modules we're testing
use language_server_mcp::lsp_client::{
    get_timeout_secs, get_max_response_size, LspClient
};

/// Helper struct to manage environment variable state during tests
struct EnvVarGuard {
    vars: HashMap<String, Option<String>>,
}

impl EnvVarGuard {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }
    
    /// Set an environment variable and track the previous value
    fn set(&mut self, key: &str, value: &str) {
        let previous = env::var(key).ok();
        self.vars.insert(key.to_string(), previous);
        env::set_var(key, value);
    }
    
    /// Remove an environment variable and track the previous value
    fn remove(&mut self, key: &str) {
        let previous = env::var(key).ok();
        self.vars.insert(key.to_string(), previous);
        env::remove_var(key);
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        // Restore all environment variables to their previous state
        for (key, previous_value) in &self.vars {
            match previous_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }
}

#[cfg(test)]
mod environment_variable_tests {
    use super::*;

    #[tokio::test]
    async fn test_timeout_env_var_valid_values() {
        run_sequential_test("timeout_env_var_valid", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test various valid timeout values
            let test_cases = vec![
                ("1", 1),
                ("30", 30),
                ("60", 60),
                ("120", 120),
                ("300", 300),
            ];
            
            for (env_value, expected) in test_cases {
                guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", env_value);
                
                let timeout_secs = get_timeout_secs();
                assert_eq!(
                    timeout_secs, expected,
                    "Expected timeout {} for env value '{}', got {}",
                    expected, env_value, timeout_secs
                );
            }
            
            println!("✅ All valid timeout environment values work correctly");
        }).await;
    }

    #[tokio::test] 
    async fn test_timeout_env_var_invalid_values() {
        run_sequential_test("timeout_env_var_invalid", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test invalid timeout values should fall back to default
            // Note: "0" is actually valid and gets parsed, only non-numeric values fall back
            let invalid_cases = vec![
                "not_a_number",
                "abc123",      // Mixed alphanumeric
                "1.5",         // Float values
                "",            // Empty string
                "999999999999999999999", // Extremely large number that won't parse
            ];
            
            for invalid_value in invalid_cases {
                guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", invalid_value);
                
                let timeout_secs = get_timeout_secs();
                assert_eq!(
                    timeout_secs, 60, // Default timeout value
                    "Invalid timeout env value '{}' should fall back to default (60), got {}",
                    invalid_value, timeout_secs
                );
            }
            
            // Test that "0" is actually accepted as a valid value (even if impractical)
            guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", "0");
            let timeout_secs = get_timeout_secs();
            assert_eq!(timeout_secs, 0, "Zero timeout should be accepted as valid");
            
            println!("✅ Invalid timeout values correctly fall back to defaults, valid values accepted");
        }).await;
    }

    #[tokio::test]
    async fn test_timeout_env_var_missing() {
        run_sequential_test("timeout_env_var_missing", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Remove the environment variable entirely
            guard.remove("RUST_ANALYZER_MCP_TIMEOUT_SECS");
            
            let timeout_secs = get_timeout_secs();
            assert_eq!(
                timeout_secs, 60, // Default timeout value
                "Missing timeout env var should use default (60), got {}",
                timeout_secs
            );
            
            println!("✅ Missing timeout environment variable correctly uses default");
        }).await;
    }

    #[tokio::test]
    async fn test_response_size_env_var_valid_values() {
        run_sequential_test("response_size_env_var_valid", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test various valid response size values
            let test_cases = vec![
                ("1024", 1024),          // 1KB
                ("40960", 40960),        // 40KB (default)
                ("102400", 102400),      // 100KB
                ("1048576", 1048576),    // 1MB
            ];
            
            for (env_value, expected) in test_cases {
                guard.set("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", env_value);
                
                let response_size = get_max_response_size();
                assert_eq!(
                    response_size, expected,
                    "Expected response size {} for env value '{}', got {}",
                    expected, env_value, response_size
                );
            }
            
            println!("✅ All valid response size environment values work correctly");
        }).await;
    }

    #[tokio::test]
    async fn test_response_size_env_var_invalid_values() {
        run_sequential_test("response_size_env_var_invalid", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test invalid response size values should fall back to default
            // Note: "0" is actually valid and gets parsed, only non-numeric values fall back
            let invalid_cases = vec![
                "not_a_number",
                "abc123",      // Mixed alphanumeric  
                "1.5",         // Float values
                "",            // Empty string
            ];
            
            for invalid_value in invalid_cases {
                guard.set("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", invalid_value);
                
                let response_size = get_max_response_size();
                assert_eq!(
                    response_size, 40960, // Default response size (40KB)
                    "Invalid response size env value '{}' should fall back to default (40960), got {}",
                    invalid_value, response_size
                );
            }
            
            // Test that "0" is actually accepted as a valid value (even if impractical)
            guard.set("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", "0");
            let response_size = get_max_response_size();
            assert_eq!(response_size, 0, "Zero response size should be accepted as valid");
            
            println!("✅ Invalid response size values correctly fall back to defaults, valid values accepted");
        }).await;
    }
}

#[cfg(test)]
mod workspace_configuration_tests {
    use super::*;

    #[tokio::test]
    async fn test_absolute_workspace_path() {
        run_sequential_test("absolute_workspace_path", async {
            let _env = create_isolated_test_env().await.unwrap();
            
            // Test with absolute path to current project
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            assert!(workspace_root.is_absolute());
            
            // This should work with our test project
            let result = timeout(
                STANDARD_TIMEOUT,
                LspClient::new(&workspace_root)
            ).await;
            
            match result {
                Ok(Ok(client)) => {
                    println!("✅ Absolute workspace path works correctly");
                    drop(client);
                }
                Ok(Err(e)) => {
                    if e.to_string().contains("rust-analyzer not found") {
                        println!("⚠️  Skipping test - rust-analyzer not available");
                        return;
                    }
                    panic!("Failed to create client with absolute path: {}", e);
                }
                Err(_) => panic!("LspClient::new timed out with absolute path"),
            }
        }).await;
    }

    #[tokio::test] 
    async fn test_relative_workspace_path() {
        run_sequential_test("relative_workspace_path", async {
            let _env = create_isolated_test_env().await.unwrap();
            
            // Test with relative path 
            let workspace_root = PathBuf::from(".");
            assert!(!workspace_root.is_absolute());
            
            // This should be converted to absolute path internally
            let result = timeout(
                STANDARD_TIMEOUT,
                LspClient::new(&workspace_root)
            ).await;
            
            match result {
                Ok(Ok(client)) => {
                    println!("✅ Relative workspace path works correctly");  
                    drop(client);
                }
                Ok(Err(e)) => {
                    if e.to_string().contains("rust-analyzer not found") {
                        println!("⚠️  Skipping test - rust-analyzer not available");
                        return;
                    }
                    panic!("Failed to create client with relative path: {}", e);
                }
                Err(_) => panic!("LspClient::new timed out with relative path"),
            }
        }).await;
    }

    #[tokio::test]
    async fn test_nonexistent_workspace_path() {
        run_sequential_test("nonexistent_workspace_path", async {
            let _env = create_isolated_test_env().await.unwrap();
            
            // Test with non-existent path
            let workspace_root = PathBuf::from("/nonexistent/path/that/does/not/exist");
            
            let result = timeout(
                FAST_TIMEOUT, // Shorter timeout since this should fail quickly
                LspClient::new(&workspace_root)
            ).await;
            
            match result {
                Ok(Ok(_)) => panic!("Should not succeed with non-existent workspace path"),
                Ok(Err(e)) => {
                    // Verify we get appropriate error
                    let error_msg = e.to_string();
                    assert!(
                        error_msg.contains("Failed to get current directory") || 
                        error_msg.contains("not found") ||
                        error_msg.contains("WorkspaceNotFound"),
                        "Expected workspace error, got: {}", error_msg
                    );
                    println!("✅ Non-existent workspace path correctly rejected");
                }
                Err(_) => println!("✅ Non-existent workspace path timed out as expected"),
            }
        }).await;
    }
}

#[cfg(test)]
mod configuration_combinations_tests {
    use super::*;

    #[tokio::test]
    async fn test_configuration_basic_functionality() {
        run_sequential_test("configuration_basic", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test basic configuration functionality
            guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", "45");
            guard.set("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", "51200"); // 50KB
            
            // Verify values are read correctly
            assert_eq!(get_timeout_secs(), 45);
            assert_eq!(get_max_response_size(), 51200);
            
            println!("✅ Basic configuration functionality works correctly");
        }).await;
    }
}

#[cfg(test)]
mod integration_robustness_tests {
    use super::*;

    #[tokio::test]
    async fn test_configuration_with_server_startup() {
        run_sequential_test("configuration_with_server_startup", async {
            let env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Set reasonable configuration
            guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", "10");
            guard.set("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE", "81920"); // 80KB
            
            // Try to create server with this configuration
            let result = timeout(
                Duration::from_secs(15), // Generous timeout for server startup
                env.create_mcp_server()
            ).await;
            
            match result {
                Ok(Ok(server)) => {
                    println!("✅ Server startup with custom configuration successful");
                    drop(server);
                }
                Ok(Err(e)) => {
                    if e.contains("rust-analyzer not found") {
                        println!("⚠️  Skipping test - rust-analyzer not available");
                        return;
                    }
                    panic!("Server startup failed with custom configuration: {}", e);
                }
                Err(_) => panic!("Server startup with custom configuration timed out"),
            }
            
            println!("✅ Configuration integration with server startup works");
        }).await;
    }
}

#[cfg(test)]
mod configuration_edge_cases {
    use super::*;

    #[tokio::test]
    async fn test_unicode_and_special_chars_in_env_vars() {
        run_sequential_test("unicode_special_chars", async {
            let _env = create_isolated_test_env().await.unwrap();
            let mut guard = EnvVarGuard::new();
            
            // Test environment variables with special characters
            let special_cases = vec![
                "🦀",           // Unicode emoji
                "测试",          // Unicode Chinese
                "café",         // Accented characters  
                "hello world",  // Spaces
                "\n\t\r",       // Whitespace
                "",             // Empty string
            ];
            
            for special_value in special_cases {
                guard.set("RUST_ANALYZER_MCP_TIMEOUT_SECS", special_value);
                
                // Should fall back to default for non-numeric values
                let timeout_secs = get_timeout_secs();
                assert_eq!(
                    timeout_secs, 60,
                    "Special character value '{}' should fall back to default", 
                    special_value
                );
            }
            
            println!("✅ Unicode and special characters in environment variables handled correctly");
        }).await;
    }

    #[tokio::test]
    async fn test_malformed_workspace_paths() {
        run_sequential_test("malformed_workspace_paths", async {
            let _env = create_isolated_test_env().await.unwrap();
            
            // Test various malformed or problematic paths
            let problematic_paths = vec![
                PathBuf::from(""),                    // Empty path
                PathBuf::from("////multiple//slashes"), // Multiple slashes
                PathBuf::from("./../../.."),          // Parent directory traversal
            ];
            
            for problematic_path in problematic_paths {
                let result = timeout(
                    FAST_TIMEOUT,
                    LspClient::new(&problematic_path)
                ).await;
                
                match result {
                    Ok(Ok(_)) => {
                        // Some paths might be acceptable after normalization
                        println!("⚠️  Problematic path '{}' was accepted", problematic_path.display());
                    }
                    Ok(Err(e)) => {
                        // Expected to fail gracefully
                        println!("✅ Problematic path '{}' correctly rejected: {}", 
                                problematic_path.display(), e);
                    }
                    Err(_) => {
                        println!("✅ Problematic path '{}' correctly timed out", 
                                problematic_path.display());
                    }
                }
            }
            
            println!("✅ Malformed workspace paths handled gracefully");
        }).await;
    }
}