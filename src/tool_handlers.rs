#![deny(dead_code)]

// Extracted tool handler implementations that can be tested directly
// This allows us to test the business logic without the MCP protocol overhead

use crate::lsp_client::LspClient;
use crate::models::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Handle hover requests - extracted for testability
pub async fn handle_hover(
    lsp_client: &Arc<Mutex<LspClient>>,
    request: HoverRequest,
) -> Result<String, String> {
    let result = {
        let lsp_client = lsp_client.lock().await;
        lsp_client
            .hover(&request.file_path, request.line, request.column)
            .await
    };

    match result {
        Ok(Some(hover)) => match hover.contents {
            lsp_types::HoverContents::Markup(markup) => Ok(markup.value),
            lsp_types::HoverContents::Array(markups) => Ok(markups
                .into_iter()
                .map(|m| match m {
                    lsp_types::MarkedString::String(s) => s,
                    lsp_types::MarkedString::LanguageString(ls) => ls.value,
                })
                .collect::<Vec<_>>()
                .join("\n\n")),
            lsp_types::HoverContents::Scalar(ms) => Ok(match ms {
                lsp_types::MarkedString::String(s) => s,
                lsp_types::MarkedString::LanguageString(ls) => ls.value,
            }),
        },
        Ok(None) => Ok("No hover information available".to_string()),
        Err(e) => Err(format!("LSP error: {e}")),
    }
}

/// Handle completion requests - extracted for testability
pub async fn handle_completion(
    lsp_client: &Arc<Mutex<LspClient>>,
    request: CompletionRequest,
) -> Result<String, String> {
    let result = {
        let lsp_client = lsp_client.lock().await;
        lsp_client
            .completion(&request.file_path, request.line, request.column)
            .await
    };

    match result {
        Ok(Some(response)) => {
            use lsp_types::CompletionResponse;
            let items = match response {
                CompletionResponse::Array(items) => items,
                CompletionResponse::List(list) => list.items,
            };

            if items.is_empty() {
                Ok("No completions available".to_string())
            } else {
                let completions: Vec<String> = items
                    .into_iter()
                    .take(crate::lsp_client::MAX_COMPLETION_ITEMS)
                    .map(|item| {
                        let detail = item.detail.unwrap_or_default();
                        let kind = item
                            .kind
                            .map(|k| format!("{:?}", k))
                            .unwrap_or_else(|| "Unknown".to_string());
                        format!("- {} [{}]: {}", item.label, kind, detail)
                    })
                    .collect();
                Ok(format!("Completions:\n{}", completions.join("\n")))
            }
        }
        Ok(None) => Ok("No completions available".to_string()),
        Err(e) => Err(format!("LSP error: {e}")),
    }
}

/// Handle diagnostics requests - extracted for testability
pub async fn handle_diagnostics(
    lsp_client: &Arc<Mutex<LspClient>>,
    request: DiagnosticsRequest,
) -> Result<String, String> {
    let result = {
        let lsp_client = lsp_client.lock().await;
        lsp_client.diagnostics(&request.file_path).await
    };

    match result {
        Ok(diagnostics) => {
            if diagnostics.is_empty() {
                Ok("No diagnostics found".to_string())
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

                Ok(format!("Diagnostics:\n{diagnostic_text}"))
            }
        }
        Err(e) => Err(format!("LSP error: {e}")),
    }
}

/// Handle goto_definition requests - extracted for testability
pub async fn handle_goto_definition(
    lsp_client: &Arc<Mutex<LspClient>>,
    request: GotoDefinitionRequest,
) -> Result<String, String> {
    let lsp_client = lsp_client.lock().await;

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
                Ok("No definition found".to_string())
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

                Ok(format!("Found definitions:\n{definition_text}"))
            }
        }
        Ok(None) => Ok("No definition found".to_string()),
        Err(e) => Err(format!("LSP error: {e}")),
    }
}

// Add more handler functions as needed...
