//! JSON format module providing parsing, formatting, and tree modeling.

pub mod formatter;
pub mod model;
pub mod parser;
pub mod syntax;

pub use formatter::{format_json, minify_json};
pub use model::{JsonSpan, JsonTreeNode, JsonType};
pub use parser::{JsonDiagnostic, JsonParser, MAX_PARSE_DEPTH};
pub use syntax::{tokenize_json_line, StyledSegment};

/// Parses a JSON string into an optional AST node with source spans.
///
/// Returns `Ok(None)` if the source is empty or whitespace only.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn parse_json(source: &str) -> Result<Option<JsonTreeNode>, JsonDiagnostic> {
    let parser = JsonParser::new(source)?;
    parser.parse()
}
