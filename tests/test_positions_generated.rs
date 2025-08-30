// Test using build-time generated positions
// These positions are automatically updated whenever source code changes

// Include the generated positions
include!(concat!(env!("OUT_DIR"), "/test_positions_generated.rs"));

/// Helper function to validate our generated positions
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

            if col_idx > lines[line_idx].len() {
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

    println!("✅ All generated test positions validated successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_generated_positions() {
        validate_positions()
            .await
            .expect("Generated positions should be valid");
    }

    #[test]
    fn test_generated_constants_exist() {
        // Verify that build.rs generated all expected constants
        assert!(
            main_rs::MAIN_FUNCTION.0 > 0,
            "main function position should be found"
        );
        assert!(
            lsp_client_rs::LSP_CLIENT_STRUCT.0 > 0,
            "LspClient struct position should be found"
        );
        assert!(
            models_rs::HOVER_REQUEST_STRUCT.0 > 0,
            "HoverRequest struct position should be found"
        );
        assert!(
            errors_rs::LSP_ERROR_ENUM.0 > 0,
            "LspError enum position should be found"
        );
    }
}
