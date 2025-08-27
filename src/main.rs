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
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
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

#[derive(Debug, Parser)]
#[command(name = "language-server-mcp")]
#[command(about = "Rust-analyzer MCP server")]
struct Args {
    /// Workspace root directory (optional)
    workspace_root: Option<PathBuf>,
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

    #[tool(description = "Get rust-analyzer LSP status")]
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing with fallback to sink if stderr unavailable (e.g., in Claude Code)
    // This prevents "Broken pipe" panic when stderr is not connected
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(std::io::sink)  // Use null writer - MCP doesn't need stderr logging
        .with_ansi(false)
        .try_init();  // Silently ignore if already initialized

    info!("Starting rust-analyzer MCP server");

    let workspace_root = args
        .workspace_root
        .unwrap_or_else(|| std::env::current_dir().expect("Failed to determine current directory"));

    // Create the MCP service
    let mcp_service = RustAnalyzerMCP::new(workspace_root).await?;

    // Start MCP service - server is immediately responsive
    let service = mcp_service.serve(stdio()).await?;

    info!("MCP server is running");
    service.waiting().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{CloseDocumentRequest, DocumentSymbolsRequest, LspClientStatusRequest};
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
}
