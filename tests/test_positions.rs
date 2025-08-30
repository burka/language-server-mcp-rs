// Known good positions in our codebase for testing
// This file documents reliable positions for testing various LSP operations

/// Test positions in src/main.rs
pub mod main_rs {
    pub const FILE: &str = "src/main.rs";

    // Function definitions (reliable for goto_definition, hover)
    pub const MAIN_FUNCTION: (u32, u32) = (18, 10); // "async fn main()" on line 19 (0-indexed is 18)

    // Import statements
    pub const RUST_ANALYZER_IMPORT: (u32, u32) = (3, 30); // "RustAnalyzerMCP" in import

    // Struct usage
    pub const ARGS_PARSE: (u32, u32) = (19, 20); // "Args::parse()" on line 20
}

/// Test positions in src/lsp_client.rs  
pub mod lsp_client_rs {
    pub const FILE: &str = "src/lsp_client.rs";

    // Struct definitions
    pub const LSP_CLIENT_STRUCT: (u32, u32) = (36, 11); // "LspClient" in struct definition

    // Method definitions
    pub const NEW_METHOD: (u32, u32) = (96, 13); // "new" in "pub async fn new" (0-based)
    pub const HOVER_METHOD: (u32, u32) = (369, 13); // "hover" in "pub async fn hover" (0-based)
}

/// Test positions in src/models.rs
pub mod models_rs {
    pub const FILE: &str = "src/models.rs";

    // Request structs (good for hover, document_symbols)
    pub const HOVER_REQUEST_STRUCT: (u32, u32) = (7, 11); // "HoverRequest"
    pub const COMPLETION_REQUEST: (u32, u32) = (13, 11); // "CompletionRequest"

    // Field definitions (good for find_references)
    pub const FILE_PATH_FIELD: (u32, u32) = (8, 4); // "file_path" field
}

/// Test positions in src/errors.rs
pub mod errors_rs {
    pub const FILE: &str = "src/errors.rs";

    // Enum definition
    pub const LSP_ERROR_ENUM: (u32, u32) = (8, 11); // "LspError"

    // Enum variants (good for find_references)
    pub const RUST_ANALYZER_NOT_FOUND: (u32, u32) = (10, 4); // "RustAnalyzerNotFound"
    pub const TIMEOUT_ERROR: (u32, u32) = (22, 4); // "TimeoutError"
}

/// Non-existent files for error testing
pub mod invalid_files {
    pub const NON_EXISTENT: &str = "src/does_not_exist.rs";
    pub const PATH_TRAVERSAL: &str = "../../../etc/passwd";
    pub const EMPTY_PATH: &str = "";
}

/// Helper function to validate our test positions
pub async fn validate_positions() -> Result<(), String> {
    use std::fs;

    let files = [
        (
            main_rs::FILE,
            vec![
                main_rs::MAIN_FUNCTION,
                main_rs::RUST_ANALYZER_IMPORT,
                main_rs::ARGS_PARSE,
            ],
        ),
        (
            lsp_client_rs::FILE,
            vec![
                lsp_client_rs::LSP_CLIENT_STRUCT,
                lsp_client_rs::NEW_METHOD,
                lsp_client_rs::HOVER_METHOD,
            ],
        ),
        (
            models_rs::FILE,
            vec![
                models_rs::HOVER_REQUEST_STRUCT,
                models_rs::COMPLETION_REQUEST,
                models_rs::FILE_PATH_FIELD,
            ],
        ),
        (
            errors_rs::FILE,
            vec![
                errors_rs::LSP_ERROR_ENUM,
                errors_rs::RUST_ANALYZER_NOT_FOUND,
                errors_rs::TIMEOUT_ERROR,
            ],
        ),
    ];

    for (file_path, positions) in files {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        let lines: Vec<&str> = content.lines().collect();

        for (line, column) in positions {
            // Convert from 0-indexed to 1-indexed for validation
            let line_idx = line as usize;
            let col_idx = column as usize;

            if line_idx >= lines.len() {
                return Err(format!(
                    "Position {}:{} in {} is out of bounds (file has {} lines)",
                    line,
                    column,
                    file_path,
                    lines.len()
                ));
            }

            if col_idx >= lines[line_idx].len() {
                return Err(format!(
                    "Position {}:{} in {} is out of bounds (line {} has {} characters)",
                    line,
                    column,
                    file_path,
                    line,
                    lines[line_idx].len()
                ));
            }
        }
    }

    println!("✅ All test positions validated successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_positions() {
        validate_positions()
            .await
            .expect("Test positions should be valid");
    }
}
