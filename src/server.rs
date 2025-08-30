#![deny(dead_code)]

// MCP Server implementation
// This module contains the main RustAnalyzerMCP server that implements all tool handlers

use crate::domain::{Position, RustAnalyzer};
use crate::lsp_client;
use crate::models::*;

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};

#[derive(Clone)]
pub struct RustAnalyzerMCP {
    rust_analyzer: Arc<RustAnalyzer>,
    workspace_root: PathBuf,
    tool_router: ToolRouter<RustAnalyzerMCP>,
    test_start_time: Instant,
}

#[tool_router]
impl RustAnalyzerMCP {
    pub async fn new(workspace_root: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        info!(
            "Initializing rust-analyzer MCP server for workspace: {:?}",
            workspace_root
        );
        let rust_analyzer = RustAnalyzer::new(workspace_root.clone())
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        info!("rust-analyzer domain service initialized and ready");

        let server = Self {
            rust_analyzer: Arc::new(rust_analyzer),
            workspace_root,
            tool_router: Self::tool_router(),
            test_start_time: Instant::now(),
        };

        Ok(server)
    }

    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Create a new RustAnalyzerMCP with throttling enabled via environment variables
    /// Sets RUST_ANALYZER_MCP_THROTTLE=1 and RUST_ANALYZER_MCP_THROTTLE_DELAY_MS
    pub async fn with_throttling(
        workspace_root: PathBuf,
        throttle_delay_ms: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Enable throttling via environment variables for this instance
        std::env::set_var("RUST_ANALYZER_MCP_THROTTLE", "1");
        std::env::set_var(
            "RUST_ANALYZER_MCP_THROTTLE_DELAY_MS",
            throttle_delay_ms.to_string(),
        );

        Self::new(workspace_root).await
    }

    /// Check if throttling is enabled (via environment variable)
    pub fn is_throttle_enabled(&self) -> bool {
        std::env::var("RUST_ANALYZER_MCP_THROTTLE").is_ok()
    }

    /// Get the throttle delay in milliseconds (via environment variable)
    pub fn get_throttle_delay_ms(&self) -> u64 {
        std::env::var("RUST_ANALYZER_MCP_THROTTLE_DELAY_MS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(100)
    }

    /// Execute a tool handler with automatic retry for any readiness errors
    /// Implements Option A: Automatic Retry with graceful handling for all operations
    #[allow(dead_code)]
    async fn execute_with_retry<F, Fut, T>(
        &self,
        operation_name: &str,
        handler: F,
    ) -> Result<T, crate::errors::LspError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        const MAX_RETRIES: usize = 3; // Increased for semantic readiness retries
        const RETRY_DELAY: Duration = Duration::from_millis(300);
        const SEMANTIC_RETRY_DELAY: Duration = Duration::from_millis(1500); // Longer wait for indexing

        for attempt in 1..=MAX_RETRIES {
            match handler().await {
                Ok(result) => return Ok(result),
                Err(error) => {
                    // Check for any readiness issues (not just semantic operations)
                    if Self::is_readiness_issue(&error) && attempt < MAX_RETRIES {
                        // Use longer delay for semantic operations, shorter for others
                        let delay = if Self::is_semantic_operation(operation_name) {
                            warn!(
                                "Operation '{}' may need semantic indexing (attempt {}). Waiting {:?} for rust-analyzer readiness...", 
                                operation_name, attempt, SEMANTIC_RETRY_DELAY
                            );
                            SEMANTIC_RETRY_DELAY
                        } else {
                            warn!(
                                "Operation '{}' got readiness issue (attempt {}). Retrying in {:?}...", 
                                operation_name, attempt, RETRY_DELAY
                            );
                            RETRY_DELAY
                        };
                        sleep(delay).await;
                        continue;
                    }

                    let lsp_error = crate::errors::LspError::from_lsp_error(error, operation_name);

                    // Handle initialization errors (original logic)
                    if lsp_error.is_initialization_error() && attempt < MAX_RETRIES {
                        warn!(
                            "Operation '{}' failed due to initialization (attempt {}). Retrying in {:?}...", 
                            operation_name, attempt, RETRY_DELAY
                        );
                        sleep(RETRY_DELAY).await;
                        continue;
                    }

                    return Err(lsp_error);
                }
            }
        }

        unreachable!("Loop should have returned")
    }

