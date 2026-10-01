//! Format-agnostic tree model shared by every supported document format.
//!
//! Both the JSON and TOML front-ends emit [`TreeNode`] values so the tree view,
//! symbol index, and editor/tree synchronization only ever deal with one node
//! shape. The concrete format is preserved through [`NodeType`], which keeps
//! format-specific type identity and display metadata intact.

use crate::formats::json::{JsonSpan, JsonType};
use crate::formats::toml::TomlType;

/// Format-specific value type of a [`TreeNode`].
///
/// The wrapper preserves the originating format so the UI can render
/// format-accurate badges, type names, and syntax colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Json(JsonType),
    Toml(TomlType),
}

impl NodeType {
    /// Returns a short human-readable name for the type.
    pub fn type_name(&self) -> &'static str {
        match self {
            NodeType::Json(t) => t.type_name(),
            NodeType::Toml(t) => t.type_name(),
        }
    }

    /// Returns a concise symbol or acronym representing the type for UI badges.
    pub fn badge_text(&self) -> &'static str {
        match self {
            NodeType::Json(t) => t.badge_text(),
            NodeType::Toml(t) => t.badge_text(),
        }
    }

    /// Returns true when the type can contain child nodes.
    pub fn is_container(&self) -> bool {
        match self {
            NodeType::Json(t) => matches!(t, JsonType::Object | JsonType::Array),
            NodeType::Toml(t) => matches!(
                t,
                TomlType::Table | TomlType::Array | TomlType::InlineTable
            ),
        }
    }
}

/// A node in a document syntax tree.
///
/// Nodes form a single recursive structure regardless of source format, which
/// lets the tree view, symbol index, and cursor synchronization share one code
/// path.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    /// Unique identifier within the tree for stable expansion/selection tracking.
    pub id: usize,
    /// Object key if this node is an entry in a parent container, or None for root / array elements.
    pub key: Option<String>,
    /// Exact source span of the key (if this node represents a key-value entry).
    pub key_span: Option<JsonSpan>,
    /// Format-specific data type of this node.
    pub node_type: NodeType,
    /// Formatted preview of the value for display in the tree.
    pub value_preview: String,
    /// Path expression representing this node's location (e.g. `$.users[0].name`).
    pub path: String,
    /// Exact source span of the value in the editor buffer.
    pub span: JsonSpan,
    /// Child nodes (for container types).
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    /// Returns the total number of nodes in this subtree.
    ///
    /// Complexity: O(N) where N is the number of descendant nodes.
    pub fn total_node_count(&self) -> usize {
        1 + self
            .children
            .iter()
            .map(TreeNode::total_node_count)
            .sum::<usize>()
    }

    /// Returns the full span including the key if present.
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

    /// Returns true if this node can contain children.
    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    /// Finds the node whose span (or key span) most tightly encloses the given byte offset.
    ///
    /// Complexity: O(D) where D is the depth of the tree, worst case O(N).
    pub fn find_node_at_offset(&self, offset: usize) -> Option<&TreeNode> {
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
    pub fn find_node_by_id(&self, target_id: usize) -> Option<&TreeNode> {
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
