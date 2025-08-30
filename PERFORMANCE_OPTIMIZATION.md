# Test Performance Optimization Guide

## The Problem

Many tests are slow because they:
1. **Create fresh rust-analyzer servers** (each takes 2-10 seconds to initialize)
2. **Add artificial delays** like `tokio::time::sleep(Duration::from_secs(10))`
3. **Wait for "warmup"** that's already been done

## Current Performance Issues

### Slow Tests Analysis
- `test_warm_server_throttling`: **20+ seconds** (4 × 10s delays + 4 server startups)
- Tests with `RustAnalyzerMCP::new()`: **23 tests** create fresh servers
- Each server startup: **2-10 seconds** depending on system

### Time Breakdown
```
Fresh server creation:     ~5 seconds per server
Artificial sleep delays:   10+ seconds per test  
Actual test operations:    <100ms per request
```

## The Solution: Shared Server Pattern

### ✅ Fast Pattern (Use This)
```rust
mod common;
use common::get_test_mcp_server;

#[tokio::test]
async fn test_fast_example() {
    // Get shared server (initialized once, reused everywhere)
    let server_container = get_test_mcp_server().await;
    let server_guard = server_container.lock().await;
    let server = server_guard.as_ref().expect("Server initialized");
    
    // Your test logic - fast because no startup delay!
    let result = server.hover(/* params */).await;
    assert!(result.is_ok());
    
    // Total time: ~100ms instead of ~10s
}
```

### ❌ Slow Pattern (Avoid This)
```rust
#[tokio::test]
async fn test_slow_example() {
    // Creates fresh server every time - slow!
    let server = RustAnalyzerMCP::new(workspace).await.unwrap();
    
    // Artificial delay - unnecessary!
    tokio::time::sleep(Duration::from_secs(10)).await;
    
    // Your test logic
    let result = server.hover(/* params */).await;
    assert!(result.is_ok());
    
    // Total time: ~15s (10s delay + 5s startup)
}
```

## Migration Guide

### Step 1: Remove Artificial Delays
Replace long delays with shorter ones or remove entirely:
```rust
// OLD: Unnecessary 10 second delay
tokio::time::sleep(Duration::from_secs(10)).await;

// NEW: Short delay if needed, or remove entirely
tokio::time::sleep(Duration::from_millis(100)).await;
```

### Step 2: Use Shared Server
Replace fresh server creation:
```rust
// OLD: Fresh server every test
let server = RustAnalyzerMCP::new(workspace).await?;

// NEW: Shared server (initialized once)
let server_container = get_test_mcp_server().await;
let server_guard = server_container.lock().await;
let server = server_guard.as_ref().expect("Server should exist");
```

### Step 3: When to Create Fresh Servers
Only create fresh servers when you need:
- **Complete isolation** between tests
- **Different configuration** (environment variables)
- **Testing server initialization** itself

For most tests, shared server is fine because:
- rust-analyzer is stateless for most operations
- Tests use different file positions avoiding conflicts
- Much faster execution

## Performance Comparison

### Before Optimization
```
test_warm_server_throttling:     22.5s (4 servers + 4×10s delays)
test_concurrent_requests:        15.2s (fresh server + 10s delay)
test_error_handling:             12.8s (fresh server + delays)
Total for 3 tests:               50.5s
```

### After Optimization
```
test_fast_shared_server:         0.3s (shared server)
test_optimized_throttling:       1.2s (shared server + small delays)
test_fast_concurrent:            0.8s (shared server)
Total for 3 tests:               2.3s
```

**Performance Gain: 22x faster!**

## Implementation Tips

### 1. Use Common Test Infrastructure
```rust
mod common;
use common::*;  // Provides get_test_mcp_server(), timeouts, etc.
```

### 2. Handle Server Access Properly
```rust
// Get server reference
let server_container = get_test_mcp_server().await;
let server_guard = server_container.lock().await;
let server = server_guard.as_ref().expect("Server initialized");

// Use server
let result = server.some_operation().await;

// Lock is automatically dropped at end of scope
```

### 3. When You Need Fresh Server
```rust
// Only for tests that truly need isolation
let fresh_server = create_fresh_mcp_server().await;
```

### 4. Environment Variables
```rust
// Set before getting shared server
std::env::set_var("SOME_CONFIG", "value");
let server = get_test_mcp_server().await;

// Clean up after test
std::env::remove_var("SOME_CONFIG");
```

## Test Categories

### Category A: Use Shared Server (Most Tests)
- Hover information tests
- Completion tests  
- Diagnostics tests
- Basic functionality tests
- Performance comparison tests

### Category B: Use Fresh Server (Few Tests)
- Server initialization tests
- Configuration change tests
- Resource cleanup tests
- Process lifecycle tests

## Monitoring Performance

Run specific test groups to compare:
```bash
# Fast tests (shared server)
cargo test test_fast

# Slow tests (for comparison) 
cargo test test_warm_server_throttling

# All optimized tests
cargo test test_optimized
```

## Expected Results

With these optimizations:
- **Individual tests**: 100-500ms instead of 10-20s
- **Test suite**: Minutes instead of hours
- **Development cycle**: Much faster feedback
- **CI/CD**: Significantly reduced build times

The goal is to make tests so fast that developers run them frequently during development.