    /// Check if this is a semantic operation that depends on indexing
    #[allow(dead_code)]
    fn is_semantic_operation(operation_name: &str) -> bool {
        matches!(
            operation_name,
            "hover"
                | "completion"
                | "goto_definition"
                | "find_references"
                | "rename"
                | "code_actions"
                | "signature_help"
        )
    }

    /// Check if the error suggests rust-analyzer isn't ready yet  
    #[allow(dead_code)]
    fn is_readiness_issue(error: &str) -> bool {
        error.contains("No hover information available")
            || error.contains("No completions available")
            || error.contains("not available")
            || error.contains("analysis not ready")
            || error.contains("indexing")
            || error.contains("not ready")
            || error.contains("still loading")
            || error.contains("No ") // Generic "No X available" pattern
    }

    /// Log timing information for tool calls
    fn log_tool_timing(&self, tool_name: &str, start_time: Instant, result: &str) {
        let elapsed = start_time.elapsed();
        let since_test_start = self.test_start_time.elapsed();
        info!(
            "🕐 TIMING: +{:?} {}ms {}: {}",
            since_test_start,
            elapsed.as_millis(),
            tool_name,
            result
        );
    }

    #[tool(description = "Get type information and documentation at a specific position")]
    pub async fn hover(
        &self,
        Parameters(request): Parameters<HoverRequest>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = Instant::now();
        let position = Position {
            line: request.line,
            column: request.column,
        };
        let result = self.rust_analyzer.hover(&request.file_path, position).await;

        match result {
            Ok(Some(hover_info)) => {
                self.log_tool_timing("hover", start_time, "ok");
                Ok(CallToolResult::success(vec![Content::text(
                    hover_info.content,
                )]))
            }
            Ok(None) => {
                self.log_tool_timing("hover", start_time, "no_info");
                Ok(CallToolResult::success(vec![Content::text(
                    "No hover information available".to_string(),
                )]))
            }
            Err(e) => {
                // If we exhausted retries with a semantic readiness issue, return a helpful result instead of error
                if e.is_informational() {
                    self.log_tool_timing("hover", start_time, "readiness_issue");
                    Ok(CallToolResult::success(vec![Content::text(e.to_string())]))
                } else {
                    let error_type = if e.to_string().contains("content modified") {
                        "content_modified_error"
                    } else {
                        "error"
                    };
                    self.log_tool_timing("hover", start_time, error_type);
                    Err(e.to_mcp_error())
                }
            }
        }
    }

    #[tool(description = "Get code completions at a specific position")]
    pub async fn completion(
        &self,
        Parameters(request): Parameters<CompletionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = Instant::now();
        let position = Position {
            line: request.line,
            column: request.column,
        };
        let result = self
            .rust_analyzer
            .completion(&request.file_path, position)
            .await;

        match result {
            Ok(completions) => {
                if completions.is_empty() {
                    self.log_tool_timing("completion", start_time, "no_completions");
                    Ok(CallToolResult::success(vec![Content::text(
                        "No completions available".to_string(),
                    )]))
                } else {
                    let formatted_completions: Vec<String> = completions
                        .into_iter()
                        .map(|item| {
                            format!(
                                "- {} [{}]: {}",
                                item.label,
                                item.kind,
                                item.detail.unwrap_or_default()
                            )
                        })
                        .collect();
                    let content = format!("Completions:\n{}", formatted_completions.join("\n"));
                    self.log_tool_timing("completion", start_time, "ok");
                    Ok(CallToolResult::success(vec![Content::text(content)]))
                }
            }
            Err(e) => {
                // If we exhausted retries with a semantic readiness issue, return a helpful result instead of error
                if e.is_informational() {
                    self.log_tool_timing("completion", start_time, "readiness_issue");
                    Ok(CallToolResult::success(vec![Content::text(e.to_string())]))
                } else {
                    let error_type = if e.to_string().contains("content modified") {
                        "content_modified_error"
                    } else {
                        "error"
                    };
                    self.log_tool_timing("completion", start_time, error_type);
                    Err(e.to_mcp_error())
                }
            }
        }
    }

