//! YAML format module providing parsing, formatting, and syntax highlighting.

pub mod formatter;
pub mod model;
pub mod parser;
pub mod syntax;

pub use formatter::{format_yaml, minify_yaml};
pub use model::YamlType;
pub use parser::{build_yaml_diagnostic, YamlDiagnostic, YamlParser, MAX_PARSE_DEPTH};
pub use syntax::tokenize_yaml_line;

use crate::formats::json::JsonSpan;
use crate::formats::node::TreeNode;

/// Parses a YAML string into an optional AST node with source spans.
///
/// Returns `Ok(None)` if the source is empty or whitespace/comment only.
///
/// Multi-document streams are returned as a synthetic root sequence with one
/// child per document, mirroring how JSON Lines is presented.
///
/// Complexity: O(N) where N is the length of the YAML string.
pub fn parse_yaml(source: &str) -> Result<Option<TreeNode>, YamlDiagnostic> {
    let parser = YamlParser::new(source)?;
    parser.parse()
}

/// Re-exported for convenience so YAML consumers do not reach into the JSON
/// module for the shared span primitive.
pub type YamlSpan = JsonSpan;
