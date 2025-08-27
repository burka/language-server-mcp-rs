use lsp_types::{request::GotoImplementationParams, *};
use serde_json::{json, Value};

use std::collections::HashSet;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{Pid, System};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;
use tracing::{debug, info, warn};

const DEFAULT_TIMEOUT_SECS: u64 = 60;
const MAX_RESPONSE_SIZE_BYTES: usize = 40 * 1024; // 40KB ≈ 10k tokens (LLM-friendly)
pub const MAX_LARGE_RESPONSE_SIZE_BYTES: usize = 120 * 1024; // 120KB ≈ 30k tokens (for diagnostics)
pub const MAX_SYMBOLS_COUNT: usize = 200; // Reasonable symbol limit with paging
pub const MAX_COMPLETION_ITEMS: usize = 25; // Reasonable completion limit

pub struct LspClient {
    process: Child,
    stdin: Mutex<tokio::process::ChildStdin>,
    stdout: Mutex<BufReader<tokio::process::ChildStdout>>,
    request_id: Mutex<i64>,
    workspace_root: PathBuf,
    is_ready: Arc<AtomicBool>,
    opened_documents: Mutex<HashSet<String>>,
    timeout_secs: u64,
    // Memory monitoring
    process_pid: Option<u32>,
    system: Mutex<System>,
}

