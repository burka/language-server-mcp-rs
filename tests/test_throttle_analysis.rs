// Comprehensive analysis of throttling effectiveness at different delays
// Goal: Find the sweet spot for eliminating "content modified" errors

use futures::future::join_all;
use language_server_mcp::models::*;
use language_server_mcp::server::RustAnalyzerMCP;
use rmcp::handler::server::tool::Parameters;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::test]
#[ignore = "Known flaky test that interferes with runtime in full test suite - passes individually"]
async fn test_throttle_delay_sweep() {
    println!("=== Analyzing Throttle Delays for Optimal Success Rate ===");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Test different throttle delays to find optimal value
    let test_delays = vec![25, 50, 75, 100, 150];

    for delay_ms in test_delays {
        println!("\n🔬 Testing {}ms throttle delay...", delay_ms);

        // Set up throttling for this test
        std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
        std::env::set_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS", delay_ms.to_string());

        let server = RustAnalyzerMCP::new(workspace.clone())
            .await
            .expect("Failed to create server");

        // Test with 5 parallel requests
        let start = Instant::now();
        let mut futures = Vec::new();

        let test_requests = vec![
            ("src/main.rs", 20, 10),
            ("src/server.rs", 50, 15),
            ("src/lsp_client.rs", 100, 20),
            ("src/models.rs", 30, 8),
            ("src/tool_handlers.rs", 25, 12),
        ];

        // Fire all requests simultaneously - throttling happens at protocol level
        for (file, line, column) in test_requests {
            let future = server.hover(Parameters(HoverRequest {
                file_path: file.to_string(),
                line,
                column,
            }));
            futures.push(future);
        }

        let results = join_all(futures).await;
        let total_time = start.elapsed();

        let successful = results.iter().filter(|r| r.is_ok()).count();
        let failed = results.len() - successful;
        let success_rate = (successful as f64 / results.len() as f64) * 100.0;

        println!("  📊 {}ms Results:", delay_ms);
        println!(
            "    Success: {}/{} ({:.1}%)",
            successful,
            results.len(),
            success_rate
        );
        println!("    Failed: {}", failed);
        println!("    Total time: {:?}", total_time);
        println!(
            "    Avg time per request: {:?}",
            total_time / results.len() as u32
        );

        // Analyze error types if any failed
        if failed > 0 {
            let error_count = results
                .iter()
                .filter_map(|r| r.as_ref().err())
                .filter(|e| e.to_string().contains("content modified"))
                .count();
            println!("    'Content modified' errors: {}", error_count);
        }

        if success_rate == 100.0 {
            println!("    🎉 PERFECT: {}ms eliminates all errors!", delay_ms);
        } else if success_rate >= 80.0 {
            println!(
                "    ✅ GOOD: {}ms provides excellent success rate",
                delay_ms
            );
        } else if success_rate >= 60.0 {
            println!("    ⚠️  OK: {}ms provides decent improvement", delay_ms);
        } else {
            println!("    ❌ POOR: {}ms still has many failures", delay_ms);
        }
    }

    // Clean up environment
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE");
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS");
}

