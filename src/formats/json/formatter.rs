//! Formatting and minification utilities for JSON documents.

use super::parser::{build_diagnostic, JsonDiagnostic};

/// Formats a raw JSON string with consistent indentation.
///
/// Preserves key order from the input document.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn format_json(raw: &str, indent_spaces: usize) -> Result<String, JsonDiagnostic> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| {
        let line = err.line();
        let col = err.column();
        // Calculate byte offset approximately from line and col
        let byte_offset = calculate_byte_offset(raw, line, col);
        build_diagnostic(
            raw,
            byte_offset,
            line,
            col,
            format!("Invalid JSON: {}", err),
        )
    })?;

    let indent_bytes = " ".repeat(indent_spaces).into_bytes();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent_bytes);
    let mut buffer = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
    serde::Serialize::serialize(&value, &mut serializer).map_err(|err| {
        build_diagnostic(raw, 0, 1, 1, format!("Failed to serialize JSON: {}", err))
    })?;

    String::from_utf8(buffer)
        .map_err(|err| build_diagnostic(raw, 0, 1, 1, format!("UTF-8 encoding error: {}", err)))
}

/// Minifies a raw JSON string by removing all insignificant whitespace.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn minify_json(raw: &str) -> Result<String, JsonDiagnostic> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| {
        let line = err.line();
        let col = err.column();
        let byte_offset = calculate_byte_offset(raw, line, col);
        build_diagnostic(
            raw,
            byte_offset,
            line,
            col,
            format!("Invalid JSON: {}", err),
        )
    })?;

    serde_json::to_string(&value)
        .map_err(|err| build_diagnostic(raw, 0, 1, 1, format!("Failed to minify JSON: {}", err)))
}

/// Helper function to compute byte offset from 1-based line and column.
///
/// Complexity: O(N).
fn calculate_byte_offset(source: &str, line: usize, col: usize) -> usize {
    let mut current_line = 1;
    let mut current_col = 1;

    for (offset, ch) in source.char_indices() {
        if current_line == line && current_col >= col {
            return offset;
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }
    source.len()
}
