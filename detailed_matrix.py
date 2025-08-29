#!/usr/bin/env python3
import re
import os
from collections import defaultdict

tools = [
    "hover", "completion", "diagnostics", "goto_definition", "find_references",
    "rename", "document_symbols", "code_actions", "workspace_symbols", 
    "signature_help", "document_highlight", "selection_range", "inlay_hints",
    "expand_macro", "runnables", "implementations", "format_document",
    "lsp_status", "close_document"
]

def analyze_tests():
    results = defaultdict(lambda: {"happy": 0, "error": 0, "timeout": 0, "validation": 0})
    
    test_files = [
        "tests/integration_tests.rs",
        "tests/test_comprehensive_error_handling.rs", 
        "tests/test_additional_tools_error_handling.rs",
        "tests/test_completion.rs",
        "tests/test_hover.rs",
        "tests/test_diagnostics.rs",
        "tests/test_tool_handlers.rs",
        "tests/test_detailed_semantic_analysis.rs"
    ]
    
    for file in test_files:
        if not os.path.exists(file):
            continue
            
        with open(file, 'r') as f:
            content = f.read()
            
        # Find test functions
        test_funcs = re.findall(r'#\[tokio::test\].*?async fn (test_\w+)', content, re.DOTALL)
        
        for tool in tools:
            # Happy path tests (general tool tests)
            happy_pattern = rf'test_.*{tool}(?!.*error|.*fail|.*timeout|.*invalid)'
            happy_tests = [t for t in test_funcs if re.match(happy_pattern, t, re.IGNORECASE)]
            results[tool]["happy"] += len(happy_tests)
            
            # Error case tests
            error_pattern = rf'test_.*{tool}.*(?:error|invalid|fail)'
            error_tests = [t for t in test_funcs if re.match(error_pattern, t, re.IGNORECASE)]
            results[tool]["error"] += len(error_tests)
            
            # Timeout tests (look for timeout_behavior calls with tool)
            timeout_matches = re.findall(rf'test_timeout_behavior\([^)]*{tool}', content)
            results[tool]["timeout"] += len(timeout_matches)
            
            # Result validation (assertions on tool results)
            validation_pattern = rf'{tool}.*assert|assert.*{tool}'
            validations = re.findall(validation_pattern, content, re.IGNORECASE)
            results[tool]["validation"] += min(len(validations), 10)  # Cap at 10 to avoid overcounting
    
    return results

results = analyze_tests()

print("| Tool | Happy Path | Error Cases | Timeout Tests | Result Checks | Total Coverage |")
print("|------|------------|-------------|---------------|---------------|----------------|")

for tool in tools:
    r = results[tool]
    total = r["happy"] + r["error"] + r["timeout"]
    status = "✅" if total >= 3 else "⚠️" if total >= 1 else "❌"
    print(f"| {tool:<20} | {r['happy']:^11} | {r['error']:^11} | {r['timeout']:^13} | {r['validation']:^13} | {total:^8} {status} |")

print("\nLegend: ✅ Well tested (3+ tests) | ⚠️ Basic testing (1-2 tests) | ❌ No tests")
