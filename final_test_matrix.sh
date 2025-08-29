#!/bin/bash

echo "==================================================================="
echo "           COMPREHENSIVE MCP TOOLS TEST COVERAGE MATRIX"
echo "==================================================================="
echo ""
echo "| Tool                 | Happy | Error | Timeout | Validation | Total | Status |"
echo "|----------------------|-------|-------|---------|------------|-------|--------|"

# Hover
happy=$(grep -l "test.*hover" tests/test_hover.rs tests/integration_tests.rs tests/test_tool_handlers.rs tests/test_detailed_semantic_analysis.rs 2>/dev/null | wc -l)
error=$(grep -c "test_hover_error" tests/test_comprehensive_error_handling.rs 2>/dev/null)
timeout=$(grep -c "test_timeout_behavior.*hover" tests/test_comprehensive_error_handling.rs 2>/dev/null)
validation=$(grep -c "assert.*hover\|hover.*assert" tests/test_detailed_semantic_analysis.rs tests/test_hover.rs 2>/dev/null | head -5 | wc -l)
total=$((happy + error + timeout))
status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "hover" $happy $error $timeout $validation $total "$status"

# Completion
happy=$(grep -l "test.*completion" tests/test_completion.rs tests/integration_tests.rs 2>/dev/null | wc -l)
error=$(grep -c "test_.*completion.*error\|test_.*completion.*invalid" tests/test_completion.rs tests/test_comprehensive_error_handling.rs 2>/dev/null)
timeout=$(grep -c "test_timeout_behavior.*completion" tests/test_comprehensive_error_handling.rs 2>/dev/null)
validation=$(grep -c "has_completions\|assert.*completion" tests/test_completion.rs 2>/dev/null | head -5 | wc -l)
total=$((happy + error + timeout))
status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "completion" $happy $error $timeout $validation $total "$status"

# Diagnostics
happy=$(grep -l "test.*diagnostics" tests/test_diagnostics.rs tests/integration_tests.rs 2>/dev/null | wc -l)
error=$(grep -c "test_diagnostics_error" tests/test_comprehensive_error_handling.rs 2>/dev/null)
timeout=$(grep -c "test_timeout_behavior.*diagnostics" tests/test_comprehensive_error_handling.rs 2>/dev/null)
validation=$(grep -c "assert.*diagnostic" tests/test_diagnostics.rs 2>/dev/null | head -5 | wc -l)
total=$((happy + error + timeout))
status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "diagnostics" $happy $error $timeout $validation $total "$status"

# Goto Definition
happy=$(grep -c "test_goto_definition[^_]" tests/integration_tests.rs 2>/dev/null)
error=$(grep -c "test_goto_definition_error" tests/test_comprehensive_error_handling.rs 2>/dev/null)
timeout=$(grep -c "test_timeout_behavior.*goto_definition" tests/test_comprehensive_error_handling.rs 2>/dev/null)
validation=$(grep -c "assert.*definition" tests/test_comprehensive_error_handling.rs 2>/dev/null | head -3 | wc -l)
total=$((happy + error + timeout))
status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "goto_definition" $happy $error $timeout $validation $total "$status"

# Find References
happy=$(grep -c "test_find_references[^_]" tests/integration_tests.rs 2>/dev/null)
error=$(grep -c "test_find_references_error" tests/test_comprehensive_error_handling.rs 2>/dev/null)
timeout=$(grep -c "test_timeout_behavior.*find_references" tests/test_comprehensive_error_handling.rs 2>/dev/null)
validation=0
total=$((happy + error + timeout))
status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "find_references" $happy $error $timeout $validation $total "$status"

# Other tools - simplified
for tool in "rename" "document_symbols" "code_actions" "workspace_symbols" "signature_help" "document_highlight" "selection_range" "inlay_hints" "expand_macro" "runnables" "implementations" "format_document" "lsp_status" "close_document"; do
    happy=$(grep -c "test_${tool}_tool\|test_${tool}[^_]" tests/integration_tests.rs 2>/dev/null)
    error=$(grep -c "test_${tool}_error" tests/test_comprehensive_error_handling.rs tests/test_additional_tools_error_handling.rs 2>/dev/null)
    timeout=$(grep -c "test_timeout_behavior.*${tool}" tests/test_comprehensive_error_handling.rs tests/test_additional_tools_error_handling.rs 2>/dev/null)
    validation=$(grep -c "assert.*${tool}" tests/test_comprehensive_error_handling.rs tests/test_additional_tools_error_handling.rs 2>/dev/null | head -3 | wc -l)
    total=$((happy + error + timeout))
    status=$([[ $total -ge 3 ]] && echo "✅" || ([[ $total -ge 1 ]] && echo "⚠️" || echo "❌"))
    printf "| %-20s | %5d | %5d | %7d | %10d | %5d | %6s |\n" "$tool" $happy $error $timeout $validation $total "$status"
done

echo "|----------------------|-------|-------|---------|------------|-------|--------|"
echo ""
echo "Legend:"
echo "  ✅ = Well tested (3+ tests)"
echo "  ⚠️ = Basic testing (1-2 tests)"  
echo "  ❌ = No tests"
echo ""
echo "Test Categories:"
echo "  Happy Path: Normal operation tests"
echo "  Error: Error handling and invalid input tests"
echo "  Timeout: 1μs timeout behavior tests"
echo "  Validation: Result assertion and validation checks"
