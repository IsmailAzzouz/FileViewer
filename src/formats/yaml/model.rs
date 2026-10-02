//! YAML domain models and type definitions.
//!
//! YAML parsing emits the format-agnostic [`crate::formats::node::TreeNode`]
//! model. [`YamlType`] remains YAML-specific and is carried inside
//! [`crate::formats::node::NodeType`].
//!
//! There is deliberately no `Alias` variant: an alias resolves to a real node
//! type, so alias nodes need no special case in the tree view.

use crate::formats::node::NodeType;

/// YAML value types supported by this viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YamlType {
    /// A block or flow mapping (`key: value`).
    Mapping,
    /// A block or flow sequence (`- item`).
    Sequence,
    /// A plain, single-quoted, double-quoted, or block scalar string.
    String,
    /// An integer scalar.
    Integer,
    /// A floating point scalar.
    Float,
    /// A boolean scalar, including the YAML 1.1 `yes`/`no`/`on`/`off` spellings.
    Boolean,
    /// `null`, `~`, or an empty value.
    Null,
}

impl YamlType {
    /// Returns a short human-readable name for the type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Mapping => "mapping",
            Self::Sequence => "sequence",
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Null => "null",
        }
    }

    /// Returns a concise symbol or acronym representing the type for UI badges.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Mapping => "{ }",
            Self::Sequence => "[ ]",
            Self::String => "str",
            Self::Integer => "int",
            Self::Float => "flt",
            Self::Boolean => "bool",
            Self::Null => "null",
        }
    }

    /// Returns true when this type can contain child nodes.
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Mapping | Self::Sequence)
    }
}

impl From<YamlType> for NodeType {
    fn from(t: YamlType) -> Self {
        NodeType::Yaml(t)
    }
}
