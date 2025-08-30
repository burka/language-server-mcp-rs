#![deny(dead_code)]

/// Domain layer for rust-analyzer operations
///
/// This module provides a clean abstraction over rust-analyzer functionality,
/// independent of MCP or LSP protocol details. It represents the business logic
/// layer that can be reused by different protocol implementations.
use crate::errors::LspError;
use crate::lsp_client::LspClient;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Position in a source file
#[derive(Debug, Clone)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// Range in a source file
#[derive(Debug, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

/// Location of a symbol in source code
#[derive(Debug, Clone)]
pub struct Location {
    pub file_path: String,
    pub range: Range,
}

/// Type and documentation information
#[derive(Debug, Clone)]
pub struct HoverInfo {
    pub content: String,
    pub range: Option<Range>,
}

/// Code completion suggestion
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: String,
    pub documentation: Option<String>,
}

/// Diagnostic information (errors, warnings)
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub range: Range,
    pub source: Option<String>,
    pub code: Option<String>,
}

#[derive(Debug, Clone)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// Symbol information
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: Location,
    pub container_name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function,
    Variable,
    Class,
    Interface,
    Module,
    Property,
    Field,
    Constructor,
    Enum,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
}

/// Main domain service for rust-analyzer operations
///
/// This service provides high-level operations for code analysis, independent
/// of the underlying LSP protocol implementation. It handles:
/// - Initialization and lifecycle management
/// - Error handling and retries
/// - Performance optimization
/// - Resource management
pub struct RustAnalyzer {
    lsp_client: Arc<Mutex<LspClient>>,
    workspace_root: PathBuf,
    #[allow(dead_code)] // Will be used for monitoring in future
    initialization_time: Instant,
}

impl RustAnalyzer {
    /// Create a new RustAnalyzer service for the given workspace
    pub async fn new(workspace_root: PathBuf) -> Result<Self, LspError> {
        info!(
            "Initializing RustAnalyzer domain service for workspace: {:?}",
            workspace_root
        );

        let lsp_client = LspClient::new(&workspace_root).await?;

        let service = Self {
            lsp_client: Arc::new(Mutex::new(lsp_client)),
            workspace_root,
            initialization_time: Instant::now(),
        };

        // Pre-warm the analyzer for better performance
        service.pre_warm().await;

        info!("RustAnalyzer domain service initialized and ready");
        Ok(service)
    }

    /// Get workspace root path
    pub fn workspace_root(&self) -> &PathBuf {
        &self.workspace_root
    }

    /// Get hover information at the specified position
    pub async fn hover(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<Option<HoverInfo>, LspError> {
        self.execute_with_retry("hover", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .hover(file_path, position.line, position.column)
                .await?;

            Ok(result.map(|hover| {
                let content = match hover.contents {
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
                };

                let range = hover.range.map(|r| Range {
                    start: Position {
                        line: r.start.line,
                        column: r.start.character,
                    },
                    end: Position {
                        line: r.end.line,
                        column: r.end.character,
                    },
                });

                HoverInfo { content, range }
            }))
        })
        .await
    }

    /// Get code completions at the specified position
    pub async fn completion(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<Vec<CompletionItem>, LspError> {
        self.execute_with_retry("completion", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .completion(file_path, position.line, position.column)
                .await?;

            match result {
                Some(response) => {
                    use lsp_types::CompletionResponse;
                    let items = match response {
                        CompletionResponse::Array(items) => items,
                        CompletionResponse::List(list) => list.items,
                    };

                    let completions = items
                        .into_iter()
                        .take(crate::lsp_client::MAX_COMPLETION_ITEMS)
                        .map(|item| CompletionItem {
                            label: item.label,
                            detail: item.detail,
                            kind: item
                                .kind
                                .map(|k| format!("{:?}", k))
                                .unwrap_or_else(|| "Unknown".to_string()),
                            documentation: item.documentation.map(|doc| match doc {
                                lsp_types::Documentation::String(s) => s,
                                lsp_types::Documentation::MarkupContent(markup) => markup.value,
                            }),
                        })
                        .collect();

                    Ok(completions)
                }
                None => Ok(vec![]),
            }
        })
        .await
    }

