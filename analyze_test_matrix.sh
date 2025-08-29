#!/bin/bash

tools=(
    "hover"
    "completion" 
    "diagnostics"
    "goto_definition"
    "find_references"
    "rename"
    "document_symbols"
    "code_actions"
    "workspace_symbols"
    "signature_help"
    "document_highlight"
    "selection_range"
    "inlay_hints"
    "expand_macro"
    "runnables"
    "implementations"
    "format_document"
    "lsp_status"
    "close_document"
)

echo "| Tool | Happy Path | Error Cases | Timeout Tests | Result Validation | Total Tests |"
echo "|------|------------|-------------|---------------|-------------------|-------------|"

for tool in "${tools[@]}"; do
    # Count different test types
    happy=$(grep -r "test.*${tool}" tests/ 2>/dev/null | grep -v "error\|fail\|timeout\|invalid" | wc -l)
    error=$(grep -r "test.*${tool}.*error\|${tool}.*invalid\|${tool}.*fail" tests/ 2>/dev/null | wc -l)
    timeout=$(grep -r "${tool}.*timeout\|timeout.*${tool}" tests/ 2>/dev/null | wc -l)
    result=$(grep -r "${tool}.*assert\|${tool}.*result\|${tool}.*content" tests/ 2>/dev/null | grep -i "assert\|validate\|check" | wc -l)
    total=$((happy + error + timeout))
    
    echo "| $tool | $happy | $error | $timeout | $result | $total |"
done
