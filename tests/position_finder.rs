/// Dynamic position finder for tests
/// This module provides utilities to find positions of Rust symbols dynamically
/// at runtime, making tests resilient to source code changes.
use std::fs;

/// Represents a position in a file (0-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

impl Position {
    #[allow(dead_code)]
    pub fn new(line: u32, column: u32) -> Self {
        Self { line, column }
    }

    pub fn to_tuple(self) -> (u32, u32) {
        (self.line, self.column)
    }
}

/// Find the position of a Rust struct definition
pub fn find_struct(file_path: &str, struct_name: &str) -> Result<Position, String> {
    find_pattern(file_path, &format!(r"pub\s+struct\s+{}\b", struct_name))
        .or_else(|_| find_pattern(file_path, &format!(r"struct\s+{}\b", struct_name)))
}

/// Find the position of a Rust enum definition
pub fn find_enum(file_path: &str, enum_name: &str) -> Result<Position, String> {
    find_pattern(file_path, &format!(r"pub\s+enum\s+{}\b", enum_name))
        .or_else(|_| find_pattern(file_path, &format!(r"enum\s+{}\b", enum_name)))
}

/// Find the position of a Rust function definition
pub fn find_function(file_path: &str, fn_name: &str) -> Result<Position, String> {
    // Try various function patterns
    let patterns = vec![
        format!(r"pub\s+async\s+fn\s+{}\b", fn_name),
        format!(r"pub\s+fn\s+{}\b", fn_name),
        format!(r"async\s+fn\s+{}\b", fn_name),
        format!(r"fn\s+{}\b", fn_name),
    ];

    for pattern in patterns {
        if let Ok(pos) = find_pattern(file_path, &pattern) {
            return Ok(pos);
        }
    }

    Err(format!("Function '{}' not found in {}", fn_name, file_path))
}

/// Find the position of an enum variant
pub fn find_enum_variant(file_path: &str, variant_name: &str) -> Result<Position, String> {
    // Look for enum variant pattern (indented, typically starts with spaces)
    find_pattern(file_path, &format!(r"^\s+{}\b", variant_name))
}

/// Find the position of a field in a struct
pub fn find_field(file_path: &str, field_name: &str) -> Result<Position, String> {
    // Look for field pattern (pub field_name: or just field_name:)
    find_pattern(file_path, &format!(r"^\s+(?:pub\s+)?{}\s*:", field_name))
}

/// Find the position of an import
pub fn find_import(file_path: &str, import_name: &str) -> Result<Position, String> {
    // Look for use statements containing the import
    find_pattern(file_path, &format!(r"use\s+.*\b{}\b", import_name))
}

/// Find the position of a method call or usage
pub fn find_usage(file_path: &str, usage_pattern: &str) -> Result<Position, String> {
    find_pattern(file_path, usage_pattern)
}

/// Generic pattern finder using regex
fn find_pattern(file_path: &str, pattern: &str) -> Result<Position, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    let regex =
        regex::Regex::new(pattern).map_err(|e| format!("Invalid pattern '{}': {}", pattern, e))?;

    for (line_num, line) in content.lines().enumerate() {
        if let Some(mat) = regex.find(line) {
            // Find the actual symbol name within the match
            let match_start = mat.start();

            // Try to find the exact position of the identifier
            // Skip keywords and whitespace to find the actual name
            let after_keywords = line[match_start..]
                .trim_start()
                .trim_start_matches("pub")
                .trim_start()
                .trim_start_matches("async")
                .trim_start()
                .trim_start_matches("fn")
                .trim_start()
                .trim_start_matches("struct")
                .trim_start()
                .trim_start_matches("enum")
                .trim_start();

            let name_offset = line.len() - after_keywords.len();

            return Ok(Position {
                line: line_num as u32,
                column: name_offset as u32,
            });
        }
    }

    Err(format!("Pattern '{}' not found in {}", pattern, file_path))
}

/// Find position with exact string match (simpler but less flexible)
#[allow(dead_code)]
pub fn find_exact(file_path: &str, exact_text: &str) -> Result<Position, String> {
    let content = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path, e))?;

    for (line_num, line) in content.lines().enumerate() {
        if let Some(col) = line.find(exact_text) {
            return Ok(Position {
                line: line_num as u32,
                column: col as u32,
            });
        }
    }

    Err(format!("Text '{}' not found in {}", exact_text, file_path))
}

/// Cached position finder for better performance
#[allow(dead_code)]
pub struct PositionCache {
    cache: std::collections::HashMap<String, Position>,
}

#[allow(dead_code)]
impl PositionCache {
    pub fn new() -> Self {
        Self {
            cache: std::collections::HashMap::new(),
        }
    }

    pub fn get_or_find<F>(&mut self, key: &str, finder: F) -> Result<Position, String>
    where
        F: FnOnce() -> Result<Position, String>,
    {
        if let Some(&pos) = self.cache.get(key) {
            return Ok(pos);
        }

        let pos = finder()?;
        self.cache.insert(key.to_string(), pos);
        Ok(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_struct() {
        // Test finding LspClient struct in our codebase
        let pos = find_struct("src/lsp_client.rs", "LspClient").unwrap();
        assert!(pos.line > 0, "Should find LspClient struct");
    }

    #[test]
    fn test_find_function() {
        // Test finding main function
        let pos = find_function("src/main.rs", "main").unwrap();
        assert!(pos.line > 0, "Should find main function");
    }

    #[test]
    fn test_find_enum() {
        // Test finding LspError enum
        let pos = find_enum("src/errors.rs", "LspError").unwrap();
        assert!(pos.line > 0, "Should find LspError enum");
    }
}