    #[tool(description = "Get compile errors and warnings for a file")]
    pub async fn diagnostics(
        &self,
        Parameters(request): Parameters<DiagnosticsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = Instant::now();
        let result = self.rust_analyzer.diagnostics(&request.file_path).await;

        match result {
            Ok(diagnostics) => {
                if diagnostics.is_empty() {
                    self.log_tool_timing("diagnostics", start_time, "ok");
                    Ok(CallToolResult::success(vec![Content::text(
                        "No diagnostics found".to_string(),
                    )]))
                } else {
                    let formatted_diagnostics: Vec<String> = diagnostics
                        .into_iter()
                        .map(|diag| {
                            let severity = match diag.severity {
                                crate::domain::DiagnosticSeverity::Error => "ERROR",
                                crate::domain::DiagnosticSeverity::Warning => "WARNING",
                                crate::domain::DiagnosticSeverity::Information => "INFO",
                                crate::domain::DiagnosticSeverity::Hint => "HINT",
                            };
                            format!(
                                "{}:{}-{}: {}: {}",
                                diag.range.start.line + 1,
                                diag.range.start.column + 1,
                                diag.range.end.column + 1,
                                severity,
                                diag.message
                            )
                        })
                        .collect();
                    let content = format!(
                        "Diagnostics ({}):\n{}",
                        formatted_diagnostics.len(),
                        formatted_diagnostics.join("\n")
                    );
                    self.log_tool_timing("diagnostics", start_time, "ok");
                    Ok(CallToolResult::success(vec![Content::text(content)]))
                }
            }
            Err(e) => {
                let error_type = if e.to_string().contains("content modified") {
                    "content_modified_error"
                } else {
                    "error"
                };
                self.log_tool_timing("diagnostics", start_time, error_type);
                Err(e.to_mcp_error())
            }
        }
    }

    #[tool(description = "Find definition of symbol at position")]
    pub async fn goto_definition(
        &self,
        Parameters(request): Parameters<GotoDefinitionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let start_time = Instant::now();
        let position = Position {
            line: request.line,
            column: request.column,
        };
        let result = self
            .rust_analyzer
            .goto_definition(&request.file_path, position)
            .await;

        match result {
            Ok(locations) => {
                if locations.is_empty() {
                    self.log_tool_timing("goto_definition", start_time, "no_definitions");
                    Ok(CallToolResult::success(vec![Content::text(
                        "No definition found".to_string(),
                    )]))
                } else {
                    let formatted_locations: Vec<String> = locations
                        .into_iter()
                        .map(|loc| {
                            format!(
                                "{}:{}:{}",
                                loc.file_path,
                                loc.range.start.line + 1,
                                loc.range.start.column + 1
                            )
                        })
                        .collect();
                    let content = format!("Definition(s):\n{}", formatted_locations.join("\n"));
                    self.log_tool_timing("goto_definition", start_time, "ok");
                    Ok(CallToolResult::success(vec![Content::text(content)]))
                }
            }
            Err(e) => {
                let error_type = if e.to_string().contains("content modified") {
                    "content_modified_error"
                } else {
                    "error"
                };
                self.log_tool_timing("goto_definition", start_time, error_type);
                Err(e.to_mcp_error())
            }
        }
    }

