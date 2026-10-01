//! JSON format module providing parsing, formatting, and syntax highlighting.
//!
//! Four dialects share this module: strict RFC 8259 JSON, JSON with Comments,
//! and JSON Lines all parse into the same format-agnostic
//! [`TreeNode`](crate::formats::node::TreeNode) model with
//! [`JsonType`] leaves, so consumers never branch on dialect.

pub mod formatter;
pub mod model;
pub mod parser;
pub mod syntax;

pub use formatter::{
    format_json, format_jsonc, format_jsonl, minify_json, minify_jsonc, minify_jsonl,
};
pub use model::{JsonSpan, JsonType};
pub use parser::{
    build_diagnostic, parse_jsonc, parse_jsonl, JsonDiagnostic, JsonDialect, JsonParser,
    MAX_PARSE_DEPTH,
};
pub use syntax::{tokenize_json_line, tokenize_jsonc_line, StyledSegment};

use crate::formats::node::TreeNode;

/// Parses a JSON string into an optional AST node with source spans.
///
/// Returns `Ok(None)` if the source is empty or whitespace only.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn parse_json(source: &str) -> Result<Option<TreeNode>, JsonDiagnostic> {
    let parser = JsonParser::new(source)?;
    parser.parse()
}
