//! TOML domain models and type definitions.
//!
//! TOML parsing emits the format-agnostic [`crate::formats::node::TreeNode`]
//! model. [`TomlType`] remains TOML-specific and is carried inside
//! [`crate::formats::node::NodeType`].

use crate::formats::node::NodeType;

/// TOML value types supported by the TOML v1.0.0 specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TomlType {
    /// A standard table declared with `[header]`.
    Table,
    /// An array value, which may hold arrays of tables.
    Array,
    /// A basic or literal string.
    String,
    /// A signed integer.
    Integer,
    /// A floating point number.
    Float,
    /// A boolean.
    Boolean,
    /// A date, time, or date-time.
    Datetime,
    /// An inline table written as `{ key = value }`.
    InlineTable,
}

impl TomlType {
    /// Returns a short human-readable name for the type.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Table => "table",
            Self::Array => "array",
            Self::String => "string",
            Self::Integer => "integer",
            Self::Float => "float",
            Self::Boolean => "boolean",
            Self::Datetime => "datetime",
            Self::InlineTable => "inline-table",
        }
    }

    /// Returns a concise symbol or acronym representing the type for UI badges.
    pub fn badge_text(&self) -> &'static str {
        match self {
            Self::Table | Self::InlineTable => "{ }",
            Self::Array => "[ ]",
            Self::String => "str",
            Self::Integer => "int",
            Self::Float => "flt",
            Self::Boolean => "bool",
            Self::Datetime => "dt",
        }
    }

    /// Returns true when this type can contain child nodes.
    pub fn is_container(&self) -> bool {
        matches!(self, Self::Table | Self::Array | Self::InlineTable)
    }
}

impl From<TomlType> for NodeType {
    fn from(t: TomlType) -> Self {
        NodeType::Toml(t)
    }
}