    /// Get diagnostics for the specified file
    pub async fn diagnostics(&self, file_path: &str) -> Result<Vec<Diagnostic>, LspError> {
        self.execute_with_retry("diagnostics", || async {
            let client = self.lsp_client.lock().await;
            let result = client.diagnostics(file_path).await?;

            let diagnostics = result
                .into_iter()
                .map(|diag| Diagnostic {
                    message: diag.message,
                    severity: match diag.severity {
                        Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
                        Some(lsp_types::DiagnosticSeverity::WARNING) => DiagnosticSeverity::Warning,
                        Some(lsp_types::DiagnosticSeverity::INFORMATION) => {
                            DiagnosticSeverity::Information
                        }
                        Some(lsp_types::DiagnosticSeverity::HINT) => DiagnosticSeverity::Hint,
                        Some(_) => DiagnosticSeverity::Error, // Handle unknown severity levels
                        None => DiagnosticSeverity::Error,
                    },
                    range: Range {
                        start: Position {
                            line: diag.range.start.line,
                            column: diag.range.start.character,
                        },
                        end: Position {
                            line: diag.range.end.line,
                            column: diag.range.end.character,
                        },
                    },
                    source: diag.source,
                    code: diag.code.map(|c| match c {
                        lsp_types::NumberOrString::Number(n) => n.to_string(),
                        lsp_types::NumberOrString::String(s) => s,
                    }),
                })
                .collect();

            Ok(diagnostics)
        })
        .await
    }

    /// Find definition of symbol at the specified position
    pub async fn goto_definition(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<Vec<Location>, LspError> {
        self.execute_with_retry("goto_definition", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .goto_definition(file_path, position.line, position.column)
                .await
                .map_err(|e| LspError::Other {
                    message: e.to_string(),
                })?;

            match result {
                Some(response) => {
                    use lsp_types::GotoDefinitionResponse;
                    let locations = match response {
                        GotoDefinitionResponse::Scalar(loc) => vec![loc],
                        GotoDefinitionResponse::Array(locs) => locs,
                        GotoDefinitionResponse::Link(links) => {
                            return Ok(links
                                .into_iter()
                                .map(|link| Location {
                                    file_path: link
                                        .target_uri
                                        .to_file_path()
                                        .unwrap_or_default()
                                        .to_string_lossy()
                                        .to_string(),
                                    range: Range {
                                        start: Position {
                                            line: link.target_range.start.line,
                                            column: link.target_range.start.character,
                                        },
                                        end: Position {
                                            line: link.target_range.end.line,
                                            column: link.target_range.end.character,
                                        },
                                    },
                                })
                                .collect());
                        }
                    };

                    let locations = locations
                        .into_iter()
                        .map(|loc| Location {
                            file_path: loc
                                .uri
                                .to_file_path()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            range: Range {
                                start: Position {
                                    line: loc.range.start.line,
                                    column: loc.range.start.character,
                                },
                                end: Position {
                                    line: loc.range.end.line,
                                    column: loc.range.end.character,
                                },
                            },
                        })
                        .collect();

                    Ok(locations)
                }
                None => Ok(vec![]),
            }
        })
        .await
    }