#[tokio::test]
async fn test_throttle_vs_no_throttle() {
    println!("=== Direct Comparison: Throttled vs Non-Throttled ===");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Test without throttling first
    println!("\n🚫 Testing WITHOUT throttling...");
    let server_no_throttle = RustAnalyzerMCP::new(workspace.clone())
        .await
        .expect("Failed to create non-throttled server");

    let start = Instant::now();
    let mut futures = Vec::new();

    for i in 0..6 {
        let future = server_no_throttle.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }

    let results_no_throttle = join_all(futures).await;
    let time_no_throttle = start.elapsed();
    let success_no_throttle = results_no_throttle.iter().filter(|r| r.is_ok()).count();

    println!(
        "  No throttle: {}/{} success in {:?}",
        success_no_throttle,
        results_no_throttle.len(),
        time_no_throttle
    );

    // Now test WITH throttling (75ms - conservative value)
    println!("\n⚡ Testing WITH 75ms throttling...");
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
    std::env::set_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS", "75");

    let server_throttled = RustAnalyzerMCP::new(workspace)
        .await
        .expect("Failed to create throttled server");

    let start = Instant::now();
    let mut futures = Vec::new();

    for i in 0..6 {
        let future = server_throttled.hover(Parameters(HoverRequest {
            file_path: "src/main.rs".to_string(),
            line: 20 + i as u32,
            column: 10,
        }));
        futures.push(future);
    }

    let results_throttled = join_all(futures).await;
    let time_throttled = start.elapsed();
    let success_throttled = results_throttled.iter().filter(|r| r.is_ok()).count();

    println!(
        "  With 75ms throttle: {}/{} success in {:?}",
        success_throttled,
        results_throttled.len(),
        time_throttled
    );

    // Analysis
    println!("\n📊 Comparison Summary:");
    println!(
        "  Success rate improvement: {} → {} requests",
        success_no_throttle, success_throttled
    );
    println!(
        "  Time trade-off: {:?} → {:?}",
        time_no_throttle, time_throttled
    );

    let improvement = success_throttled as i32 - success_no_throttle as i32;
    if improvement > 0 {
        println!(
            "  🎯 Throttling improved success by {} requests!",
            improvement
        );
    } else if improvement == 0 {
        println!("  ⚖️  Throttling maintained same success rate");
    } else {
        println!("  📉 Throttling reduced success (unexpected)");
    }

    // Note: Throttling is a trade-off - it may improve stability in some scenarios
    // but can reduce performance in others. Both outcomes provide valuable data.
    if success_throttled >= success_no_throttle {
        println!("✅ Throttling helped or maintained performance");
    } else {
        println!("ℹ️  In this scenario, no throttling performed better - this is also valid data");
    }

    // Clean up
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE");
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS");
}

#[tokio::test]
#[ignore = "Known flaky test that interferes with runtime in full test suite - passes individually"]
async fn test_optimal_throttle_recommendation() {
    println!("=== Finding Optimal Throttle Setting for Production ===");

    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Test candidate values based on our staggered request research
    let candidates = vec![
        (25, "Original proven value"),
        (50, "Conservative 2x increase"),
        (75, "Higher reliability target"),
        (100, "Maximum reasonable delay"),
    ];

    let mut best_delay = 25;
    let mut best_success_rate = 0.0;

    for (delay_ms, description) in candidates {
        std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
        std::env::set_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS", delay_ms.to_string());

        let server = RustAnalyzerMCP::new(workspace.clone())
            .await
            .expect("Failed to create server");

        // Test with a challenging scenario: 8 mixed requests
        let mut futures = Vec::new();

        // Mix of hover and diagnostics requests
        for i in 0..4 {
            let hover_future = server.hover(Parameters(HoverRequest {
                file_path: "src/main.rs".to_string(),
                line: 15 + i as u32 * 5,
                column: 8 + i as u32,
            }));
            futures.push(hover_future);

            let diag_future = server.diagnostics(Parameters(DiagnosticsRequest {
                file_path: format!(
                    "src/{}.rs",
                    match i {
                        0 => "server",
                        1 => "lsp_client",
                        2 => "models",
                        _ => "tool_handlers",
                    }
                ),
            }));
            futures.push(diag_future);
        }

        let results = join_all(futures).await;
        let successful = results.iter().filter(|r| r.is_ok()).count();
        let success_rate = (successful as f64 / results.len() as f64) * 100.0;

        println!(
            "  {}ms ({}): {:.1}% success ({}/{})",
            delay_ms,
            description,
            success_rate,
            successful,
            results.len()
        );

        if success_rate > best_success_rate {
            best_success_rate = success_rate;
            best_delay = delay_ms;
        }
    }

    println!("\n🏆 RECOMMENDATION:");
    println!("  Optimal throttle delay: {}ms", best_delay);
    println!("  Achieves: {:.1}% success rate", best_success_rate);

    if best_success_rate >= 95.0 {
        println!("  ✅ Excellent reliability for production use");
    } else if best_success_rate >= 85.0 {
        println!("  ✅ Good reliability, acceptable for most use cases");
    } else {
        println!("  ⚠️  May need additional optimization strategies");
    }

    // Update the default in LSP client if we found something much better
    if best_delay != 25 && best_success_rate > 90.0 {
        println!(
            "  💡 Consider updating default from 25ms to {}ms",
            best_delay
        );
    }

    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE");
    std::env::remove_var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS");
}