    #[tool(description = "Find all references to symbol at position")]
    pub async fn find_references(
        &self,
        Parameters(request): Parameters<FindReferencesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .find_references(&request.file_path, position, request.include_declaration)
            .await
        {
            Ok(locations) => {
                if locations.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No references found",
                    )]))
                } else {
                    let references_text = locations
                        .into_iter()
                        .map(|loc| {
                            format!(
                                "Reference at: {}:{}:{}",
                                loc.file_path,
                                loc.range.start.line + 1,
                                loc.range.start.column + 1
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found references:\n{references_text}"
                    ))]))
                }
            }
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Format Rust code")]
    pub async fn format_document(
        &self,
        Parameters(request): Parameters<FormatRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.rust_analyzer.format_document(&request.file_path).await {
            Ok(Some(message)) => Ok(CallToolResult::success(vec![Content::text(message)])),
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No formatting changes needed",
            )])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Rename symbols across the entire workspace safely")]
    pub async fn rename(
        &self,
        Parameters(request): Parameters<RenameRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };
        match self
            .rust_analyzer
            .rename(&request.file_path, position, &request.new_name)
            .await
        {
            Ok(message) => Ok(CallToolResult::success(vec![Content::text(message)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get available quick fixes and refactorings")]
    pub async fn code_actions(
        &self,
        Parameters(request): Parameters<CodeActionsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .code_actions(&request.file_path, position)
            .await
        {
            Ok(message) => Ok(CallToolResult::success(vec![Content::text(message)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Search for symbols across entire workspace")]
    pub async fn workspace_symbols(
        &self,
        Parameters(request): Parameters<WorkspaceSymbolsRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.rust_analyzer.workspace_symbols(&request.query).await {
            Ok(symbols) => {
                if symbols.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No symbols found matching the query",
                    )]))
                } else {
                    let symbols_text = symbols
                        .into_iter()
                        .map(|symbol| {
                            format!(
                                "• {} [{:?}]: {}:{}:{}",
                                symbol.name,
                                symbol.kind,
                                symbol.location.file_path,
                                symbol.location.range.start.line + 1,
                                symbol.location.range.start.column + 1
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found symbols:\n{symbols_text}"
                    ))]))
                }
            }
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get type and parameter hints")]
    pub async fn inlay_hints(
        &self,
        Parameters(request): Parameters<InlayHintsRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.rust_analyzer.inlay_hints(&request.file_path).await {
            Ok(hints_text) => Ok(CallToolResult::success(vec![Content::text(hints_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Expand Rust macros to see generated code")]
    pub async fn expand_macro(
        &self,
        Parameters(request): Parameters<ExpandMacroRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .expand_macro(&request.file_path, position)
            .await
        {
            Ok(expansion_text) => Ok(CallToolResult::success(vec![Content::text(expansion_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get document structure and symbols for code analysis")]
    pub async fn document_symbols(
        &self,
        Parameters(request): Parameters<DocumentSymbolsRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .rust_analyzer
            .document_symbols(&request.file_path)
            .await
        {
            Ok(symbols) => {
                let symbols_text = symbols
                    .into_iter()
                    .map(|symbol| {
                        format!(
                            "• {} [{:?}] at line {}",
                            symbol.name,
                            symbol.kind,
                            symbol.location.range.start.line + 1
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                if symbols_text.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No symbols found in document",
                    )]))
                } else {
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Document symbols:\n{symbols_text}"
                    ))]))
                }
            }
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get function signature help for parameter assistance")]
    pub async fn signature_help(
        &self,
        Parameters(request): Parameters<SignatureHelpRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .signature_help(&request.file_path, position)
            .await
        {
            Ok(help_text) => Ok(CallToolResult::success(vec![Content::text(help_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Highlight all occurrences of symbol at position")]
    pub async fn document_highlight(
        &self,
        Parameters(request): Parameters<DocumentHighlightRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .document_highlight(&request.file_path, position)
            .await
        {
            Ok(highlight_text) => Ok(CallToolResult::success(vec![Content::text(highlight_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get smart selection ranges for code expansion")]
    pub async fn selection_range(
        &self,
        Parameters(request): Parameters<SelectionRangeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let positions: Vec<Position> = request
            .positions
            .into_iter()
            .map(|pos| Position {
                line: pos.line,
                column: pos.column,
            })
            .collect();

        match self
            .rust_analyzer
            .selection_range(&request.file_path, positions)
            .await
        {
            Ok(ranges_text) => Ok(CallToolResult::success(vec![Content::text(ranges_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(
        description = "Find runnable items (tests, benchmarks, executables) with cargo commands"
    )]
    pub async fn runnables(
        &self,
        Parameters(request): Parameters<RunnablesRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.rust_analyzer.runnables(&request.file_path).await {
            Ok(runnables_text) => Ok(CallToolResult::success(vec![Content::text(runnables_text)])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Find all implementations of a trait at the given position")]
    pub async fn implementations(
        &self,
        Parameters(request): Parameters<ImplementationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let position = Position {
            line: request.line,
            column: request.column,
        };

        match self
            .rust_analyzer
            .implementations(&request.file_path, position)
            .await
        {
            Ok(implementations_text) => Ok(CallToolResult::success(vec![Content::text(
                implementations_text,
            )])),
            Err(e) => Err(e.to_mcp_error()),
        }
    }

    #[tool(description = "Get rust-analyzer LSP status")]
    pub async fn lsp_status(
        &self,
        Parameters(_request): Parameters<LspClientStatusRequest>,
    ) -> Result<CallToolResult, McpError> {
        // Get status information from domain service
        let (is_ready, opened_count, (memory_mb_val, _doc_count)) =
            self.rust_analyzer.get_lsp_status().await;
        let memory_mb = if memory_mb_val > 0 {
            Some(memory_mb_val)
        } else {
            None
        };

        let mut status_info = vec![
            format!(
                "rust-analyzer Status: {}",
                if is_ready { "Ready" } else { "Initializing" }
            ),
            format!("Opened Documents: {opened_count} files"),
            format!("Workspace Root: {:?}", self.workspace_root),
        ];

        // Add memory usage information
        if let Some(memory) = memory_mb {
            status_info.push(format!("Memory Usage: {memory}MB"));
        } else {
            status_info.push(
                "Memory Usage: Unable to monitor (rust-analyzer process not found)".to_string(),
            );
        }

        // Add memory optimization suggestion if many documents are open
        if opened_count > 20 {
            status_info.push("\n[Warning] Many documents are open in rust-analyzer.".to_string());
            status_info.push("This may cause high memory usage and slow responses.".to_string());
            status_info
                .push("Consider using 'close_unused_documents' action if available.".to_string());
        } else if opened_count > 10 {
            status_info.push("\n[Info] Moderate number of documents open.".to_string());
            status_info.push("Monitor memory usage if working with large files.".to_string());
        }

        // Add configuration info
        status_info.push("\nConfiguration:".to_string());
        status_info.push(format!("- Timeout: {}s", lsp_client::get_timeout_secs()));
        status_info.push(format!(
            "- Max Response Size: {}KB",
            lsp_client::get_max_response_size() / 1024
        ));
        status_info.push(format!(
            "- Max Symbols per Page: {}",
            lsp_client::MAX_SYMBOLS_COUNT
        ));

        Ok(CallToolResult::success(vec![Content::text(
            status_info.join("\n"),
        )]))
    }

    #[tool(
        description = "Close a document in rust-analyzer to free memory and reduce resource usage"
    )]
    pub async fn close_document(
        &self,
        Parameters(request): Parameters<CloseDocumentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let (opened_count_before, opened_count_after, close_result): (
            usize,
            usize,
            Result<(), String>,
        ) = match self.rust_analyzer.close_document(&request.file_path).await {
            Ok((before, after)) => (before, after, Ok(())),
            Err(e) => {
                // In case of error, we can't get the counts, so use 0 and return the error message
                let error_msg = format!("Failed to close document: {}", e);
                return Err(McpError::internal_error(error_msg, None));
            }
        };

        match close_result {
            Ok(()) => {
                let result_msg = if opened_count_after < opened_count_before {
                    format!(
                        "Document closed successfully: {}\nOpened documents: {} → {} (-{} document)",
                        request.file_path,
                        opened_count_before,
                        opened_count_after,
                        opened_count_before - opened_count_after
                    )
                } else {
                    format!(
                        "Document close request sent: {}\nOpened documents: {} (no change - document may not have been opened)",
                        request.file_path,
                        opened_count_after
                    )
                };

                Ok(CallToolResult::success(vec![Content::text(result_msg)]))
            }
            Err(error_msg) => Err(McpError::internal_error(error_msg, None)),
        }
    }
}

#[tool_handler]
impl ServerHandler for RustAnalyzerMCP {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some("This server provides rust-analyzer functionality through MCP tools. Available tools: 'hover' for type information, 'completion' for code completions, 'diagnostics' for compile errors, 'goto_definition' to find definitions, 'find_references' to find all references, 'format_document' to format code, 'rename' to rename symbols across the workspace, 'code_actions' to get quick fixes and refactorings, 'workspace_symbols' to search symbols across the workspace, 'inlay_hints' to get type and parameter hints, 'expand_macro' to expand Rust macros, 'document_symbols' for code structure analysis, 'signature_help' for function parameter assistance, 'document_highlight' for symbol occurrence highlighting, 'selection_range' for smart selection expansion, 'runnables' to find tests, benchmarks, and executables, 'implementations' to find all implementations of a trait, 'lsp_status' for monitoring rust-analyzer status, and 'close_document' for closing documents to free memory.".to_string()),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}
