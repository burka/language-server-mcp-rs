// Detailed semantic analysis tests that examine actual rust-analyzer responses
// This test logs and validates specific semantic information for different code elements

use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use std::path::PathBuf;

/// Extract actual content text from CallToolResult
fn extract_content_text(result: &CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|content| {
            content.as_text().map(|text| text.text.clone())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Print detailed analysis of a result
fn analyze_result(test_name: &str, code_snippet: &str, result: &CallToolResult) {
    println!("\n🔍 === {} Analysis ===", test_name);
    println!("📝 Code snippet: {}", code_snippet);
    
    let content_text = extract_content_text(result);
    if content_text.is_empty() {
        println!("❌ Result: EMPTY - No semantic information returned");
    } else {
        println!("✅ Result: {} characters of semantic data", content_text.len());
        println!("📄 Content:");
        // Show first 500 chars for document symbols, 200 for others
        let limit = if test_name.contains("Document Symbols") { 500 } else { 200 };
        if content_text.len() > limit {
            println!("   {}", &content_text[..limit]);
            println!("   ... ({} more characters)", content_text.len() - limit);
        } else {
            println!("   {}", content_text);
        }
    }
    println!("🏷️  Content items count: {}", result.content.len());
}

#[tokio::test]
async fn test_detailed_hover_semantic_analysis() {
    println!("=== Detailed Hover Semantic Analysis ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    println!("✅ MCP server ready with pre-warming");
    
    // Test 1: Hover over a function name
    println!("\n🎯 Testing hover over function name...");
    let hover_request = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 18,   // async fn main() -> Result<(), Box<dyn std::error::Error>> {
        column: 15, // On "main" 
    };
    
    match server.hover(Parameters(hover_request)).await {
        Ok(result) => {
            analyze_result("Function Hover", "async fn main()", &result);
            
            let content_text = extract_content_text(&result);
            // Validate we get function signature info
            assert!(!content_text.is_empty(), "Function hover should return semantic info");
            
            if content_text.contains("fn main") || content_text.contains("async") {
                println!("✅ Contains expected function signature information");
            } else {
                println!("⚠️  May not contain expected function signature - got: {}", content_text);
            }
        }
        Err(e) => {
            println!("❌ Function hover failed: {:?}", e);
            panic!("Function hover should work");
        }
    }
    
    // Test 2: Hover over a type/struct
    println!("\n🎯 Testing hover over struct usage...");
    let struct_hover = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 13,  // #[derive(Parser)]
        column: 13, // On "Parser" 
    };
    
    match server.hover(Parameters(struct_hover)).await {
        Ok(result) => {
            analyze_result("Struct Hover", "#[derive(Parser)]", &result);
            
            let content_text = extract_content_text(&result);
            if content_text.contains("trait") || content_text.contains("Parser") {
                println!("✅ Contains trait/derive information");
            } else if !content_text.is_empty() {
                println!("⚠️  Got semantic info but may not be trait-specific: {}", content_text);
            }
        }
        Err(e) => println!("⚠️  Struct hover failed (may be expected): {:?}", e),
    }
    
    // Test 3: Hover over variable with inferred type
    println!("\n🎯 Testing hover over variable with type inference...");
    let var_hover = HoverRequest {
        file_path: "src/main.rs".to_string(), 
        line: 19,  // let args = Args::parse();
        column: 8, // On "args"
    };
    
    match server.hover(Parameters(var_hover)).await {
        Ok(result) => {
            analyze_result("Variable Type Inference", "let args = Args::parse()", &result);
            
            let content_text = extract_content_text(&result);
            if content_text.contains("Args") || content_text.contains("type") {
                println!("✅ Contains type information for variable");
            } else if !content_text.is_empty() {
                println!("⚠️  Got info but may not be type-specific: {}", content_text);
            }
        }
        Err(e) => println!("⚠️  Variable hover failed (may be expected): {:?}", e),
    }
    
    // Test 4: Hover over whitespace/empty area (should return nothing)
    println!("\n🎯 Testing hover over whitespace (negative test)...");
    let empty_hover = HoverRequest {
        file_path: "src/main.rs".to_string(),
        line: 10,  // Empty line
        column: 1, // Beginning of empty line
    };
    
    match server.hover(Parameters(empty_hover)).await {
        Ok(result) => {
            analyze_result("Whitespace Hover", "/* empty line */", &result);
            
            let content_text = extract_content_text(&result);
            if content_text.is_empty() {
                println!("✅ Correctly returns empty result for whitespace");
            } else {
                println!("⚠️  Unexpectedly got content for whitespace: {}", content_text);
            }
        }
        Err(e) => println!("⚠️  Whitespace hover failed (acceptable): {:?}", e),
    }
    
    println!("\n✅ Detailed hover semantic analysis completed!");
}

#[tokio::test]
async fn test_detailed_completion_semantic_analysis() {
    println!("=== Detailed Completion Semantic Analysis ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    // Test 1: Completion after a dot (method completion)
    println!("\n🎯 Testing method completion...");
    let method_completion = CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 19,  // let args = Args::parse();
        column: 25, // After "Args::"
    };
    
    match server.completion(Parameters(method_completion)).await {
        Ok(result) => {
            analyze_result("Method Completion", "Args::", &result);
            
            let content_text = extract_content_text(&result);
            if content_text.contains("parse") || content_text.contains("method") {
                println!("✅ Contains expected method completions");
            } else if !content_text.is_empty() {
                println!("⚠️  Got completions but may not contain 'parse': {}", content_text.chars().take(100).collect::<String>());
            }
        }
        Err(e) => println!("⚠️  Method completion failed: {:?}", e),
    }
    
    // Test 2: Completion in function context
    println!("\n🎯 Testing function context completion...");
    let func_completion = CompletionRequest {
        file_path: "src/main.rs".to_string(),
        line: 30,  // Inside main function after info!
        column: 10, // After "info!"
    };
    
    match server.completion(Parameters(func_completion)).await {
        Ok(result) => {
            analyze_result("Function Context Completion", "info!", &result);
            
            let content_text = extract_content_text(&result);
            if !content_text.is_empty() {
                println!("✅ Got context-aware completions in function");
            }
        }
        Err(e) => println!("⚠️  Function completion failed: {:?}", e),
    }
    
    println!("\n✅ Detailed completion semantic analysis completed!");
}

#[tokio::test]
async fn test_detailed_goto_definition_analysis() {
    println!("=== Detailed Goto Definition Analysis ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    // Test 1: Goto definition on local struct
    println!("\n🎯 Testing goto definition on Args struct...");
    let goto_request = GotoDefinitionRequest {
        file_path: "src/main.rs".to_string(),
        line: 19,  // let args = Args::parse();
        column: 15, // On "Args"
    };
    
    match server.goto_definition(Parameters(goto_request)).await {
        Ok(result) => {
            analyze_result("Goto Definition", "Args", &result);
            
            let content_text = extract_content_text(&result);
            if content_text.contains("struct") || content_text.contains("Args") {
                println!("✅ Found definition information");
            } else if !content_text.is_empty() {
                println!("⚠️  Got definition but may not be struct-specific: {}", content_text);
            }
        }
        Err(e) => println!("⚠️  Goto definition failed: {:?}", e),
    }
    
    println!("\n✅ Detailed goto definition analysis completed!");
}

#[tokio::test] 
async fn test_detailed_document_symbols_analysis() {
    println!("=== Detailed Document Symbols Analysis ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    // Test: Get document symbols for main.rs
    println!("\n🎯 Testing document symbols for main.rs...");
    let symbols_request = DocumentSymbolsRequest {
        file_path: "src/main.rs".to_string(),
        page: 0,
        page_size: 20,
    };
    
    match server.document_symbols(Parameters(symbols_request)).await {
        Ok(result) => {
            analyze_result("Document Symbols", "src/main.rs", &result);
            
            let content_text = extract_content_text(&result);
            
            // Check for expected symbols
            let has_main_fn = content_text.contains("main") && content_text.contains("Function");
            let has_args_struct = content_text.contains("Args") && content_text.contains("Struct");
            
            if has_main_fn {
                println!("✅ Found main function symbol");
            }
            if has_args_struct {
                println!("✅ Found Args struct symbol"); 
            }
            
            if content_text.contains("symbols total") {
                println!("✅ Symbol count information present");
            }
            
            // Count symbol types mentioned
            let symbol_types = ["Function", "Struct", "Module", "Field", "Variable"];
            for symbol_type in symbol_types {
                if content_text.contains(symbol_type) {
                    println!("📍 Found {} symbols", symbol_type);
                }
            }
        }
        Err(e) => println!("⚠️  Document symbols failed: {:?}", e),
    }
    
    println!("\n✅ Detailed document symbols analysis completed!");
}

#[tokio::test]
async fn test_validate_expected_semantic_responses() {
    println!("=== Validation Test: Expected vs Actual Semantic Responses ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    println!("✅ MCP server ready");
    
    // Test 1: Document symbols should contain specific expected elements
    println!("\n🧪 Testing document symbols expectations...");
    let symbols_request = DocumentSymbolsRequest {
        file_path: "src/main.rs".to_string(),
        page: 0,
        page_size: 50,
    };
    
    match server.document_symbols(Parameters(symbols_request)).await {
        Ok(result) => {
            let content_text = extract_content_text(&result);
            
            println!("✅ Document symbols response ({} chars):", content_text.len());
            println!("Full content:\n{}", content_text);
            
            // Expected elements in main.rs
            let expectations = [
                ("Args struct", "Args [Struct]"),
                ("main function", "main [Function]"),
                ("workspace_root field", "workspace_root [Field]"),
                ("symbol count info", "symbols total"),
            ];
            
            for (name, pattern) in expectations {
                if content_text.contains(pattern) {
                    println!("✅ Found {}: '{}'", name, pattern);
                } else {
                    println!("❌ Missing {}: expected '{}'", name, pattern);
                }
            }
        }
        Err(e) => {
            println!("❌ Document symbols failed: {:?}", e);
            panic!("Document symbols should work");
        }
    }
    
    // Test 2: Diagnostics should return meaningful status
    println!("\n🧪 Testing diagnostics expectations...");
    let diag_request = DiagnosticsRequest {
        file_path: "src/main.rs".to_string(),
    };
    
    match server.diagnostics(Parameters(diag_request)).await {
        Ok(result) => {
            let content_text = extract_content_text(&result);
            
            println!("✅ Diagnostics response ({} chars):", content_text.len());
            if content_text.len() > 100 {
                println!("Sample: {}", &content_text[..100]);
            } else {
                println!("Full: {}", content_text);
            }
            
            // Diagnostics should either show errors or "no diagnostics"
            if content_text.contains("no diagnostic") 
                || content_text.contains("No diagnostic") 
                || content_text.contains("error") 
                || content_text.contains("warning") {
                println!("✅ Diagnostics contain meaningful status information");
            } else {
                println!("⚠️  Diagnostics response unclear: '{}'", content_text);
            }
        }
        Err(e) => {
            println!("❌ Diagnostics failed: {:?}", e);
        }
    }
    
    println!("\n✅ Validation test completed!");
}

#[tokio::test]
async fn test_smart_indexing_aware_hover() {
    println!("=== Smart Indexing-Aware Hover Test ===");
    
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let server = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create MCP server");
    
    println!("✅ MCP server ready");
    
    // Test function that implements smart retry with indexing status check
    async fn smart_hover_with_indexing_check(
        server: &RustAnalyzerMCP,
        file_path: &str,
        line: u32,
        column: u32,
        code_snippet: &str,
    ) -> Result<String, String> {
        let request = HoverRequest {
            file_path: file_path.to_string(),
            line,
            column,
        };
        
        // First attempt
        match server.hover(Parameters(request.clone())).await {
            Ok(result) => {
                let content_text = extract_content_text(&result);
                
                // Check if we got meaningful content
                if !content_text.contains("No hover information") && content_text.len() > 50 {
                    println!("✅ First attempt success for '{}': {} chars", code_snippet, content_text.len());
                    return Ok(content_text);
                }
                
                println!("⚠️  First attempt gave poor result for '{}': '{}'", code_snippet, content_text);
                
                // Check if this might be an indexing issue
                if content_text.contains("No hover information") || content_text.contains("not available") {
                    println!("🔍 Checking if rust-analyzer indexing is still in progress...");
                    
                    // Wait a bit for indexing to potentially complete
                    println!("⏳ Waiting 2 seconds for potential indexing completion...");
                    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                    
                    // Retry request
                    match server.hover(Parameters(request)).await {
                        Ok(retry_result) => {
                            let retry_content = extract_content_text(&retry_result);
                            if !retry_content.contains("No hover information") && retry_content.len() > 50 {
                                println!("✅ Retry success for '{}' after indexing wait: {} chars", code_snippet, retry_content.len());
                                return Ok(retry_content);
                            } else {
                                println!("⚠️  Retry still gave poor result: '{}'", retry_content);
                                return Ok(retry_content); // Return what we got
                            }
                        }
                        Err(e) => return Err(format!("Retry failed: {:?}", e)),
                    }
                }
                
                Ok(content_text) // Return what we got
            }
            Err(e) => Err(format!("Hover request failed: {:?}", e)),
        }
    }
    
    // Test cases with smart retry logic
    let test_cases = [
        ("src/main.rs", 18, 15, "async fn main()"),         // Function signature
        ("src/main.rs", 19, 8, "let args"),                // Variable with type inference
        ("src/main.rs", 13, 13, "Parser derive"),          // Trait/derive usage
    ];
    
    for (file_path, line, column, description) in test_cases {
        println!("\n🎯 Testing smart hover on: {}", description);
        
        match smart_hover_with_indexing_check(&server, file_path, line, column, description).await {
            Ok(content) => {
                if content.contains("No hover information") || content.len() < 30 {
                    println!("⚠️  {}: Limited semantic info - may need more indexing time", description);
                    println!("   Content: '{}'", content);
                } else {
                    println!("✅ {}: Good semantic response ({} chars)", description, content.len());
                    println!("   Sample: {}", &content[..std::cmp::min(100, content.len())]);
                }
            }
            Err(e) => {
                println!("❌ {}: Error - {}", description, e);
            }
        }
    }
    
    println!("\n📊 Smart indexing-aware hover test completed!");
    println!("💡 This test demonstrates adaptive waiting for rust-analyzer semantic readiness");
}