    /// Find all references to symbol at the specified position
    pub async fn find_references(
        &self,
        file_path: &str,
        position: Position,
        include_declaration: bool,
    ) -> Result<Vec<Location>, LspError> {
        self.execute_with_retry("find_references", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .find_references(
                    file_path,
                    position.line,
                    position.column,
                    include_declaration,
                )
                .await
                .map_err(|e| LspError::Other {
                    message: e.to_string(),
                })?;

            match result {
                Some(locations) => {
                    let locations = locations
                        .into_iter()
                        .map(|loc| Location {
                            file_path: loc
                                .uri
                                .to_file_path()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            range: Range {
                                start: Position {
                                    line: loc.range.start.line,
                                    column: loc.range.start.character,
                                },
                                end: Position {
                                    line: loc.range.end.line,
                                    column: loc.range.end.character,
                                },
                            },
                        })
                        .collect();

                    Ok(locations)
                }
                None => Ok(vec![]),
            }
        })
        .await
    }

    /// Format the specified document
    pub async fn format_document(&self, file_path: &str) -> Result<Option<String>, LspError> {
        self.execute_with_retry("format_document", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .format_document(file_path)
                .await
                .map_err(|e| LspError::Other {
                    message: e.to_string(),
                })?;

            match result {
                Some(edits) => {
                    if edits.is_empty() {
                        Ok(Some("No formatting changes needed".to_string()))
                    } else {
                        Ok(Some(format!(
                            "Formatting would apply {} edits to the file",
                            edits.len()
                        )))
                    }
                }
                None => Ok(Some("Document does not support formatting".to_string())),
            }
        })
        .await
    }

    /// Get workspace symbols matching the query
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<Symbol>, LspError> {
        self.execute_with_retry("workspace_symbols", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .workspace_symbols(query)
                .await
                .map_err(|e| LspError::Other {
                    message: e.to_string(),
                })?;

            let symbols = result
                .unwrap_or_default()
                .into_iter()
                .map(|symbol| {
                    let kind = match symbol.kind {
                        lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
                        lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
                        lsp_types::SymbolKind::CLASS => SymbolKind::Class,
                        lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
                        lsp_types::SymbolKind::MODULE => SymbolKind::Module,
                        lsp_types::SymbolKind::PROPERTY => SymbolKind::Property,
                        lsp_types::SymbolKind::FIELD => SymbolKind::Field,
                        lsp_types::SymbolKind::CONSTRUCTOR => SymbolKind::Constructor,
                        lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
                        lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
                        lsp_types::SymbolKind::EVENT => SymbolKind::Event,
                        lsp_types::SymbolKind::OPERATOR => SymbolKind::Operator,
                        lsp_types::SymbolKind::TYPE_PARAMETER => SymbolKind::TypeParameter,
                        _ => SymbolKind::Object, // Default for unknown kinds
                    };

                    Symbol {
                        name: symbol.name,
                        kind,
                        location: Location {
                            file_path: symbol
                                .location
                                .uri
                                .to_file_path()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                            range: Range {
                                start: Position {
                                    line: symbol.location.range.start.line,
                                    column: symbol.location.range.start.character,
                                },
                                end: Position {
                                    line: symbol.location.range.end.line,
                                    column: symbol.location.range.end.character,
                                },
                            },
                        },
                        container_name: symbol.container_name,
                    }
                })
                .collect();

            Ok(symbols)
        })
        .await
    }

    /// Get document symbols for the specified file  
    pub async fn document_symbols(&self, file_path: &str) -> Result<Vec<Symbol>, LspError> {
        self.execute_with_retry("document_symbols", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .document_symbols(file_path)
                .await
                .map_err(|e| LspError::Other {
                    message: e.to_string(),
                })?;

            let symbols = match result {
                Some(response) => match response {
                    lsp_types::DocumentSymbolResponse::Flat(symbol_info_vec) => {
                        symbol_info_vec
                            .into_iter()
                            .map(|symbol| {
                                let kind = match symbol.kind {
                                    lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
                                    lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
                                    lsp_types::SymbolKind::CLASS => SymbolKind::Class,
                                    lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
                                    lsp_types::SymbolKind::MODULE => SymbolKind::Module,
                                    lsp_types::SymbolKind::PROPERTY => SymbolKind::Property,
                                    lsp_types::SymbolKind::FIELD => SymbolKind::Field,
                                    lsp_types::SymbolKind::CONSTRUCTOR => SymbolKind::Constructor,
                                    lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
                                    lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
                                    lsp_types::SymbolKind::EVENT => SymbolKind::Event,
                                    lsp_types::SymbolKind::OPERATOR => SymbolKind::Operator,
                                    lsp_types::SymbolKind::TYPE_PARAMETER => {
                                        SymbolKind::TypeParameter
                                    }
                                    _ => SymbolKind::Object, // Default for unknown kinds
                                };

                                Symbol {
                                    name: symbol.name,
                                    kind,
                                    location: Location {
                                        file_path: symbol
                                            .location
                                            .uri
                                            .to_file_path()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string(),
                                        range: Range {
                                            start: Position {
                                                line: symbol.location.range.start.line,
                                                column: symbol.location.range.start.character,
                                            },
                                            end: Position {
                                                line: symbol.location.range.end.line,
                                                column: symbol.location.range.end.character,
                                            },
                                        },
                                    },
                                    container_name: symbol.container_name,
                                }
                            })
                            .collect()
                    }
                    lsp_types::DocumentSymbolResponse::Nested(doc_symbols) => {
                        // For nested symbols, we flatten them for simplicity
                        doc_symbols
                            .into_iter()
                            .map(|symbol| {
                                let kind = match symbol.kind {
                                    lsp_types::SymbolKind::FUNCTION => SymbolKind::Function,
                                    lsp_types::SymbolKind::VARIABLE => SymbolKind::Variable,
                                    lsp_types::SymbolKind::CLASS => SymbolKind::Class,
                                    lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
                                    lsp_types::SymbolKind::MODULE => SymbolKind::Module,
                                    lsp_types::SymbolKind::PROPERTY => SymbolKind::Property,
                                    lsp_types::SymbolKind::FIELD => SymbolKind::Field,
                                    lsp_types::SymbolKind::CONSTRUCTOR => SymbolKind::Constructor,
                                    lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
                                    lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
                                    lsp_types::SymbolKind::EVENT => SymbolKind::Event,
                                    lsp_types::SymbolKind::OPERATOR => SymbolKind::Operator,
                                    lsp_types::SymbolKind::TYPE_PARAMETER => {
                                        SymbolKind::TypeParameter
                                    }
                                    _ => SymbolKind::Object, // Default for unknown kinds
                                };

                                Symbol {
                                    name: symbol.name,
                                    kind,
                                    location: Location {
                                        file_path: file_path.to_string(),
                                        range: Range {
                                            start: Position {
                                                line: symbol.range.start.line,
                                                column: symbol.range.start.character,
                                            },
                                            end: Position {
                                                line: symbol.range.end.line,
                                                column: symbol.range.end.character,
                                            },
                                        },
                                    },
                                    container_name: None, // Nested symbols don't have container names in this context
                                }
                            })
                            .collect()
                    }
                },
                None => Vec::new(), // Return empty vector if no symbols found
            };

            Ok(symbols)
        })
        .await
    }

    /// Simple wrapper methods for remaining functionality
    /// These provide basic domain service interfaces for less commonly used features
    pub async fn lsp_status(&self) -> Result<String, LspError> {
        let _client = self.lsp_client.lock().await;
        Ok("rust-analyzer is running".to_string())
    }

    pub async fn close_document(&self, file_path: &str) -> Result<(usize, usize), LspError> {
        let client = self.lsp_client.lock().await;
        let opened_count_before = client.get_opened_documents_count().await;

        client
            .close_document(file_path)
            .await
            .map_err(|e| LspError::Other {
                message: format!("Failed to close document: {}", e),
            })?;

        let opened_count_after = client.get_opened_documents_count().await;
        Ok((opened_count_before, opened_count_after))
    }

    /// Placeholder methods for advanced features
    /// These return simple string responses until full implementation is needed
    pub async fn rename(
        &self,
        file_path: &str,
        position: Position,
        new_name: &str,
    ) -> Result<String, LspError> {
        self.execute_with_retry("rename", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .rename(file_path, position.line, position.column, new_name)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(workspace_edit) => {
                    if workspace_edit.changes.is_none() && workspace_edit.document_changes.is_none()
                    {
                        Ok("No rename available at this position".to_string())
                    } else {
                        let change_count = workspace_edit
                            .changes
                            .as_ref()
                            .map(|changes| changes.len())
                            .unwrap_or(0);
                        Ok(format!(
                            "Rename available with {} file changes",
                            change_count
                        ))
                    }
                }
                None => Ok("No rename available at this position".to_string()),
            }
        })
        .await
    }

    pub async fn code_actions(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<String, LspError> {
        self.execute_with_retry("code_actions", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .code_actions(file_path, position.line, position.column)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(actions) => {
                    if actions.is_empty() {
                        Ok("No code actions available at this position".to_string())
                    } else {
                        let action_types: Vec<String> = actions
                            .iter()
                            .map(|action| match action {
                                lsp_types::CodeActionOrCommand::CodeAction(ca) => ca.title.clone(),
                                lsp_types::CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
                            })
                            .collect();
                        Ok(format!(
                            "Found {} code actions: {}",
                            actions.len(),
                            action_types.join(", ")
                        ))
                    }
                }
                None => Ok("No code actions available at this position".to_string()),
            }
        })
        .await
    }

    pub async fn inlay_hints(&self, file_path: &str) -> Result<String, LspError> {
        self.execute_with_retry("inlay_hints", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .inlay_hints(file_path)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(hints) if !hints.is_empty() => {
                    let hints_text = hints
                        .iter()
                        .take(50) // Limit to first 50 for readability
                        .enumerate()
                        .map(|(i, hint)| {
                            let position = hint.position;
                            let label = match &hint.label {
                                lsp_types::InlayHintLabel::String(s) => s.clone(),
                                lsp_types::InlayHintLabel::LabelParts(parts) => parts
                                    .iter()
                                    .map(|p| p.value.clone())
                                    .collect::<Vec<_>>()
                                    .join(""),
                            };
                            let kind = hint.kind.map(|k| format!(" ({k:?})")).unwrap_or_default();

                            format!(
                                "{}. Line {}:{}: {}{}",
                                i + 1,
                                position.line + 1,
                                position.character + 1,
                                label,
                                kind
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(format!("Inlay hints:\n{hints_text}"))
                }
                _ => Ok("No inlay hints available".to_string()),
            }
        })
        .await
    }

    pub async fn expand_macro(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<String, LspError> {
        self.execute_with_retry("expand_macro", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .expand_macro(file_path, position.line, position.column)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(expansion) => {
                    // Try to extract meaningful content from the JSON response
                    let expansion_text = if let Some(expansion_str) = expansion.as_str() {
                        expansion_str.to_string()
                    } else if let Some(obj) = expansion.as_object() {
                        // Look for common rust-analyzer response fields
                        if let Some(expanded) = obj.get("expansion").and_then(|v| v.as_str()) {
                            expanded.to_string()
                        } else if let Some(expanded) = obj.get("text").and_then(|v| v.as_str()) {
                            expanded.to_string()
                        } else {
                            serde_json::to_string_pretty(&expansion)
                                .unwrap_or_else(|_| expansion.to_string())
                        }
                    } else {
                        expansion.to_string()
                    };

                    if expansion_text.trim().is_empty() {
                        Ok("No macro expansion available at this position".to_string())
                    } else {
                        Ok(format!("Macro expansion:\n```rust\n{expansion_text}\n```"))
                    }
                }
                None => Ok("No macro found at this position".to_string()),
            }
        })
        .await
    }

    pub async fn signature_help(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<String, LspError> {
        self.execute_with_retry("signature_help", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .signature_help(file_path, position.line, position.column)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(signature_help) => {
                    if signature_help.signatures.is_empty() {
                        Ok("No signature help available at this position".to_string())
                    } else {
                        let active_signature =
                            signature_help.active_signature.unwrap_or(0) as usize;
                        if let Some(signature) = signature_help.signatures.get(active_signature) {
                            let params = signature
                                .parameters
                                .as_ref()
                                .map(|params| {
                                    params
                                        .iter()
                                        .map(|p| match &p.label {
                                            lsp_types::ParameterLabel::Simple(s) => s.clone(),
                                            lsp_types::ParameterLabel::LabelOffsets(_) => {
                                                "param".to_string()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            Ok(format!("Signature: {}({})", signature.label, params))
                        } else {
                            Ok("No signature help available at this position".to_string())
                        }
                    }
                }
                None => Ok("No signature help available at this position".to_string()),
            }
        })
        .await
    }

    pub async fn document_highlight(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<String, LspError> {
        self.execute_with_retry("document_highlight", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .document_highlight(file_path, position.line, position.column)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(highlights) if !highlights.is_empty() => {
                    let highlights_text = highlights
                        .iter()
                        .enumerate()
                        .map(|(i, highlight)| {
                            let kind = highlight
                                .kind
                                .map(|k| format!(" ({k:?})"))
                                .unwrap_or_default();
                            format!(
                                "{}. Line {}:{}-{}:{}{}",
                                i + 1,
                                highlight.range.start.line + 1,
                                highlight.range.start.character + 1,
                                highlight.range.end.line + 1,
                                highlight.range.end.character + 1,
                                kind
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(format!("Document highlights:\n{highlights_text}"))
                }
                _ => Ok("No highlights found at this position".to_string()),
            }
        })
        .await
    }

    pub async fn selection_range(
        &self,
        _file_path: &str,
        _positions: Vec<Position>,
    ) -> Result<String, LspError> {
        Ok("Selection range not fully implemented in domain service yet".to_string())
    }

    pub async fn runnables(&self, _file_path: &str) -> Result<String, LspError> {
        Ok("Runnables not fully implemented in domain service yet".to_string())
    }

    pub async fn implementations(
        &self,
        file_path: &str,
        position: Position,
    ) -> Result<String, LspError> {
        self.execute_with_retry("implementations", || async {
            let client = self.lsp_client.lock().await;
            let result = client
                .implementations(file_path, position.line, position.column)
                .await
                .map_err(|e| e.to_string())?;

            match result {
                Some(locations) if !locations.is_empty() => {
                    let implementations_text = locations
                        .iter()
                        .enumerate()
                        .map(|(i, location)| {
                            let path = location
                                .uri
                                .to_file_path()
                                .ok()
                                .and_then(|p| p.to_str().map(|s| s.to_string()))
                                .unwrap_or_else(|| location.uri.to_string());
                            format!(
                                "{}. Implementation at: {}:{}:{}",
                                i + 1,
                                path,
                                location.range.start.line + 1,
                                location.range.start.character + 1
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    Ok(format!("Found implementations:\n{implementations_text}"))
                }
                _ => Ok("No implementations found".to_string()),
            }
        })
        .await
    }

    /// Pre-warm rust-analyzer with a diagnostics request for optimal performance
    async fn pre_warm(&self) {
        info!("Pre-warming rust-analyzer domain service...");
        let start = Instant::now();

        // Try to find a Rust file to use for pre-warming
        let prewarm_file = if self.workspace_root.join("src/main.rs").exists() {
            "src/main.rs"
        } else if self.workspace_root.join("src/lib.rs").exists() {
            "src/lib.rs"
        } else {
            "src/main.rs" // fallback
        };

        match self.diagnostics(prewarm_file).await {
            Ok(_) => {
                let elapsed = start.elapsed();
                info!("Pre-warming completed successfully in {:?}", elapsed);
            }
            Err(e) => {
                let elapsed = start.elapsed();
                debug!(
                    "Pre-warming had expected initialization error in {:?}: {}",
                    elapsed, e
                );
                // This is expected and OK - the pre-warming still helps performance
            }
        }
    }

    /// Execute an operation with automatic retry for readiness errors
    async fn execute_with_retry<F, Fut, T>(
        &self,
        operation_name: &str,
        handler: F,
    ) -> Result<T, LspError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, LspError>>,
    {
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY: Duration = Duration::from_millis(300);
        const SEMANTIC_RETRY_DELAY: Duration = Duration::from_millis(1500);

        for attempt in 1..=MAX_RETRIES {
            match handler().await {
                Ok(result) => return Ok(result),
                Err(e) if e.is_retryable() && attempt < MAX_RETRIES => {
                    let delay = if e.is_semantic_issue() {
                        warn!("Operation '{}' may need semantic indexing (attempt {}). Waiting {:?} for rust-analyzer readiness...", 
                              operation_name, attempt, SEMANTIC_RETRY_DELAY);
                        SEMANTIC_RETRY_DELAY
                    } else {
                        debug!("Operation '{}' failed with retryable error (attempt {}): {}. Retrying in {:?}...", 
                               operation_name, attempt, e, RETRY_DELAY);
                        RETRY_DELAY
                    };

                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }

        // This should never be reached due to the loop logic, but just in case
        Err(LspError::Other {
            message: format!(
                "Operation '{}' failed after {} retries",
                operation_name, MAX_RETRIES
            ),
        })
    }

    // Infrastructure methods for direct LSP client access

    /// Get LSP client status information
    pub async fn get_lsp_status(&self) -> (bool, usize, (usize, usize)) {
        let client = self.lsp_client.lock().await;
        let is_ready = client.is_ready();
        let opened_count = client.get_opened_documents_count().await;
        let (memory_mb_opt, doc_count) = client.get_memory_status().await;
        let memory_mb = memory_mb_opt.unwrap_or(0) as usize;
        (is_ready, opened_count, (memory_mb, doc_count))
    }
}