pub fn get_timeout_secs() -> u64 {
    env::var("RUST_ANALYZER_MCP_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
}

pub fn get_max_response_size() -> usize {
    env::var("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(MAX_RESPONSE_SIZE_BYTES)
}

fn get_max_response_size_for_method(method: &str) -> usize {
    match method {
        // Diagnostics can be large with many issues
        "textDocument/diagnostic" => env::var("RUST_ANALYZER_MCP_MAX_DIAGNOSTIC_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(MAX_LARGE_RESPONSE_SIZE_BYTES),
        // Completions should be limited to avoid overwhelming UX
        "textDocument/completion" => get_max_response_size() / 2, // Smaller limit for completions
        // Document symbols can be large but should be reasonable
        "textDocument/documentSymbol" => get_max_response_size(),
        // Default for other methods
        _ => get_max_response_size(),
    }
}

impl LspClient {
    pub async fn new(workspace_root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        info!("Starting rust-analyzer process");

        let mut process = Command::new("rust-analyzer")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = process.stdin.take().unwrap();
        let stdout = BufReader::new(process.stdout.take().unwrap());

        // Get process PID for memory monitoring
        let process_pid = process.id();

        let mut client = Self {
            process,
            stdin: Mutex::new(stdin),
            stdout: Mutex::new(stdout),
            request_id: Mutex::new(0),
            workspace_root: workspace_root.to_path_buf(),
            is_ready: Arc::new(AtomicBool::new(false)),
            opened_documents: Mutex::new(HashSet::new()),
            timeout_secs: get_timeout_secs(),
            process_pid,
            system: Mutex::new(System::new()),
        };

        // Initialize synchronously for now - we'll add async initialization later
        client.initialize().await?;
        client.is_ready.store(true, Ordering::Relaxed);

        Ok(client)
    }

    pub async fn wait_for_ready(&self) {
        while !self.is_ready.load(Ordering::Relaxed) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }


    pub fn is_ready(&self) -> bool {
        self.is_ready.load(Ordering::Relaxed)
    }

    async fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let workspace_folder = WorkspaceFolder {
            uri: Url::from_file_path(&self.workspace_root).unwrap(),
            name: self
                .workspace_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
        };

        let initialize_params = InitializeParams {
            capabilities: ClientCapabilities::default(),
            workspace_folders: Some(vec![workspace_folder]),
            initialization_options: Some(json!({
                "cargo": {
                    "runBuildScripts": true,
                    "features": "all"
                }
            })),
            ..Default::default()
        };

        let response: InitializeResult = self.request("initialize", initialize_params).await?;
        info!(
            "LSP initialized with capabilities: {:?}",
            response.capabilities
        );

        self.notify("initialized", InitializedParams {}).await?;

        // rust-analyzer will now automatically analyze the entire workspace
        // based on Cargo.toml. No need to manually open individual files!
        info!(
            "rust-analyzer is automatically analyzing workspace at: {:?}",
            self.workspace_root
        );

        Ok(())
    }

    pub async fn open_document(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Check if document is already opened
        {
            let opened_docs = self.opened_documents.lock().await;
            if opened_docs.contains(file_path) {
                debug!("Document already opened, using cache: {}", file_path);
                return Ok(());
            }
        }

        // Document not opened yet, open it
        debug!("Opening new document: {}", file_path);
        let content = tokio::fs::read_to_string(file_path).await?;
        let params = DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: Url::from_file_path(file_path).unwrap(),
                language_id: "rust".to_string(),
                version: 1,
                text: content,
            },
        };

        self.notify("textDocument/didOpen", params).await?;

        // Mark document as opened
        {
            let mut opened_docs = self.opened_documents.lock().await;
            opened_docs.insert(file_path.to_string());
            debug!(
                "Document opened and cached. Total opened documents: {}",
                opened_docs.len()
            );
        }

        Ok(())
    }

    pub async fn close_document(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Check if document is opened
        {
            let opened_docs = self.opened_documents.lock().await;
            if !opened_docs.contains(file_path) {
                debug!("Document not opened, no need to close: {}", file_path);
                return Ok(()); // Already closed or never opened
            }
        }

        // Close the document
        debug!("Closing document: {}", file_path);
        let params = DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
        };

        self.notify("textDocument/didClose", params).await?;

        // Remove from opened documents tracking
        {
            let mut opened_docs = self.opened_documents.lock().await;
            opened_docs.remove(file_path);
            debug!(
                "Document closed. Total opened documents: {}",
                opened_docs.len()
            );
        }

        Ok(())
    }

    pub async fn get_opened_documents_count(&self) -> usize {
        let opened_docs = self.opened_documents.lock().await;
        opened_docs.len()
    }

    pub async fn hover(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Hover>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/hover", params).await
    }

    pub async fn completion(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<CompletionResponse>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        };

        self.request("textDocument/completion", params).await
    }

    pub async fn diagnostics(
        &self,
        file_path: &str,
    ) -> Result<Vec<Diagnostic>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let response: DocumentDiagnosticReportResult =
            self.request("textDocument/diagnostic", params).await?;

        match response {
            DocumentDiagnosticReportResult::Report(report) => match report {
                DocumentDiagnosticReport::Full(full) => {
                    Ok(full.full_document_diagnostic_report.items)
                }
                DocumentDiagnosticReport::Unchanged(_) => Ok(vec![]),
            },
            DocumentDiagnosticReportResult::Partial(_) => Ok(vec![]),
        }
    }

    pub async fn goto_definition(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<GotoDefinitionResponse>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/definition", params).await
    }

    pub async fn find_references(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
        include_declaration: bool,
    ) -> Result<Option<Vec<Location>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration,
            },
        };

        self.request("textDocument/references", params).await
    }

    pub async fn format_document(
        &self,
        file_path: &str,
    ) -> Result<Option<Vec<TextEdit>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = DocumentFormattingParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            options: FormattingOptions {
                tab_size: 4,
                insert_spaces: true,
                ..Default::default()
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/formatting", params).await
    }

    pub async fn rename(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
        new_name: &str,
    ) -> Result<Option<WorkspaceEdit>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            new_name: new_name.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/rename", params).await
    }

    pub async fn code_actions(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<CodeActionResponse>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            range: Range {
                start: Position {
                    line,
                    character: column,
                },
                end: Position {
                    line,
                    character: column,
                },
            },
            context: CodeActionContext {
                diagnostics: vec![], // We could pass current diagnostics here
                only: None,          // Request all types of code actions
                trigger_kind: Some(CodeActionTriggerKind::INVOKED),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/codeAction", params).await
    }

    pub async fn workspace_symbols(
        &self,
        query: &str,
    ) -> Result<Option<Vec<SymbolInformation>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        let params = WorkspaceSymbolParams {
            query: query.to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("workspace/symbol", params).await
    }

    pub async fn inlay_hints(
        &self,
        file_path: &str,
    ) -> Result<Option<Vec<InlayHint>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;

        // Read the file to get its content and determine the range
        let content = tokio::fs::read_to_string(file_path).await?;
        let lines: Vec<&str> = content.lines().collect();
        let end_line = lines.len().saturating_sub(1) as u32;
        let end_character = lines.last().map(|line| line.len()).unwrap_or(0) as u32;

        let params = InlayHintParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: end_line,
                    character: end_character,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        self.request("textDocument/inlayHint", params).await
    }

    pub async fn expand_macro(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;

        // rust-analyzer uses a custom expandMacro request
        let params = json!({
            "textDocument": {
                "uri": Url::from_file_path(file_path).unwrap()
            },
            "position": {
                "line": line,
                "character": column
            }
        });

        // This is a rust-analyzer specific extension, not standard LSP
        self.request("rust-analyzer/expandMacro", params).await
    }

    pub async fn document_symbols(
        &self,
        file_path: &str,
    ) -> Result<Option<DocumentSymbolResponse>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/documentSymbol", params).await
    }

    pub async fn signature_help(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<SignatureHelp>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = SignatureHelpParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            context: None,
        };

        self.request("textDocument/signatureHelp", params).await
    }

    pub async fn document_highlight(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Vec<DocumentHighlight>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = DocumentHighlightParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/documentHighlight", params).await
    }

    pub async fn selection_range(
        &self,
        file_path: &str,
        positions: Vec<Position>,
    ) -> Result<Option<Vec<SelectionRange>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = SelectionRangeParams {
            text_document: TextDocumentIdentifier {
                uri: Url::from_file_path(file_path).unwrap(),
            },
            positions,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/selectionRange", params).await
    }

    pub async fn runnables(
        &self,
        file_path: &str,
    ) -> Result<Option<Value>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;

        // rust-analyzer uses a custom runnables request
        let params = json!({
            "textDocument": {
                "uri": Url::from_file_path(file_path).unwrap()
            }
        });

        // This is a rust-analyzer specific extension, not standard LSP
        self.request("rust-analyzer/runnables", params).await
    }

    pub async fn implementations(
        &self,
        file_path: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<Vec<Location>>, Box<dyn std::error::Error>> {
        self.wait_for_ready().await;
        // Ensure document is open
        self.open_document(file_path).await?;
        let params = GotoImplementationParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: Url::from_file_path(file_path).unwrap(),
                },
                position: Position {
                    line,
                    character: column,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        self.request("textDocument/implementation", params).await
    }

    async fn request<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R, Box<dyn std::error::Error>> {
        self.request_with_timeout(method, params, Duration::from_secs(self.timeout_secs))
            .await
    }

    pub async fn request_with_timeout<P: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        params: P,
        timeout_duration: Duration,
    ) -> Result<R, Box<dyn std::error::Error>> {
        let mut id = self.request_id.lock().await;
        *id += 1;
        let request_id = *id;

        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });

        self.send_message(&request).await?;

        let response = match timeout(timeout_duration, self.read_response(request_id)).await {
            Ok(response) => response?,
            Err(_) => {
                warn!("LSP request '{method}' timed out after {timeout_duration:?}");
                return Err(
                    format!("LSP request '{method}' timed out after {timeout_duration:?}").into(),
                );
            }
        };

        if let Some(error) = response.get("error") {
            return Err(format!("LSP error: {error:?}").into());
        }

        let result = response.get("result").ok_or("Missing result in response")?;

        // Check response size before deserializing (method-specific limits)
        let result_json = serde_json::to_string(result)?;
        let max_size = get_max_response_size_for_method(method);
        if result_json.len() > max_size {
            warn!(
                "LSP response for '{}' is too large ({} bytes, max {} for this method)",
                method,
                result_json.len(),
                max_size
            );
            return Err(format!(
                "Response too large ({} bytes, max {} for {}). Try: smaller file, more specific query, or set RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE env var.",
                result_json.len(),
                max_size,
                method
            ).into());
        }

        Ok(serde_json::from_value(result.clone())?)
    }

    async fn notify<P: serde::Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(&notification).await
    }

    async fn send_message(&self, message: &Value) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string(message)?;
        let header = format!("Content-Length: {}\r\n\r\n", content.len());

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(header.as_bytes()).await?;
        stdin.write_all(content.as_bytes()).await?;
        stdin.flush().await?;

        debug!("Sent LSP message: {}", content);

        Ok(())
    }

    async fn read_response(&self, expected_id: i64) -> Result<Value, Box<dyn std::error::Error>> {
        let mut stdout = self.stdout.lock().await;

        loop {
            let mut header = String::new();
            stdout.read_line(&mut header).await?;

            if header.starts_with("Content-Length:") {
                let length: usize = header
                    .trim_start_matches("Content-Length:")
                    .trim()
                    .parse()?;

                stdout.read_line(&mut header).await?;

                let mut content = vec![0; length];
                stdout.read_exact(&mut content).await?;

                let response: Value = serde_json::from_slice(&content)?;
                debug!("Received LSP response: {}", response);

                if let Some(id) = response.get("id") {
                    if id.as_i64() == Some(expected_id) {
                        return Ok(response);
                    }
                }
            }
        }
    }

    /// Get current memory usage of the rust-analyzer process in MB
    pub async fn get_memory_usage_mb(&self) -> Option<u64> {
        if let Some(pid) = self.process_pid {
            let mut system = self.system.lock().await;
            system.refresh_processes();

            if let Some(process) = system.process(Pid::from_u32(pid)) {
                // Convert from bytes to MB
                return Some(process.memory() / 1024 / 1024);
            }
        }
        None
    }

    /// Get memory and document status for monitoring
    pub async fn get_memory_status(&self) -> (Option<u64>, usize) {
        let memory_mb = self.get_memory_usage_mb().await;
        let doc_count = self.get_opened_documents_count().await;
        (memory_mb, doc_count)
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        std::mem::drop(self.process.kill());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{
        get_max_response_size, get_max_response_size_for_method, get_timeout_secs,
        MAX_COMPLETION_ITEMS, MAX_LARGE_RESPONSE_SIZE_BYTES, MAX_RESPONSE_SIZE_BYTES,
        MAX_SYMBOLS_COUNT,
    };
    use serde_json::json;
    use std::time::Duration;
    use tokio::time::sleep;

    // Mock LSP client for testing timeout and size limit behavior
    struct MockLspClient {
        request_id: Mutex<i64>,
        should_timeout: bool,
        should_error: bool,
        response_size: Option<usize>, // If set, returns a response of this size
    }

    impl MockLspClient {
        fn new(should_timeout: bool, should_error: bool) -> Self {
            Self {
                request_id: Mutex::new(0),
                should_timeout,
                should_error,
                response_size: None,
            }
        }

        fn new_with_response_size(response_size: usize) -> Self {
            Self {
                request_id: Mutex::new(0),
                should_timeout: false,
                should_error: false,
                response_size: Some(response_size),
            }
        }

        async fn mock_request_with_timeout<P: serde::Serialize, R: serde::de::DeserializeOwned>(
            &self,
            method: &str,
            _params: P,
            timeout_duration: Duration,
        ) -> Result<R, Box<dyn std::error::Error>> {
            let mut id = self.request_id.lock().await;
            *id += 1;

            if self.should_timeout {
                // Simulate a request that takes longer than the timeout
                let long_delay = timeout_duration + Duration::from_millis(100);
                match timeout(timeout_duration, sleep(long_delay)).await {
                    Ok(_) => unreachable!("Should have timed out"),
                    Err(_) => {
                        return Err(format!(
                            "LSP request '{method}' timed out after {timeout_duration:?}"
                        )
                        .into());
                    }
                }
            }

            if self.should_error {
                return Err("LSP error: Mock error".into());
            }

            // Simulate successful response
            let mock_result = if let Some(size) = self.response_size {
                // Create a large response of specified size
                let large_string = "x".repeat(size);
                json!({"large_data": large_string})
            } else {
                json!({"mock": "result"})
            };

            // Check response size like the real implementation
            let result_json = serde_json::to_string(&mock_result)?;
            let max_size = get_max_response_size_for_method(method);
            if result_json.len() > max_size {
                return Err(format!(
                    "Response too large ({} bytes, max {} for {}). Try: smaller file, more specific query, or set RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE env var.",
                    result_json.len(),
                    max_size,
                    method
                ).into());
            }

            Ok(serde_json::from_value(mock_result)?)
        }
    }

    #[tokio::test]
    async fn test_timeout_constants_are_reasonable() {
        // Test that timeout constants are reasonable
        assert!(DEFAULT_TIMEOUT_SECS > 0);
        assert!(DEFAULT_TIMEOUT_SECS <= 120); // Should not be too long
        assert!(DEFAULT_TIMEOUT_SECS >= 10); // Should be long enough for normal operations

        // Test timeout duration creation
        let timeout_duration = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        assert_eq!(timeout_duration.as_secs(), DEFAULT_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn test_timeout_error_message_format() {
        // Test that we can create timeout error messages without panicking
        let method = "test/method";
        let timeout_duration = Duration::from_secs(1);
        let error_msg = format!("LSP request '{method}' timed out after {timeout_duration:?}");

        assert!(error_msg.contains("test/method"));
        assert!(error_msg.contains("timed out"));
        assert!(error_msg.contains("1s"));
    }

    #[tokio::test]
    async fn test_timeout_behavior_with_mock() {
        // Test actual timeout behavior
        let mock_client = MockLspClient::new(true, false); // should timeout
        let short_timeout = Duration::from_millis(50);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("test/method", json!({}), short_timeout)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("timed out"));
        assert!(error_msg.contains("test/method"));
    }

    #[tokio::test]
    async fn test_successful_request_within_timeout() {
        // Test that requests that complete within timeout work fine
        let mock_client = MockLspClient::new(false, false); // should not timeout or error
        let long_timeout = Duration::from_secs(10);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("test/method", json!({}), long_timeout)
            .await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["mock"], "result");
    }

    #[tokio::test]
    async fn test_error_handling_with_mock() {
        // Test that LSP errors are properly propagated
        let mock_client = MockLspClient::new(false, true); // should error
        let timeout = Duration::from_secs(5);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("test/method", json!({}), timeout)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("LSP error"));
        assert!(error_msg.contains("Mock error"));
    }

    #[tokio::test]
    async fn test_duration_operations() {
        // Test that Duration operations work as expected
        let duration = Duration::from_secs(5);
        assert_eq!(duration.as_secs(), 5);
        assert_eq!(duration.as_millis(), 5000);

        let short_duration = Duration::from_millis(100);
        assert_eq!(short_duration.as_millis(), 100);
        assert_eq!(short_duration.as_secs(), 0);

        // Test duration comparisons
        assert!(duration > short_duration);
        assert!(short_duration < duration);
    }

    #[tokio::test]
    async fn test_request_id_incrementing() {
        // Test that request IDs increment properly
        let mock_client = MockLspClient::new(false, false);

        // Make multiple requests and verify they don't interfere
        let _result1: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("method1", json!({}), Duration::from_secs(1))
            .await;

        let _result2: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("method2", json!({}), Duration::from_secs(1))
            .await;

        // Both should succeed if not configured to fail
        let result3: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("method3", json!({}), Duration::from_secs(1))
            .await;

        assert!(result3.is_ok());
    }

    #[tokio::test]
    async fn test_default_timeout_usage() {
        // Test that DEFAULT_TIMEOUT_SECS is used correctly
        let default_duration = Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let manual_duration = Duration::from_secs(60); // Should match DEFAULT_TIMEOUT_SECS

        assert_eq!(default_duration, manual_duration);
        assert_eq!(DEFAULT_TIMEOUT_SECS, 60);

        // Test that configurable timeout uses the default when no env var is set
        let configured_timeout = get_timeout_secs();
        assert_eq!(configured_timeout, DEFAULT_TIMEOUT_SECS);
    }

    #[tokio::test]
    async fn test_very_short_timeout() {
        // Test behavior with extremely short timeout
        let mock_client = MockLspClient::new(true, false);
        let very_short_timeout = Duration::from_millis(1);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("test/method", json!({}), very_short_timeout)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("timed out"));
        assert!(error_msg.contains("1ms"));
    }

    // Response size limit tests

    #[tokio::test]
    async fn test_response_size_constants() {
        // Test that response size constants are reasonable for LLM token limits
        assert!(MAX_RESPONSE_SIZE_BYTES > 0);
        assert!(MAX_RESPONSE_SIZE_BYTES >= 10 * 1024); // At least 10KB
        assert!(MAX_RESPONSE_SIZE_BYTES <= 1024 * 1024); // Not more than 1MB
        assert_eq!(MAX_RESPONSE_SIZE_BYTES, 40 * 1024); // 40KB ≈ 10k tokens

        assert!(MAX_LARGE_RESPONSE_SIZE_BYTES > MAX_RESPONSE_SIZE_BYTES);
        assert_eq!(MAX_LARGE_RESPONSE_SIZE_BYTES, 120 * 1024); // 120KB ≈ 30k tokens

        assert!(MAX_SYMBOLS_COUNT > 0);
        assert!(MAX_SYMBOLS_COUNT <= 10000); // Reasonable limit
        assert_eq!(MAX_SYMBOLS_COUNT, 200); // Exactly 200

        assert!(MAX_COMPLETION_ITEMS > 0);
        assert!(MAX_COMPLETION_ITEMS <= 200); // Reasonable UX limit
        assert_eq!(MAX_COMPLETION_ITEMS, 25); // Exactly 25
    }

    #[tokio::test]
    async fn test_small_response_size_accepted() {
        // Test that small responses are accepted
        let small_size = 1024; // 1KB
        let mock_client = MockLspClient::new_with_response_size(small_size);
        let timeout = Duration::from_secs(5);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("test/method", json!({}), timeout)
            .await;

        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value.get("large_data").is_some());
        assert_eq!(value["large_data"].as_str().unwrap().len(), small_size);
    }

    #[tokio::test]
    async fn test_large_response_size_rejected() {
        // Test that responses exceeding method-specific limits are rejected
        let large_size = MAX_RESPONSE_SIZE_BYTES + 1000; // Exceed default limit
        let mock_client = MockLspClient::new_with_response_size(large_size);
        let timeout = Duration::from_secs(5);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("textDocument/hover", json!({}), timeout)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Response too large"));
        assert!(error_msg.contains(&format!("{}", MAX_RESPONSE_SIZE_BYTES)));
        assert!(error_msg.contains("textDocument/hover"));
        assert!(error_msg.contains("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE"));
    }

    #[tokio::test]
    async fn test_response_at_exact_size_limit() {
        // Test response exactly at the size limit (should be accepted)
        // We need to account for JSON overhead, so use a slightly smaller size
        let json_overhead = 50; // Estimate for JSON structure
        let exact_size = MAX_RESPONSE_SIZE_BYTES - json_overhead;
        let mock_client = MockLspClient::new_with_response_size(exact_size);
        let timeout = Duration::from_secs(5);

        let result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("textDocument/hover", json!({}), timeout)
            .await;

        // This should succeed as we're under the limit when accounting for JSON structure
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_response_size_error_message_format() {
        // Test the format of response size error messages
        let large_size = MAX_RESPONSE_SIZE_BYTES + 5000;
        let method = "textDocument/hover";
        let max_size = get_max_response_size_for_method(method);
        let error_msg = format!(
            "Response too large ({} bytes, max {} for {}). Try: smaller file, more specific query, or set RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE env var.",
            large_size,
            max_size,
            method
        );

        assert!(error_msg.contains("Response too large"));
        assert!(error_msg.contains(&format!("{}", large_size)));
        assert!(error_msg.contains(&format!("{}", max_size)));
        assert!(error_msg.contains("textDocument/hover"));
        assert!(error_msg.contains("RUST_ANALYZER_MCP_MAX_RESPONSE_SIZE"));
        assert!(error_msg.contains("smaller file"));
        assert!(error_msg.contains("more specific query"));
    }

    #[tokio::test]
    async fn test_method_specific_size_limits() {
        // Test that different methods have different size limits
        let hover_limit = get_max_response_size_for_method("textDocument/hover");
        let completion_limit = get_max_response_size_for_method("textDocument/completion");
        let diagnostic_limit = get_max_response_size_for_method("textDocument/diagnostic");
        let symbol_limit = get_max_response_size_for_method("textDocument/documentSymbol");

        // Diagnostics should have the largest limit
        assert!(diagnostic_limit > hover_limit);
        assert!(diagnostic_limit > completion_limit);
        assert_eq!(diagnostic_limit, MAX_LARGE_RESPONSE_SIZE_BYTES);

        // Completions should have smaller limit (better UX)
        assert!(completion_limit < hover_limit);
        assert_eq!(completion_limit, MAX_RESPONSE_SIZE_BYTES / 2);

        // Hover and symbols use default limit
        assert_eq!(hover_limit, MAX_RESPONSE_SIZE_BYTES);
        assert_eq!(symbol_limit, MAX_RESPONSE_SIZE_BYTES);

        // Test with actual mock responses
        let large_size = MAX_RESPONSE_SIZE_BYTES + 1000;
        let mock_client = MockLspClient::new_with_response_size(large_size);
        let timeout = Duration::from_secs(5);

        // This should fail for hover (exceeds limit)
        let hover_result: Result<serde_json::Value, _> = mock_client
            .mock_request_with_timeout("textDocument/hover", json!({}), timeout)
            .await;
        assert!(hover_result.is_err());

        // But same size might succeed for diagnostics (higher limit)
        let diagnostic_size = MAX_LARGE_RESPONSE_SIZE_BYTES - 1000; // Under diagnostic limit
        let diagnostic_mock = MockLspClient::new_with_response_size(diagnostic_size);
        let diagnostic_result: Result<serde_json::Value, _> = diagnostic_mock
            .mock_request_with_timeout("textDocument/diagnostic", json!({}), timeout)
            .await;
        assert!(diagnostic_result.is_ok());
    }

    #[tokio::test]
    async fn test_combined_timeout_and_size_limits() {
        // Test interaction between timeout and size limits

        // First test: timeout should occur before size check for slow large responses
        let mock_client_timeout = MockLspClient::new(true, false);
        let short_timeout = Duration::from_millis(50);

        let result: Result<serde_json::Value, _> = mock_client_timeout
            .mock_request_with_timeout("test/method", json!({}), short_timeout)
            .await;

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("timed out"));

        // Second test: size limit should be checked for fast large responses
        let large_size = MAX_RESPONSE_SIZE_BYTES + 1000;
        let mock_client_large = MockLspClient::new_with_response_size(large_size);
        let long_timeout = Duration::from_secs(10);

        let result2: Result<serde_json::Value, _> = mock_client_large
            .mock_request_with_timeout("textDocument/hover", json!({}), long_timeout)
            .await;

        assert!(result2.is_err());
        let error_msg2 = result2.unwrap_err().to_string();
        assert!(error_msg2.contains("Response too large"));
    }

    #[tokio::test]
    async fn test_max_symbols_count_usage() {
        // Test that MAX_SYMBOLS_COUNT constant is used and reasonable
        assert!(MAX_SYMBOLS_COUNT > 0);
        assert!(MAX_SYMBOLS_COUNT <= 10000); // Reasonable upper bound
        assert_eq!(MAX_SYMBOLS_COUNT, 200); // Exactly what we expect

        // Test that the constant can be used in calculations
        let half_limit = MAX_SYMBOLS_COUNT / 2;
        assert_eq!(half_limit, 100);

        let double_limit = MAX_SYMBOLS_COUNT * 2;
        assert_eq!(double_limit, 400);
    }

    #[tokio::test]
    async fn test_configurable_timeout() {
        // Test default timeout
        let default_timeout = get_timeout_secs();
        assert_eq!(default_timeout, 60); // Default should be 60 seconds

        // Test that timeout is reasonable
        assert!(default_timeout > 0);
        assert!(default_timeout <= 300); // Not more than 5 minutes
    }

    #[tokio::test]
    async fn test_configurable_response_size() {
        // Test default response size
        let default_size = get_max_response_size();
        assert_eq!(default_size, MAX_RESPONSE_SIZE_BYTES);

        // Test that size is reasonable for LLM tokens
        assert!(default_size > 0);
        assert!(default_size <= 1024 * 1024); // Max 1MB
        assert!(default_size >= 10 * 1024); // Min 10KB
    }

    #[tokio::test]
    async fn test_environment_variable_configuration() {
        // Test that environment variables would be read (we can't easily set them in tests
        // without affecting other tests, but we can test the function exists and works)

        // These should return defaults when no env vars are set
        let timeout = get_timeout_secs();
        let size = get_max_response_size();

        assert!(timeout > 0);
        assert!(size > 0);

        // Test method-specific limits
        let hover_limit = get_max_response_size_for_method("textDocument/hover");
        let completion_limit = get_max_response_size_for_method("textDocument/completion");
        let diagnostic_limit = get_max_response_size_for_method("textDocument/diagnostic");

        assert!(hover_limit > 0);
        assert!(completion_limit > 0);
        assert!(diagnostic_limit > 0);
        assert!(diagnostic_limit > hover_limit); // Diagnostics should allow larger responses
    }

    #[tokio::test]
    async fn test_token_size_estimates() {
        // Test that our size limits make sense for token counts
        // ~4 chars per token average

        let chars_per_token = 4;
        let default_tokens = MAX_RESPONSE_SIZE_BYTES / chars_per_token;
        let large_tokens = MAX_LARGE_RESPONSE_SIZE_BYTES / chars_per_token;

        // Default should be around 10k tokens (good for most LLMs)
        assert!(default_tokens >= 8_000);
        assert!(default_tokens <= 12_000);

        // Large should be around 30k tokens (still reasonable for larger context models)
        assert!(large_tokens >= 25_000);
        assert!(large_tokens <= 35_000);

        // Completion limit should be much smaller for better UX
        let completion_limit = get_max_response_size_for_method("textDocument/completion");
        let completion_tokens = completion_limit / chars_per_token;
        assert!(completion_tokens <= 7_000); // Much smaller for completions
    }

    #[tokio::test]
    async fn test_completion_items_constant() {
        // Test MAX_COMPLETION_ITEMS constant
        assert!(MAX_COMPLETION_ITEMS > 0);
        assert!(MAX_COMPLETION_ITEMS <= 100);
        assert_eq!(MAX_COMPLETION_ITEMS, 25);

        // Test that it can be used in calculations
        let half_completion = MAX_COMPLETION_ITEMS / 2;
        assert_eq!(half_completion, 12);
    }

    #[tokio::test]
    async fn test_cache_warming_performance_estimation() {
        // Test performance estimation calculations
        let files_processed = 100;
        let duration_secs: f64 = 5.0;
        let files_per_sec = files_processed as f64 / duration_secs.max(0.001);

        assert_eq!(files_per_sec, 20.0);

        // Test with very small duration
        let small_duration: f64 = 0.0;
        let safe_files_per_sec = files_processed as f64 / small_duration.max(0.001);
        assert!(safe_files_per_sec > 0.0); // Should not divide by zero

        // Test realistic performance expectations
        assert!(safe_files_per_sec >= files_processed as f64 / 0.001); // At least this fast
    }

    #[tokio::test]
    async fn test_cache_warming_delay_calculation() {
        // Test the small delay used in cache warming
        let delay = Duration::from_millis(10);
        assert_eq!(delay.as_millis(), 10);
        assert!(delay < Duration::from_secs(1)); // Should be small

        // Test that delay is reasonable for not overwhelming rust-analyzer
        assert!(delay >= Duration::from_millis(1)); // Not too small
        assert!(delay <= Duration::from_millis(100)); // Not too large
    }
}
