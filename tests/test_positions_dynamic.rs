// Dynamic test positions that automatically adapt to source changes
// This replaces the fragile hardcoded positions with runtime discovery

mod position_finder;
use position_finder::*;

/// Dynamically discovered positions in src/main.rs
pub mod main_rs {
    use super::*;

    pub const FILE: &str = "src/main.rs";

    pub fn main_function() -> (u32, u32) {
        find_function(FILE, "main")
            .expect("main function should exist")
            .to_tuple()
    }

    pub fn rust_analyzer_import() -> (u32, u32) {
        find_import(FILE, "RustAnalyzerMCP")
            .expect("RustAnalyzerMCP import should exist")
            .to_tuple()
    }

    pub fn args_parse() -> (u32, u32) {
        find_usage(FILE, r"Args::parse\(\)")
            .expect("Args::parse() call should exist")
            .to_tuple()
    }
}

/// Dynamically discovered positions in src/lsp_client.rs
pub mod lsp_client_rs {
    use super::*;

    pub const FILE: &str = "src/lsp_client.rs";

    pub fn lsp_client_struct() -> (u32, u32) {
        find_struct(FILE, "LspClient")
            .expect("LspClient struct should exist")
            .to_tuple()
    }

    pub fn new_method() -> (u32, u32) {
        find_function(FILE, "new")
            .expect("new method should exist")
            .to_tuple()
    }

    pub fn hover_method() -> (u32, u32) {
        find_function(FILE, "hover")
            .expect("hover method should exist")
            .to_tuple()
    }
}

/// Dynamically discovered positions in src/models.rs
pub mod models_rs {
    use super::*;

    pub const FILE: &str = "src/models.rs";

    pub fn hover_request_struct() -> (u32, u32) {
        find_struct(FILE, "HoverRequest")
            .expect("HoverRequest struct should exist")
            .to_tuple()
    }

    pub fn completion_request() -> (u32, u32) {
        find_struct(FILE, "CompletionRequest")
            .expect("CompletionRequest struct should exist")
            .to_tuple()
    }

    pub fn file_path_field() -> (u32, u32) {
        find_field(FILE, "file_path")
            .expect("file_path field should exist")
            .to_tuple()
    }
}

/// Dynamically discovered positions in src/errors.rs
pub mod errors_rs {
    use super::*;

    pub const FILE: &str = "src/errors.rs";

    pub fn lsp_error_enum() -> (u32, u32) {
        find_enum(FILE, "LspError")
            .expect("LspError enum should exist")
            .to_tuple()
    }

    pub fn rust_analyzer_not_found() -> (u32, u32) {
        find_enum_variant(FILE, "RustAnalyzerNotFound")
            .expect("RustAnalyzerNotFound variant should exist")
            .to_tuple()
    }

    pub fn timeout_error() -> (u32, u32) {
        find_enum_variant(FILE, "TimeoutError")
            .expect("TimeoutError variant should exist")
            .to_tuple()
    }
}

/// Non-existent files for error testing (these remain static)
pub mod invalid_files {
    pub const NON_EXISTENT: &str = "src/does_not_exist.rs";
    pub const PATH_TRAVERSAL: &str = "../../../etc/passwd";
    pub const EMPTY_PATH: &str = "";
}

/// Helper function to validate our dynamically discovered positions
pub async fn validate_positions() -> Result<(), String> {
    use std::fs;

    // Test that we can find all positions
    let positions = vec![
        (main_rs::FILE, main_rs::main_function()),
        (main_rs::FILE, main_rs::rust_analyzer_import()),
        (main_rs::FILE, main_rs::args_parse()),
        (lsp_client_rs::FILE, lsp_client_rs::lsp_client_struct()),
        (lsp_client_rs::FILE, lsp_client_rs::new_method()),
        (lsp_client_rs::FILE, lsp_client_rs::hover_method()),
        (models_rs::FILE, models_rs::hover_request_struct()),
        (models_rs::FILE, models_rs::completion_request()),
        (models_rs::FILE, models_rs::file_path_field()),
        (errors_rs::FILE, errors_rs::lsp_error_enum()),
        (errors_rs::FILE, errors_rs::rust_analyzer_not_found()),
        (errors_rs::FILE, errors_rs::timeout_error()),
    ];

    for (file_path, (line, column)) in positions {
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

        let lines: Vec<&str> = content.lines().collect();

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

    println!("✅ All dynamically discovered positions validated successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_dynamic_positions() {
        validate_positions()
            .await
            .expect("Dynamically discovered positions should be valid");
    }

    #[test]
    fn test_positions_are_discoverable() {
        // Test that all key positions can be found
        assert!(main_rs::main_function().0 > 0);
        assert!(lsp_client_rs::lsp_client_struct().0 > 0);
        assert!(models_rs::hover_request_struct().0 > 0);
        assert!(errors_rs::lsp_error_enum().0 > 0);
    }
}
