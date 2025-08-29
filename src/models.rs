#![deny(dead_code)]

use crate::lsp_client::MAX_SYMBOLS_COUNT;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct HoverRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CompletionRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DiagnosticsRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GotoDefinitionRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct FormatRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RenameRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub new_name: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CodeActionsRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct WorkspaceSymbolsRequest {
    pub query: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct InlayHintsRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ExpandMacroRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SignatureHelpRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DocumentHighlightRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct SelectionRangeRequest {
    pub file_path: String,
    pub positions: Vec<PositionInfo>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct RunnablesRequest {
    pub file_path: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ImplementationsRequest {
    pub file_path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct LspClientStatusRequest {
    // No fields needed
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct CloseDocumentRequest {
    pub file_path: String,
}

// Additional types
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct PositionInfo {
    pub line: u32,
    pub column: u32,
}
