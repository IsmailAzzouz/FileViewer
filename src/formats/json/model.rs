//! JSON domain models and tree representation.
//!
//! Provides structured representations of parsed JSON values with exact source
//! location spans to enable two-way synchronization between the text editor and
//! the hierarchical tree view.

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
}

/// A node in the JSON syntax tree.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonTreeNode {
    /// Unique identifier within the tree for stable expansion/selection tracking.
    pub id: usize,
    /// Object key if this node is an entry in a parent object, or None for root / array elements.
    pub key: Option<String>,
    /// Exact source span of the object key (if this node represents a key-value entry).
    pub key_span: Option<JsonSpan>,
    /// JSON data type of this node.
    pub json_type: JsonType,
    /// Formatted preview of the value for display in the tree.
    pub value_preview: String,
    /// JSONPath expression representing this node's location (e.g. `$.users[0].name`).
    pub path: String,
    /// Exact source span of the value in the editor buffer.
    pub span: JsonSpan,
    /// Child nodes (for Object and Array types).
    pub children: Vec<JsonTreeNode>,
}

impl JsonTreeNode {
    /// Returns the total number of nodes in this subtree.
    ///
    /// Complexity: O(N) where N is the number of descendant nodes.
    pub fn total_node_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(JsonTreeNode::total_node_count)
            .sum::<usize>()
    }

    /// Returns the full span including the object key if present.
    ///
    /// Complexity: O(1).
    pub fn full_span(&self) -> JsonSpan {
        if let Some(key_span) = self.key_span {
            JsonSpan::new(
                key_span.start_byte,
                self.span.end_byte,
                key_span.start_line,
                key_span.start_col,
                self.span.end_line,
                self.span.end_col,
            )
        } else {
            self.span
        }
    }

    /// Finds the node whose span (or key span) most tightly encloses the given byte offset.
    ///
    /// Complexity: O(D) where D is the depth of the tree, worst case O(N).
    pub fn find_node_at_offset(&self, offset: usize) -> Option<&JsonTreeNode> {
        let inside = self.span.contains_offset(offset)
            || offset == self.span.end_byte
            || self
                .key_span
                .is_some_and(|k| k.contains_offset(offset) || offset == k.end_byte);

        if !inside {
            return None;
        }

        for child in &self.children {
            if let Some(matching) = child.find_node_at_offset(offset) {
                return Some(matching);
            }
        }

        Some(self)
    }

    /// Checks if this node directly matches the filter query.
    ///
    /// Complexity: O(K + V + P) where K, V, P are lengths of key, preview, and path.
    pub fn matches_filter(&self, query_lower: &str) -> bool {
        self.key
            .as_deref()
            .is_some_and(|k| k.to_lowercase().contains(query_lower))
            || self.value_preview.to_lowercase().contains(query_lower)
            || self.path.to_lowercase().contains(query_lower)
    }

    /// Checks if any descendant of this node matches the filter query.
    ///
    /// Complexity: O(N) where N is the subtree size.
    pub fn has_matching_descendant(&self, query_lower: &str) -> bool {
        self.children
            .iter()
            .any(|c| c.matches_filter(query_lower) || c.has_matching_descendant(query_lower))
    }

    /// Finds a node by its unique ID.
    ///
    /// Complexity: O(N) where N is the number of nodes in the subtree.
    pub fn find_node_by_id(&self, target_id: usize) -> Option<&JsonTreeNode> {
        if self.id == target_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_node_by_id(target_id) {
                return Some(found);
            }
        }
        None
    }
}
