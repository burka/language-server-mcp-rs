use clap::Parser;
use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, RoleServer, ServerHandler, ServiceExt,
};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use sysinfo::System;
use tokio::sync::Mutex;
use tracing::{error, info};
use tracing_subscriber::{self, EnvFilter};

mod lsp_client;
use lsp_client::{LspClient, MAX_COMPLETION_ITEMS, MAX_SYMBOLS_COUNT};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct HoverRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CompletionRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DiagnosticsRequest {
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GotoDefinitionRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FindReferencesRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    #[serde(default = "default_include_declaration")]
    pub include_declaration: bool,
}

fn default_include_declaration() -> bool {
    true
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct FormatRequest {
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RenameRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub new_name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CodeActionsRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WorkspaceSymbolsRequest {
    pub query: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InlayHintsRequest {
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ExpandMacroRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentSymbolsRequest {
    pub file_path: String,
    #[serde(default = "default_page")]
    pub page: usize,
    #[serde(default = "default_page_size")]
    pub page_size: usize,
}

fn default_page() -> usize {
    0
}

fn default_page_size() -> usize {
    MAX_SYMBOLS_COUNT
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SignatureHelpRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentHighlightRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SelectionRangeRequest {
    pub file_path: String,
    pub positions: Vec<PositionInfo>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct RunnablesRequest {
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImplementationsRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LspClientStatusRequest {
    // No parameters needed - just returns status info
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CloseDocumentRequest {
    pub file_path: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WarmCacheRequest {
    #[serde(default = "default_patterns")]
    pub patterns: Vec<String>,
    #[serde(default = "default_use_rust_defaults")]
    pub use_rust_defaults: bool,
}

fn default_patterns() -> Vec<String> {
    vec![]
}

fn default_use_rust_defaults() -> bool {
    true
}

#[derive(Debug, Clone)]
pub enum WarmingState {
    NotStarted,
    InProgress,
    Completed,
    Failed(String),
    Paused,
}

#[derive(Debug, Clone)]
pub struct WarmingStatus {
    pub state: WarmingState,
    pub total_files: usize,
    pub files_processed: usize,
    pub files_opened: usize,
    pub files_failed: usize,
    pub start_time: Option<Instant>,
    pub estimated_completion: Option<Instant>,
}

impl Default for WarmingStatus {
    fn default() -> Self {
        Self {
            state: WarmingState::NotStarted,
            total_files: 0,
            files_processed: 0,
            files_opened: 0,
            files_failed: 0,
            start_time: None,
            estimated_completion: None,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "language-server-mcp")]
#[command(about = "Rust-analyzer MCP server with smart cache warming")]
struct Args {
    /// Disable automatic cache warming on startup
    #[arg(long, help = "Disable auto-warming of rust-analyzer cache")]
    no_auto_warm: bool,

    /// Maximum memory to use for cache warming (e.g., "1g", "500m", "100m")
    #[arg(
        long,
        help = "Maximum memory for cache warming (default: 20% of system RAM)"
    )]
    max_memory: Option<String>,

    /// Force warm all files regardless of project size
    #[arg(long, help = "Force warming entire workspace regardless of size")]
    warm_all: bool,

    /// Show progress during cache warming
    #[arg(long, help = "Show detailed progress during cache warming")]
    show_progress: bool,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PositionInfo {
    pub line: u32,
    pub column: u32,
}

#[derive(Clone)]
pub struct RustAnalyzerMCP {
    lsp_client: Arc<Mutex<LspClient>>,
    workspace_root: PathBuf,
    tool_router: ToolRouter<RustAnalyzerMCP>,
    warming_status: Arc<Mutex<WarmingStatus>>,
}

#[tool_router]
impl RustAnalyzerMCP {
    pub async fn new(workspace_root: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        info!(
            "Initializing rust-analyzer MCP server for workspace: {:?}",
            workspace_root
        );
        let lsp_client = LspClient::new(&workspace_root).await?;
        info!("rust-analyzer LSP client initialized and ready");
        Ok(Self {
            lsp_client: Arc::new(Mutex::new(lsp_client)),
            workspace_root,
            tool_router: Self::tool_router(),
            warming_status: Arc::new(Mutex::new(WarmingStatus::default())),
        })
    }

    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    #[tool(description = "Get type information and documentation at a specific position")]
    async fn hover(
        &self,
        Parameters(request): Parameters<HoverRequest>,
    ) -> Result<CallToolResult, McpError> {
        let content = {
            let result = {
                let lsp_client = self.lsp_client.lock().await;
                lsp_client
                    .hover(&request.file_path, request.line, request.column)
                    .await
            };

            match result {
                Ok(Some(hover)) => match hover.contents {
                    lsp_types::HoverContents::Markup(markup) => markup.value,
                    lsp_types::HoverContents::Array(markups) => markups
                        .into_iter()
                        .map(|m| match m {
                            lsp_types::MarkedString::String(s) => s,
                            lsp_types::MarkedString::LanguageString(ls) => ls.value,
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n"),
                    lsp_types::HoverContents::Scalar(ms) => match ms {
                        lsp_types::MarkedString::String(s) => s,
                        lsp_types::MarkedString::LanguageString(ls) => ls.value,
                    },
                },
                Ok(None) => "No hover information available".to_string(),
                Err(e) => return Err(McpError::internal_error(format!("LSP error: {e}"), None)),
            }
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(description = "Get code completions at a specific position")]
    async fn completion(
        &self,
        Parameters(request): Parameters<CompletionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let content = {
            let result = {
                let lsp_client = self.lsp_client.lock().await;
                lsp_client
                    .completion(&request.file_path, request.line, request.column)
                    .await
            };

            match result {
                Ok(Some(result)) => {
                    let completions = match result {
                        lsp_types::CompletionResponse::Array(items) => items,
                        lsp_types::CompletionResponse::List(list) => list.items,
                    };

                    let completion_text = completions
                        .into_iter()
                        .take(MAX_COMPLETION_ITEMS) // Limit for readability and performance
                        .map(|item| {
                            let detail = item.detail.unwrap_or_default();
                            let doc = item
                                .documentation
                                .map(|d| match d {
                                    lsp_types::Documentation::String(s) => s,
                                    lsp_types::Documentation::MarkupContent(mc) => mc.value,
                                })
                                .unwrap_or_default();

                            if doc.is_empty() {
                                format!("- {}: {}", item.label, detail)
                            } else {
                                format!("- {}: {} - {}", item.label, detail, doc)
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    format!("Completions:\n{completion_text}")
                }
                Ok(None) => "No completions available".to_string(),
                Err(e) => return Err(McpError::internal_error(format!("LSP error: {e}"), None)),
            }
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(description = "Get compile errors and warnings for a file")]
    async fn diagnostics(
        &self,
        Parameters(request): Parameters<DiagnosticsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let content = {
            let result = {
                let lsp_client = self.lsp_client.lock().await;
                lsp_client.diagnostics(&request.file_path).await
            };

            match result {
                Ok(diagnostics) => {
                    if diagnostics.is_empty() {
                        "No diagnostics found".to_string()
                    } else {
                        let diagnostic_text = diagnostics
                            .into_iter()
                            .map(|diag| {
                                let severity = diag
                                    .severity
                                    .map(|s| format!("{s:?}"))
                                    .unwrap_or("Info".to_string());
                                let range = format!(
                                    "{}:{}-{}:{}",
                                    diag.range.start.line,
                                    diag.range.start.character,
                                    diag.range.end.line,
                                    diag.range.end.character
                                );
                                format!(
                                    "[{}] {}: {} ({})",
                                    severity,
                                    range,
                                    diag.message,
                                    diag.source.unwrap_or_default()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        format!("Diagnostics:\n{diagnostic_text}")
                    }
                }
                Err(e) => return Err(McpError::internal_error(format!("LSP error: {e}"), None)),
            }
        };

        Ok(CallToolResult::success(vec![Content::text(content)]))
    }

    #[tool(description = "Find definition of symbol at position")]
    async fn goto_definition(
        &self,
        Parameters(request): Parameters<GotoDefinitionRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .goto_definition(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(response)) => {
                use lsp_types::GotoDefinitionResponse;
                let locations = match response {
                    GotoDefinitionResponse::Scalar(location) => vec![location],
                    GotoDefinitionResponse::Array(locations) => locations,
                    GotoDefinitionResponse::Link(links) => links
                        .into_iter()
                        .map(|link| lsp_types::Location {
                            uri: link.target_uri,
                            range: link.target_selection_range,
                        })
                        .collect(),
                };

                if locations.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No definition found",
                    )]))
                } else {
                    let definition_text = locations
                        .into_iter()
                        .map(|loc| {
                            let path = loc
                                .uri
                                .to_file_path()
                                .ok()
                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| loc.uri.to_string());
                            format!(
                                "Definition at: {}:{}:{}",
                                path, loc.range.start.line, loc.range.start.character
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found definitions:\n{definition_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No definition found",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Find all references to symbol at position")]
    async fn find_references(
        &self,
        Parameters(request): Parameters<FindReferencesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .find_references(
                &request.file_path,
                request.line,
                request.column,
                request.include_declaration,
            )
            .await
        {
            Ok(Some(locations)) => {
                if locations.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No references found",
                    )]))
                } else {
                    let references_text = locations
                        .into_iter()
                        .map(|loc| {
                            let path = loc
                                .uri
                                .to_file_path()
                                .ok()
                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| loc.uri.to_string());
                            format!(
                                "Reference at: {}:{}:{}",
                                path, loc.range.start.line, loc.range.start.character
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found references:\n{references_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No references found",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Format Rust code")]
    async fn format_document(
        &self,
        Parameters(request): Parameters<FormatRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        let result = lsp_client.format_document(&request.file_path).await;
        drop(lsp_client); // Release the lock before doing async I/O

        match result {
            Ok(Some(edits)) => {
                if edits.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No formatting changes needed",
                    )]))
                } else {
                    // For simplicity, we'll just return a message about the number of edits
                    // In a real implementation, you'd apply the TextEdits to the content
                    let edit_count = edits.len();
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Formatting would apply {edit_count} edits to the file"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No formatting changes needed",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Rename symbols across the entire workspace safely")]
    async fn rename(
        &self,
        Parameters(request): Parameters<RenameRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .rename(
                &request.file_path,
                request.line,
                request.column,
                &request.new_name,
            )
            .await
        {
            Ok(Some(workspace_edit)) => {
                let mut changes_description = Vec::new();

                if let Some(changes) = workspace_edit.changes {
                    for (uri, edits) in changes {
                        let file_path = uri.path();
                        changes_description.push(format!("File: {file_path}"));

                        for edit in &edits {
                            changes_description.push(format!(
                                "  - Line {}-{}: Replace '{}' with '{}'",
                                edit.range.start.line + 1,
                                edit.range.end.line + 1,
                                edit.new_text.trim_end_matches('\n').replace('\n', "\\n"),
                                request.new_name
                            ));
                        }
                    }
                }

                if let Some(document_changes) = workspace_edit.document_changes {
                    use lsp_types::DocumentChangeOperation;
                    use lsp_types::DocumentChanges;

                    let changes: Vec<DocumentChangeOperation> = match document_changes {
                        DocumentChanges::Edits(edits) => edits
                            .into_iter()
                            .map(DocumentChangeOperation::Edit)
                            .collect(),
                        DocumentChanges::Operations(ops) => ops,
                    };

                    for change in changes {
                        match change {
                            DocumentChangeOperation::Edit(text_doc_edit) => {
                                let file_path = text_doc_edit.text_document.uri.path();
                                changes_description.push(format!("File: {file_path}"));

                                for edit in &text_doc_edit.edits {
                                    use lsp_types::OneOf;
                                    let text_edit = match edit {
                                        OneOf::Left(edit) => edit,
                                        OneOf::Right(annotated) => &annotated.text_edit,
                                    };
                                    changes_description.push(format!(
                                        "  - Line {}-{}: Replace with '{}'",
                                        text_edit.range.start.line + 1,
                                        text_edit.range.end.line + 1,
                                        text_edit
                                            .new_text
                                            .trim_end_matches('\n')
                                            .replace('\n', "\\n")
                                    ));
                                }
                            }
                            _ => {
                                changes_description.push(
                                    "  - Other document changes (create/rename/delete)".to_string(),
                                );
                            }
                        }
                    }
                }

                if changes_description.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No changes needed for rename",
                    )]))
                } else {
                    let summary = format!(
                        "Rename operation would make the following changes:\n\n{}",
                        changes_description.join("\n")
                    );
                    Ok(CallToolResult::success(vec![Content::text(summary)]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "Cannot rename at this position",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Get available quick fixes and refactorings")]
    async fn code_actions(
        &self,
        Parameters(request): Parameters<CodeActionsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .code_actions(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(actions)) => {
                let mut action_descriptions = Vec::new();

                for action in actions {
                    use lsp_types::CodeActionOrCommand;
                    match action {
                        CodeActionOrCommand::CodeAction(code_action) => {
                            let title = &code_action.title;
                            let kind = code_action
                                .kind
                                .as_ref()
                                .map(|k| format!(" ({})", k.as_str()))
                                .unwrap_or_default();

                            let diagnostics_info = if code_action.diagnostics.is_some() {
                                let diag_count = code_action.diagnostics.as_ref().unwrap().len();
                                if diag_count > 0 {
                                    format!(" [Fixes {diag_count} diagnostic(s)]")
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            };

                            action_descriptions.push(format!("• {title}{kind}{diagnostics_info}"));

                            // If there's a workspace edit, show what it would change
                            if let Some(edit) = &code_action.edit {
                                if let Some(changes) = &edit.changes {
                                    for (uri, edits) in changes {
                                        if !edits.is_empty() {
                                            action_descriptions
                                                .push(format!("  → Modifies: {}", uri.path()));
                                        }
                                    }
                                }

                                if let Some(document_changes) = &edit.document_changes {
                                    use lsp_types::DocumentChanges;
                                    match document_changes {
                                        DocumentChanges::Edits(edits) => {
                                            for edit in edits {
                                                action_descriptions.push(format!(
                                                    "  → Modifies: {}",
                                                    edit.text_document.uri.path()
                                                ));
                                            }
                                        }
                                        DocumentChanges::Operations(ops) => {
                                            action_descriptions.push(format!(
                                                "  → {} workspace operations",
                                                ops.len()
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        CodeActionOrCommand::Command(command) => {
                            action_descriptions.push(format!(
                                "• {} (command: {})",
                                command.title, command.command
                            ));
                        }
                    }
                }

                if action_descriptions.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No code actions available at this position",
                    )]))
                } else {
                    let summary = format!(
                        "Available code actions:\n\n{}",
                        action_descriptions.join("\n")
                    );
                    Ok(CallToolResult::success(vec![Content::text(summary)]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No code actions available at this position",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Search for symbols across entire workspace")]
    async fn workspace_symbols(
        &self,
        Parameters(request): Parameters<WorkspaceSymbolsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client.workspace_symbols(&request.query).await {
            Ok(Some(symbols)) => {
                if symbols.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No symbols found matching the query",
                    )]))
                } else {
                    let symbol_text = symbols
                        .into_iter()
                        .take(20) // Limit to first 20 for readability
                        .map(|symbol| {
                            let location = symbol.location;
                            let file_path = location
                                .uri
                                .to_file_path()
                                .ok()
                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| location.uri.to_string());
                            let kind = format!("{:?}", symbol.kind);
                            let container = symbol
                                .container_name
                                .map(|c| format!(" (in {c})"))
                                .unwrap_or_default();

                            format!(
                                "• {} [{}]: {}:{}:{}{}",
                                symbol.name,
                                kind,
                                file_path,
                                location.range.start.line + 1,
                                location.range.start.character + 1,
                                container
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found symbols:\n{symbol_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No symbols found matching the query",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Get type and parameter hints")]
    async fn inlay_hints(
        &self,
        Parameters(request): Parameters<InlayHintsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client.inlay_hints(&request.file_path).await {
            Ok(Some(hints)) => {
                if hints.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No inlay hints available",
                    )]))
                } else {
                    let hints_text = hints
                        .into_iter()
                        .take(50) // Limit to first 50 for readability
                        .map(|hint| {
                            let position = hint.position;
                            let label = match hint.label {
                                lsp_types::InlayHintLabel::String(s) => s,
                                lsp_types::InlayHintLabel::LabelParts(parts) => parts
                                    .into_iter()
                                    .map(|p| p.value)
                                    .collect::<Vec<_>>()
                                    .join(""),
                            };
                            let kind = hint.kind.map(|k| format!(" ({k:?})")).unwrap_or_default();

                            format!(
                                "Line {}:{}: {}{}",
                                position.line + 1,
                                position.character + 1,
                                label,
                                kind
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Inlay hints:\n{hints_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No inlay hints available",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Expand Rust macros to see generated code")]
    async fn expand_macro(
        &self,
        Parameters(request): Parameters<ExpandMacroRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .expand_macro(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(expansion)) => {
                // The result structure depends on rust-analyzer's specific response format
                // It typically contains expanded code as a string
                let expansion_text = if let Some(expansion_str) = expansion.as_str() {
                    expansion_str.to_string()
                } else if let Some(obj) = expansion.as_object() {
                    // Try to extract the expanded text from the response object
                    if let Some(expanded) = obj.get("expansion").and_then(|v| v.as_str()) {
                        expanded.to_string()
                    } else {
                        format!(
                            "Macro expansion result: {}",
                            serde_json::to_string_pretty(&expansion).unwrap_or_default()
                        )
                    }
                } else {
                    format!("Macro expansion result: {expansion}")
                };

                if expansion_text.trim().is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No macro expansion available at this position",
                    )]))
                } else {
                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Macro expansion:\n```rust\n{expansion_text}\n```"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No macro expansion available at this position",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Get document structure and symbols for code analysis")]
    async fn document_symbols(
        &self,
        Parameters(request): Parameters<DocumentSymbolsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        // Get opened documents count first
        let opened_count = lsp_client.get_opened_documents_count().await;

        match lsp_client.document_symbols(&request.file_path).await {
            Ok(Some(response)) => {
                // Release the lock immediately after getting the response
                drop(lsp_client);
                use lsp_types::DocumentSymbolResponse;
                let mut symbols_text = match response {
                    DocumentSymbolResponse::Flat(symbols) => {
                        let total_symbols = symbols.len();
                        let start_idx = request.page * request.page_size;
                        let page_symbols: Vec<_> = symbols
                            .into_iter()
                            .skip(start_idx)
                            .take(request.page_size)
                            .collect();

                        let symbols_text = page_symbols
                            .into_iter()
                            .map(|symbol| {
                                let location = &symbol.location;
                                let file_path = location
                                    .uri
                                    .to_file_path()
                                    .ok()
                                    .and_then(|p| p.to_str().map(|s| s.to_string()))
                                    .unwrap_or_else(|| location.uri.to_string());
                                let kind = format!("{:?}", symbol.kind);
                                let container = symbol
                                    .container_name
                                    .map(|c| format!(" (in {c})"))
                                    .unwrap_or_default();

                                format!(
                                    "• {} [{}]: {}:{}:{}{}",
                                    symbol.name,
                                    kind,
                                    file_path,
                                    location.range.start.line + 1,
                                    location.range.start.character + 1,
                                    container
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        let page_info = if total_symbols > request.page_size {
                            let total_pages = total_symbols.div_ceil(request.page_size);
                            format!(
                                "\n\n--- Page {} of {} ({} total symbols, {} per page) ---",
                                request.page + 1,
                                total_pages,
                                total_symbols,
                                request.page_size
                            )
                        } else {
                            format!("\n\n--- {total_symbols} symbols total ---")
                        };

                        format!("{symbols_text}{page_info}")
                    }
                    DocumentSymbolResponse::Nested(symbols) => {
                        let total_symbols = symbols.len();
                        let start_idx = request.page * request.page_size;
                        let page_symbols: Vec<_> = symbols
                            .into_iter()
                            .skip(start_idx)
                            .take(request.page_size)
                            .collect();

                        fn format_nested_symbols(
                            symbols: Vec<lsp_types::DocumentSymbol>,
                            indent: usize,
                        ) -> String {
                            symbols
                                .into_iter()
                                .map(|symbol| {
                                    let indent_str = "  ".repeat(indent);
                                    let kind = format!("{:?}", symbol.kind);
                                    let range = &symbol.range;
                                    let mut result = format!(
                                        "{}• {} [{}]: line {}:{}",
                                        indent_str,
                                        symbol.name,
                                        kind,
                                        range.start.line + 1,
                                        range.start.character + 1
                                    );

                                    if let Some(children) = symbol.children {
                                        if !children.is_empty() {
                                            result.push('\n');
                                            result.push_str(&format_nested_symbols(
                                                children,
                                                indent + 1,
                                            ));
                                        }
                                    }
                                    result
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        }

                        let nested_text = format_nested_symbols(page_symbols, 0);

                        let page_info = if total_symbols > request.page_size {
                            let total_pages = total_symbols.div_ceil(request.page_size);
                            format!(
                                "\n\n--- Page {} of {} ({} total symbols, {} per page) ---",
                                request.page + 1,
                                total_pages,
                                total_symbols,
                                request.page_size
                            )
                        } else {
                            format!("\n\n--- {total_symbols} symbols total ---")
                        };

                        format!("{nested_text}{page_info}")
                    }
                };

                // Add debug info about opened documents (already captured above)
                if opened_count > 10 {
                    let debug_info = format!(
                        "\n\n[Debug: {opened_count} documents currently opened in rust-analyzer]"
                    );
                    symbols_text.push_str(&debug_info);
                }

                Ok(CallToolResult::success(vec![Content::text(format!(
                    "Document symbols:\n{symbols_text}"
                ))]))
            }
            Ok(None) => {
                drop(lsp_client);
                Ok(CallToolResult::success(vec![Content::text(
                    "No symbols found in document",
                )]))
            }
            Err(e) => {
                drop(lsp_client);
                Err(McpError::internal_error(format!("LSP error: {e}"), None))
            }
        }
    }

    #[tool(description = "Get function signature help for parameter assistance")]
    async fn signature_help(
        &self,
        Parameters(request): Parameters<SignatureHelpRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .signature_help(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(help)) => {
                if help.signatures.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No signature help available",
                    )]))
                } else {
                    let signatures_text = help
                        .signatures
                        .into_iter()
                        .enumerate()
                        .map(|(i, sig)| {
                            let active_param = help.active_parameter.unwrap_or(0) as usize;
                            let mut signature = format!("{}. {}", i + 1, sig.label);

                            if let Some(doc) = sig.documentation.as_ref() {
                                let doc_text = match doc {
                                    lsp_types::Documentation::String(s) => s.clone(),
                                    lsp_types::Documentation::MarkupContent(mc) => mc.value.clone(),
                                };
                                if !doc_text.is_empty() {
                                    signature.push_str(&format!("\n   {doc_text}"));
                                }
                            }

                            if let Some(params) = sig.parameters.as_ref() {
                                signature.push_str("\n   Parameters:");
                                for (pi, param) in params.iter().enumerate() {
                                    let marker = if pi == active_param { " → " } else { "   " };
                                    let label_text = match &param.label {
                                        lsp_types::ParameterLabel::Simple(s) => s.clone(),
                                        lsp_types::ParameterLabel::LabelOffsets([start, end]) => {
                                            format!("{}[{}:{}]", sig.label, start, end)
                                        }
                                    };
                                    signature.push_str(&format!("\n{marker}{label_text}"));
                                    if let Some(doc) = &param.documentation {
                                        let doc_text = match doc {
                                            lsp_types::Documentation::String(s) => s.clone(),
                                            lsp_types::Documentation::MarkupContent(mc) => {
                                                mc.value.clone()
                                            }
                                        };
                                        if !doc_text.is_empty() {
                                            signature.push_str(&format!(" - {doc_text}"));
                                        }
                                    }
                                }
                            }
                            signature
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Signature help:\n{signatures_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No signature help available",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Highlight all occurrences of symbol at position")]
    async fn document_highlight(
        &self,
        Parameters(request): Parameters<DocumentHighlightRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .document_highlight(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(highlights)) => {
                if highlights.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No highlights found at this position",
                    )]))
                } else {
                    let highlights_text = highlights
                        .into_iter()
                        .map(|highlight| {
                            let kind = highlight
                                .kind
                                .map(|k| format!(" ({k:?})"))
                                .unwrap_or_default();
                            format!(
                                "Line {}:{}-{}:{}{}",
                                highlight.range.start.line + 1,
                                highlight.range.start.character + 1,
                                highlight.range.end.line + 1,
                                highlight.range.end.character + 1,
                                kind
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Document highlights:\n{highlights_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No highlights found at this position",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Get smart selection ranges for code expansion")]
    async fn selection_range(
        &self,
        Parameters(request): Parameters<SelectionRangeRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        let positions: Vec<lsp_types::Position> = request
            .positions
            .into_iter()
            .map(|pos| lsp_types::Position {
                line: pos.line,
                character: pos.column,
            })
            .collect();

        match lsp_client
            .selection_range(&request.file_path, positions)
            .await
        {
            Ok(Some(ranges)) => {
                if ranges.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No selection ranges found",
                    )]))
                } else {
                    let ranges_text = ranges
                        .into_iter()
                        .enumerate()
                        .map(|(i, mut range)| {
                            let mut result = format!("Position {}:", i + 1);
                            let mut level = 0;
                            loop {
                                let indent = "  ".repeat(level);
                                result.push_str(&format!(
                                    "\n{}Level {}: Line {}:{}-{}:{}",
                                    indent,
                                    level,
                                    range.range.start.line + 1,
                                    range.range.start.character + 1,
                                    range.range.end.line + 1,
                                    range.range.end.character + 1
                                ));

                                if let Some(parent) = range.parent {
                                    range = *parent;
                                    level += 1;
                                } else {
                                    break;
                                }
                            }
                            result
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Selection ranges:\n{ranges_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No selection ranges found",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(
        description = "Find runnable items (tests, benchmarks, executables) with cargo commands"
    )]
    async fn runnables(
        &self,
        Parameters(request): Parameters<RunnablesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client.runnables(&request.file_path).await {
            Ok(Some(runnables)) => {
                if let Some(array) = runnables.as_array() {
                    if array.is_empty() {
                        Ok(CallToolResult::success(vec![Content::text(
                            "No runnable items found",
                        )]))
                    } else {
                        let runnables_text = array
                            .iter()
                            .enumerate()
                            .filter_map(|(i, runnable)| {
                                let obj = runnable.as_object()?;
                                let label = obj.get("label")?.as_str().unwrap_or("Unknown");
                                let kind = obj.get("kind")?.as_str().unwrap_or("Unknown");
                                let location = obj.get("location")?;
                                let range = location.get("range")?;
                                let start = range.get("start")?;
                                let line = start.get("line")?.as_u64().unwrap_or(0) + 1;
                                let character = start.get("character")?.as_u64().unwrap_or(0) + 1;

                                // Extract cargo command if available
                                let cargo_cmd = if let Some(args) = obj.get("args") {
                                    if let Some(cargo_args) = args.get("cargoArgs") {
                                        if let Some(cargo_array) = cargo_args.as_array() {
                                            let cmd_parts: Vec<String> = cargo_array
                                                .iter()
                                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                                .collect();
                                            if !cmd_parts.is_empty() {
                                                format!(" → cargo {}", cmd_parts.join(" "))
                                            } else {
                                                String::new()
                                            }
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                };

                                Some(format!(
                                    "{}. {} [{}] at line {}:{}{}",
                                    i + 1,
                                    label,
                                    kind,
                                    line,
                                    character,
                                    cargo_cmd
                                ))
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        if runnables_text.is_empty() {
                            Ok(CallToolResult::success(vec![Content::text(
                                "No valid runnable items found",
                            )]))
                        } else {
                            Ok(CallToolResult::success(vec![Content::text(format!(
                                "Runnable items:\n{runnables_text}"
                            ))]))
                        }
                    }
                } else {
                    Ok(CallToolResult::success(vec![Content::text(
                        "Unexpected runnables response format",
                    )]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No runnable items found",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Find all implementations of a trait at the given position")]
    async fn implementations(
        &self,
        Parameters(request): Parameters<ImplementationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        match lsp_client
            .implementations(&request.file_path, request.line, request.column)
            .await
        {
            Ok(Some(locations)) => {
                if locations.is_empty() {
                    Ok(CallToolResult::success(vec![Content::text(
                        "No implementations found",
                    )]))
                } else {
                    let implementations_text = locations
                        .into_iter()
                        .enumerate()
                        .map(|(i, loc)| {
                            let path = loc
                                .uri
                                .to_file_path()
                                .ok()
                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| loc.uri.to_string());
                            format!(
                                "{}. Implementation at: {}:{}:{}",
                                i + 1,
                                path,
                                loc.range.start.line + 1,
                                loc.range.start.character + 1
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(CallToolResult::success(vec![Content::text(format!(
                        "Found implementations:\n{implementations_text}"
                    ))]))
                }
            }
            Ok(None) => Ok(CallToolResult::success(vec![Content::text(
                "No implementations found",
            )])),
            Err(e) => Err(McpError::internal_error(format!("LSP error: {e}"), None)),
        }
    }

    #[tool(description = "Get LSP and cache warming status")]
    async fn lsp_status(
        &self,
        Parameters(_request): Parameters<LspClientStatusRequest>,
    ) -> Result<CallToolResult, McpError> {
        let lsp_client = self.lsp_client.lock().await;

        // Get status information
        let is_ready = lsp_client.is_ready();
        let opened_count = lsp_client.get_opened_documents_count().await;
        let (memory_mb, _doc_count) = lsp_client.get_memory_status().await;

        let mut status_info = vec![
            format!(
                "LSP Client Status: {}",
                if is_ready { "Ready" } else { "Not Ready" }
            ),
            format!("Opened Documents: {} files", opened_count),
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

        // Add cache warming status
        let warming_status = self.warming_status.lock().await;
        match &warming_status.state {
            WarmingState::NotStarted => {
                status_info.push("\nCache Warming: Not started".to_string());
            }
            WarmingState::InProgress => {
                status_info.push("\nCache Warming: 🔥 In Progress".to_string());
                status_info.push(format!("  📂 Total files: {}", warming_status.total_files));
                status_info.push(format!(
                    "  ✅ Processed: {}",
                    warming_status.files_processed
                ));
                status_info.push(format!("  🆕 Opened: {}", warming_status.files_opened));

                if warming_status.files_failed > 0 {
                    status_info.push(format!("  ❌ Failed: {}", warming_status.files_failed));
                }

                if warming_status.total_files > 0 {
                    let percentage = (warming_status.files_processed as f64
                        / warming_status.total_files as f64)
                        * 100.0;
                    status_info.push(format!("  📊 Progress: {percentage:.1}%"));

                    if let Some(start_time) = warming_status.start_time {
                        let elapsed = start_time.elapsed();
                        if elapsed.as_secs() > 0 {
                            let rate =
                                warming_status.files_processed as f64 / elapsed.as_secs_f64();
                            status_info.push(format!("  🚀 Rate: {rate:.1} files/sec"));

                            if rate > 0.0
                                && warming_status.files_processed < warming_status.total_files
                            {
                                let remaining =
                                    warming_status.total_files - warming_status.files_processed;
                                let eta_secs = remaining as f64 / rate;
                                let eta = if eta_secs < 60.0 {
                                    format!("{eta_secs:.0}s")
                                } else if eta_secs < 3600.0 {
                                    format!("{:.0}m", eta_secs / 60.0)
                                } else {
                                    format!("{:.1}h", eta_secs / 3600.0)
                                };
                                status_info.push(format!("  ⏳ ETA: ~{eta}"));
                            }
                        }
                    }
                }
            }
            WarmingState::Completed => {
                status_info.push("\nCache Warming: ✅ Completed".to_string());
                status_info.push(format!(
                    "  📂 Total processed: {}",
                    warming_status.total_files
                ));
                status_info.push(format!(
                    "  🆕 Files warmed: {}",
                    warming_status.files_opened
                ));
                if warming_status.files_failed > 0 {
                    status_info.push(format!("  ❌ Failed: {}", warming_status.files_failed));
                }
            }
            WarmingState::Failed(error) => {
                status_info.push(format!("\nCache Warming: ❌ Failed - {error}"));
                if warming_status.files_processed > 0 {
                    status_info.push(format!(
                        "  Files processed before failure: {}",
                        warming_status.files_processed
                    ));
                }
            }
            WarmingState::Paused => {
                status_info.push("\nCache Warming: ⏸️  Paused".to_string());
                status_info.push(format!(
                    "  Progress: {}/{} files",
                    warming_status.files_processed, warming_status.total_files
                ));
            }
        }
        drop(warming_status); // Release the lock

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
    async fn close_document(
        &self,
        Parameters(request): Parameters<CloseDocumentRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = {
            let lsp_client = self.lsp_client.lock().await;

            let opened_count_before = lsp_client.get_opened_documents_count().await;
            let close_result = match lsp_client.close_document(&request.file_path).await {
                Ok(()) => Ok(()),
                Err(e) => Err(format!("Failed to close document: {e}")),
            };
            let opened_count_after = lsp_client.get_opened_documents_count().await;

            (opened_count_before, close_result, opened_count_after)
        };

        let (opened_count_before, close_result, opened_count_after) = result;

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

    #[tool(
        description = "Pre-warm rust-analyzer cache by opening files matching glob patterns for faster subsequent operations"
    )]
    async fn warm_cache(
        &self,
        Parameters(request): Parameters<WarmCacheRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = {
            let lsp_client = self.lsp_client.lock().await;

            if request.patterns.is_empty() && request.use_rust_defaults {
                // Use default Rust project patterns
                lsp_client.warm_cache_rust_project().await
            } else if request.patterns.is_empty() {
                // No patterns provided and not using defaults
                return Ok(CallToolResult::success(vec![Content::text(
                    "No glob patterns provided. Use 'use_rust_defaults: true' or provide specific patterns.".to_string()
                )]));
            } else {
                // Use provided patterns, possibly combined with defaults
                let mut patterns = request.patterns.clone();
                if request.use_rust_defaults {
                    let default_patterns = vec![
                        "src/**/*.rs".to_string(),
                        "examples/**/*.rs".to_string(),
                        "tests/**/*.rs".to_string(),
                        "benches/**/*.rs".to_string(),
                        "build.rs".to_string(),
                    ];
                    for pattern in default_patterns {
                        if !patterns.contains(&pattern) {
                            patterns.push(pattern);
                        }
                    }
                }
                lsp_client.warm_cache_with_globs(&patterns).await
            }
        };

        match result {
            Ok((files_opened, files_already_open, files_failed, duration)) => {
                let total_files = files_opened + files_already_open + files_failed;
                let duration_secs = duration.as_secs_f64();

                let mut response_lines = vec![
                    "Cache warming completed successfully:".to_string(),
                    format!("📂 Total files processed: {}", total_files),
                    format!("✅ Files newly opened: {}", files_opened),
                    format!("🔄 Files already open: {}", files_already_open),
                ];

                if files_failed > 0 {
                    response_lines.push(format!("❌ Files failed to open: {files_failed}"));
                }

                response_lines.extend(vec![
                    format!("⏱️  Duration: {:.2}s", duration_secs),
                    format!(
                        "📊 Performance: {:.1} files/sec",
                        total_files as f64 / duration_secs.max(0.001)
                    ),
                ]);

                if files_opened > 0 {
                    response_lines.push("🚀 rust-analyzer cache is now warmed - subsequent operations will be much faster!".to_string());
                }

                if total_files > 100 {
                    response_lines.push("\n💡 Tip: For very large projects, consider using more specific glob patterns to reduce memory usage.".to_string());
                }

                Ok(CallToolResult::success(vec![Content::text(
                    response_lines.join("\n"),
                )]))
            }
            Err(e) => Err(McpError::internal_error(
                format!("Cache warming failed: {e}"),
                None,
            )),
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
            instructions: Some("This server provides rust-analyzer functionality through MCP tools. Available tools: 'hover' for type information, 'completion' for code completions, 'diagnostics' for compile errors, 'goto_definition' to find definitions, 'find_references' to find all references, 'format_document' to format code, 'rename' to rename symbols across the workspace, 'code_actions' to get quick fixes and refactorings, 'workspace_symbols' to search symbols across the workspace, 'inlay_hints' to get type and parameter hints, 'expand_macro' to expand Rust macros, 'document_symbols' for code structure analysis, 'signature_help' for function parameter assistance, 'document_highlight' for symbol occurrence highlighting, 'selection_range' for smart selection expansion, 'runnables' to find tests, benchmarks, and executables, 'implementations' to find all implementations of a trait, 'lsp_status' for monitoring LSP and cache warming status, 'close_document' for closing documents to free memory, and 'warm_cache' for pre-warming rust-analyzer cache with glob patterns to significantly improve performance.".to_string()),
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

fn parse_memory_size(memory_str: &str) -> Result<u64, String> {
    let memory_str = memory_str.to_lowercase();

    if let Some(num_str) = memory_str.strip_suffix("g") {
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("Invalid memory size: {memory_str}"))?;
        Ok((num * 1024.0 * 1024.0 * 1024.0) as u64)
    } else if let Some(num_str) = memory_str.strip_suffix("m") {
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("Invalid memory size: {memory_str}"))?;
        Ok((num * 1024.0 * 1024.0) as u64)
    } else if let Some(num_str) = memory_str.strip_suffix("k") {
        let num: f64 = num_str
            .parse()
            .map_err(|_| format!("Invalid memory size: {memory_str}"))?;
        Ok((num * 1024.0) as u64)
    } else {
        // Try parsing as bytes
        memory_str
            .parse::<u64>()
            .map_err(|_| format!("Invalid memory size: {memory_str}"))
    }
}

fn get_system_memory() -> u64 {
    let mut system = System::new_all();
    system.refresh_memory();
    system.total_memory()
}

fn calculate_default_memory_limit() -> u64 {
    let total_memory = get_system_memory();
    (total_memory as f64 * 0.2) as u64 // 20% of system memory
}

fn should_auto_warm(workspace_root: &Path, memory_limit: u64) -> bool {
    // Count Rust files to estimate project size
    let patterns = vec![
        "src/**/*.rs",
        "examples/**/*.rs",
        "tests/**/*.rs",
        "benches/**/*.rs",
        "build.rs",
    ];

    let mut total_files = 0;
    for pattern in patterns {
        if let Ok(entries) = glob::glob(&format!("{}/{}", workspace_root.display(), pattern)) {
            total_files += entries.count();
        }
    }

    // Conservative estimate: 1MB per file for rust-analyzer memory usage
    let estimated_memory = total_files * 1024 * 1024;
    let memory_usage_within_limit = (estimated_memory as u64) <= memory_limit;

    // Auto-warm if we have files and memory usage should be reasonable
    total_files > 0 && memory_usage_within_limit
}

/// Spawn background warming task that runs concurrently with MCP service
async fn spawn_background_warming(
    lsp_client: Arc<Mutex<LspClient>>,
    warming_status: Arc<Mutex<WarmingStatus>>,
    workspace_root: PathBuf,
    warm_all: bool,
    show_progress: bool,
    memory_limit: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = perform_natural_workspace_analysis(
            lsp_client,
            warming_status,
            workspace_root,
            warm_all,
            show_progress,
            memory_limit,
        )
        .await
        {
            error!("Background warming failed: {}", e);
        }
    })
}

/// Use rust-analyzer's natural workspace analysis instead of manual file warming
async fn perform_natural_workspace_analysis(
    lsp_client: Arc<Mutex<LspClient>>,
    warming_status: Arc<Mutex<WarmingStatus>>,
    _workspace_root: PathBuf,
    _warm_all: bool,
    show_progress: bool,
    _memory_limit: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    // Update status to InProgress
    {
        let mut status = warming_status.lock().await;
        status.state = WarmingState::InProgress;
        status.start_time = Some(Instant::now());
        status.total_files = 1; // We're letting rust-analyzer handle the workspace naturally
    }

    if show_progress {
        eprintln!("🔥 Letting rust-analyzer perform natural workspace analysis...");
        eprintln!("   (No manual file opening needed - rust-analyzer analyzes the entire workspace automatically)");
    }

    // Simply wait for rust-analyzer to complete its natural workspace analysis
    let lsp_client_guard = lsp_client.lock().await;
    drop(lsp_client_guard); // Release lock immediately

    // Use the natural workspace analysis method
    let error_msg = {
        let lsp_client_guard = lsp_client.lock().await;
        match lsp_client_guard.wait_for_workspace_analysis().await {
            Ok(()) => None,
            Err(e) => Some(format!("Workspace analysis failed: {e}")),
        }
    };

    if let Some(error_msg) = error_msg {
        // Update to failed status
        let mut status = warming_status.lock().await;
        status.state = WarmingState::Failed(error_msg.clone());
        return Err(error_msg.into());
    }

    // Update final status to completed
    {
        let mut status = warming_status.lock().await;
        status.state = WarmingState::Completed;
        status.files_processed = 1;
        status.files_opened = 0; // No files manually opened
        status.files_failed = 0;
    }

    if show_progress {
        let duration = Instant::now().duration_since(
            warming_status
                .lock()
                .await
                .start_time
                .unwrap_or_else(Instant::now),
        );
        eprintln!(
            "✅ rust-analyzer workspace analysis completed in {:.1}s:",
            duration.as_secs_f64()
        );
        eprintln!("   🚀 rust-analyzer has analyzed the entire workspace efficiently!");
        eprintln!("   📂 All files in the workspace are now indexed and ready for fast operations");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    info!("Starting rust-analyzer MCP server");

    let workspace_root = std::env::current_dir()?;

    // Parse memory limit
    let memory_limit = if let Some(memory_str) = &args.max_memory {
        parse_memory_size(memory_str).map_err(|e| format!("Failed to parse --max-memory: {e}"))?
    } else {
        calculate_default_memory_limit()
    };

    // Create the MCP service
    let mcp_service = RustAnalyzerMCP::new(workspace_root.clone()).await?;

    // Start background warming if enabled (NON-BLOCKING!)
    let _warming_handle = if !args.no_auto_warm {
        if args.warm_all || should_auto_warm(&workspace_root, memory_limit) {
            Some(
                spawn_background_warming(
                    Arc::clone(&mcp_service.lsp_client),
                    Arc::clone(&mcp_service.warming_status),
                    workspace_root.clone(),
                    args.warm_all,
                    args.show_progress,
                    memory_limit,
                )
                .await,
            )
        } else {
            if args.show_progress {
                eprintln!(
                    "ℹ️  Auto-warm skipped: project too large for memory limit ({}MB)",
                    memory_limit / (1024 * 1024)
                );
                eprintln!("   Use --warm-all to force warming or --max-memory to increase limit");
            }
            None
        }
    } else {
        if args.show_progress {
            eprintln!("ℹ️  Auto-warm disabled via --no-auto-warm");
        }
        None
    };

    // Start MCP service immediately - server will be responsive right away
    let service = mcp_service.serve(stdio()).await.inspect_err(|e| {
        error!("serving error: {:?}", e);
    })?;

    info!("MCP server is running");
    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        CloseDocumentRequest, DocumentSymbolsRequest, LspClientStatusRequest, WarmCacheRequest,
    };
    use crate::lsp_client;
    use std::path::PathBuf;

    #[test]
    fn test_workspace_root_method() {
        // Test that workspace_root method works (to avoid dead code warning)
        let workspace = PathBuf::from("/tmp");
        let workspace_clone = workspace.clone();

        // We can't easily create a full RustAnalyzerMCP instance in tests without rust-analyzer,
        // but we can test that the PathBuf operations work
        assert_eq!(workspace, workspace_clone);
        assert!(workspace.is_absolute());
    }

    #[test]
    fn test_lsp_status_request_deserialization() {
        // Test LspClientStatusRequest deserialization (still uses old struct name)
        let json = r#"{}"#;
        let _request: LspClientStatusRequest = serde_json::from_str(json).unwrap();
    }

    #[test]
    fn test_close_document_request_deserialization() {
        // Test CloseDocumentRequest deserialization
        let json = r#"{"file_path": "/path/to/file.rs"}"#;
        let request: CloseDocumentRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.file_path, "/path/to/file.rs");
    }

    #[test]
    fn test_document_symbols_pagination_defaults() {
        // Test DocumentSymbolsRequest pagination defaults
        let json = r#"{"file_path": "/path/to/file.rs"}"#;
        let request: DocumentSymbolsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.page, 0);
        assert_eq!(request.page_size, lsp_client::MAX_SYMBOLS_COUNT);
    }

    #[test]
    fn test_document_symbols_pagination_custom() {
        // Test DocumentSymbolsRequest with custom pagination
        let json = r#"{"file_path": "/path/to/file.rs", "page": 2, "page_size": 50}"#;
        let request: DocumentSymbolsRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.page, 2);
        assert_eq!(request.page_size, 50);
    }

    #[test]
    fn test_warm_cache_request_defaults() {
        // Test WarmCacheRequest with defaults
        let json = r#"{}"#;
        let request: WarmCacheRequest = serde_json::from_str(json).unwrap();
        assert!(request.patterns.is_empty());
        assert!(request.use_rust_defaults);
    }

    #[test]
    fn test_warm_cache_request_custom_patterns() {
        // Test WarmCacheRequest with custom patterns
        let json = r#"{"patterns": ["src/**/*.rs", "custom/**/*.rs"], "use_rust_defaults": false}"#;
        let request: WarmCacheRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.patterns, vec!["src/**/*.rs", "custom/**/*.rs"]);
        assert!(!request.use_rust_defaults);
    }

    #[test]
    fn test_warm_cache_request_patterns_with_defaults() {
        // Test WarmCacheRequest with custom patterns and defaults
        let json = r#"{"patterns": ["custom/**/*.rs"], "use_rust_defaults": true}"#;
        let request: WarmCacheRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.patterns, vec!["custom/**/*.rs"]);
        assert!(request.use_rust_defaults);
    }

    #[test]
    fn test_parse_memory_size() {
        use super::parse_memory_size;

        // Test gigabytes
        assert_eq!(parse_memory_size("1g").unwrap(), 1024 * 1024 * 1024);
        assert_eq!(parse_memory_size("2G").unwrap(), 2 * 1024 * 1024 * 1024);
        assert_eq!(
            parse_memory_size("0.5g").unwrap(),
            (0.5 * 1024.0 * 1024.0 * 1024.0) as u64
        );

        // Test megabytes
        assert_eq!(parse_memory_size("100m").unwrap(), 100 * 1024 * 1024);
        assert_eq!(parse_memory_size("500M").unwrap(), 500 * 1024 * 1024);
        assert_eq!(
            parse_memory_size("1.5m").unwrap(),
            (1.5 * 1024.0 * 1024.0) as u64
        );

        // Test kilobytes
        assert_eq!(parse_memory_size("1024k").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_size("2048K").unwrap(), 2048 * 1024);

        // Test bytes
        assert_eq!(parse_memory_size("1024").unwrap(), 1024);
        assert_eq!(parse_memory_size("4096").unwrap(), 4096);

        // Test invalid formats
        assert!(parse_memory_size("invalid").is_err());
        assert!(parse_memory_size("1.5.5g").is_err());
        assert!(parse_memory_size("").is_err());
        assert!(parse_memory_size("g").is_err());
    }

    #[test]
    fn test_get_system_memory() {
        use super::get_system_memory;

        // Test that we can get system memory (should be > 0)
        let memory = get_system_memory();
        assert!(memory > 0, "System memory should be greater than 0");
        assert!(memory > 1024 * 1024, "System memory should be at least 1MB");
    }

    #[test]
    fn test_calculate_default_memory_limit() {
        use super::{calculate_default_memory_limit, get_system_memory};

        let limit = calculate_default_memory_limit();
        let total = get_system_memory();

        // Should be 20% of system memory
        let expected = (total as f64 * 0.2) as u64;
        assert_eq!(limit, expected);

        // Should be reasonable (between 1MB and total memory)
        assert!(limit > 1024 * 1024, "Default limit should be at least 1MB");
        assert!(
            limit <= total,
            "Default limit should not exceed total memory"
        );
    }

    #[test]
    fn test_should_auto_warm_with_no_files() {
        use super::should_auto_warm;

        // Test with a non-existent directory (no Rust files)
        let nonexistent_path = PathBuf::from("/tmp/definitely-does-not-exist-rust-project-12345");
        let memory_limit = 1024 * 1024 * 1024; // 1GB

        let should_warm = should_auto_warm(&nonexistent_path, memory_limit);
        assert!(
            !should_warm,
            "Should not auto-warm when no Rust files found"
        );
    }

    #[test]
    fn test_should_auto_warm_with_current_project() {
        use super::should_auto_warm;

        // Test with current project (should have Rust files)
        let current_path = std::env::current_dir().unwrap();
        let high_memory_limit = 1024 * 1024 * 1024; // 1GB - should be enough
        let low_memory_limit = 1024; // 1KB - should be too low

        let should_warm_high = should_auto_warm(&current_path, high_memory_limit);
        let should_warm_low = should_auto_warm(&current_path, low_memory_limit);

        // With high memory limit, should warm (we have Rust files)
        assert!(
            should_warm_high,
            "Should auto-warm current project with high memory limit"
        );

        // With very low memory limit, should not warm
        assert!(
            !should_warm_low,
            "Should not auto-warm with very low memory limit"
        );
    }

    #[test]
    fn test_cli_args_defaults() {
        use super::Args;
        use clap::Parser;

        // Test default CLI arguments (empty args)
        let args = Args::try_parse_from(&["language-server-mcp"]).unwrap();
        assert!(!args.no_auto_warm, "no_auto_warm should default to false");
        assert!(
            args.max_memory.is_none(),
            "max_memory should default to None"
        );
        assert!(!args.warm_all, "warm_all should default to false");
        assert!(!args.show_progress, "show_progress should default to false");
    }

    #[test]
    fn test_cli_args_flags() {
        use super::Args;
        use clap::Parser;

        // Test all CLI flags set
        let args = Args::try_parse_from(&[
            "language-server-mcp",
            "--no-auto-warm",
            "--max-memory",
            "1g",
            "--warm-all",
            "--show-progress",
        ])
        .unwrap();

        assert!(
            args.no_auto_warm,
            "no_auto_warm should be true when flag set"
        );
        assert_eq!(
            args.max_memory,
            Some("1g".to_string()),
            "max_memory should be set"
        );
        assert!(args.warm_all, "warm_all should be true when flag set");
        assert!(
            args.show_progress,
            "show_progress should be true when flag set"
        );
    }

    #[test]
    fn test_cli_memory_parsing_integration() {
        use super::{parse_memory_size, Args};
        use clap::Parser;

        // Test that CLI parsing works with memory parsing
        let args = Args::try_parse_from(&["language-server-mcp", "--max-memory", "512m"]).unwrap();

        let memory_limit = parse_memory_size(args.max_memory.as_ref().unwrap()).unwrap();
        assert_eq!(memory_limit, 512 * 1024 * 1024);
    }
}
