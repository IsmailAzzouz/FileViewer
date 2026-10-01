//! JSON domain models and shared span type.
//!
//! Parsing produces the format-agnostic [`crate::formats::node::TreeNode`]
//! model. [`JsonType`] and [`JsonSpan`] remain format-specific: `JsonType`
//! identifies JSON value kinds, and `JsonSpan` is the shared source-location
//! primitive reused by every format.

use crate::formats::node::NodeType;

/// Represents a source code location range in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct JsonSpan {
    /// Zero-based byte offset where the token/node starts.
    pub start_byte: usize,
    /// Zero-based byte offset where the token/node ends (exclusive).
    pub end_byte: usize,
    /// 1-based line number where the node starts.
    pub start_line: usize,
    /// 1-based column number where the node starts.
    pub start_col: usize,
    /// 1-based line number where the node ends.
    pub end_line: usize,
    /// 1-based column number where the node ends.
    pub end_col: usize,
}

impl JsonSpan {
    /// Creates a new span with byte offsets and line/col locations.
    pub fn new(
        start_byte: usize,
        end_byte: usize,
        start_line: usize,
        start_col: usize,
        end_line: usize,
        end_col: usize,
    ) -> Self {
        Self {
            start_byte,
            end_byte,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Checks if a given byte offset falls within this span.
    ///
    /// Complexity: O(1).
    pub fn contains_offset(&self, offset: usize) -> bool {
        offset >= self.start_byte && offset < self.end_byte
    }
}

/// JSON value types supported by RFC 8259.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonType {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

impl JsonType {
    /// Returns a short human-readable name for the type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Object => "object",
            Self::Array => "array",
            Self::String => "string",
            Self::Number => "number",
            Self::Boolean => "boolean",
            Self::Null => "null",
        }
    }

    /// Returns a concise symbol or acronym representing the type for UI badges.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Object => "{ }",
            Self::Array => "[ ]",
            Self::String => "str",
            Self::Number => "num",
            Self::Boolean => "bool",
            Self::Null => "null",
        }
    }

    /// Returns true when this type can contain child nodes.
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Object | Self::Array)
    }
}

impl From<JsonType> for NodeType {
    fn from(t: JsonType) -> Self {
        NodeType::Json(t)
    }
}
