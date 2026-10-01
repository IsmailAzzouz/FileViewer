//! TOML format module providing parsing, formatting, and syntax highlighting.

pub mod formatter;
pub mod model;
pub mod parser;
pub mod syntax;

pub use formatter::{format_toml, minify_toml};
pub use model::TomlType;
pub use parser::{build_toml_diagnostic, TomlDiagnostic, TomlParser, MAX_PARSE_DEPTH};
pub use syntax::tokenize_toml_line;

use crate::formats::json::JsonSpan;
use crate::formats::node::TreeNode;

/// Parses a TOML string into an optional AST node with source spans.
///
/// Returns `Ok(None)` if the source is empty or whitespace/comment only.
///
/// Complexity: O(N) where N is the length of the TOML string.
pub fn parse_toml(source: &str) -> Result<Option<TreeNode>, TomlDiagnostic> {
    let parser = TomlParser::new(source)?;
    parser.parse()
}

/// Re-exported for convenience so TOML consumers do not reach into the JSON
/// module for the shared span primitive.
pub type TomlSpan = JsonSpan;
