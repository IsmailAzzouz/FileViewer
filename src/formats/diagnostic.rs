//! Unified validation diagnostics across supported document formats.

use crate::formats::json::JsonDiagnostic;
use crate::formats::toml::TomlDiagnostic;

/// A parse or format diagnostic from any supported document format.
#[derive(Debug, Clone)]
pub enum Diagnostic {
    Json(JsonDiagnostic),
    Toml(TomlDiagnostic),
}

impl Diagnostic {
    /// 1-based line number where the error occurred.
    pub fn line(&self) -> usize {
        match self {
            Diagnostic::Json(d) => d.line,
            Diagnostic::Toml(d) => d.line,
        }
    }

    /// 1-based column number where the error occurred.
    pub fn column(&self) -> usize {
        match self {
            Diagnostic::Json(d) => d.column,
            Diagnostic::Toml(d) => d.column,
        }
    }

    /// Human-readable explanation of the error.
    pub fn message(&self) -> &str {
        match self {
            Diagnostic::Json(d) => &d.message,
            Diagnostic::Toml(d) => &d.message,
        }
    }

    /// Contextual source lines surrounding the error.
    pub fn context_snippet(&self) -> &str {
        match self {
            Diagnostic::Json(d) => &d.context_snippet,
            Diagnostic::Toml(d) => &d.context_snippet,
        }
    }

    /// Caret pointer alignment string (e.g. `      ^`).
    pub fn pointer(&self) -> &str {
        match self {
            Diagnostic::Json(d) => &d.pointer,
            Diagnostic::Toml(d) => &d.pointer,
        }
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Diagnostic::Json(d) => write!(f, "{}", d),
            Diagnostic::Toml(d) => write!(f, "{}", d),
        }
    }
}

/// Supported file formats for the viewer.
///
/// All three JSON variants share [`JsonType`](crate::formats::json::JsonType)
/// leaves, so only this enum and the parser/formatter dispatch differ between
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileFormat {
    /// RFC 8259 JSON documents.
    #[default]
    Json,
    /// JSON with Comments, allowing `//` and `/* */` comments and trailing commas.
    JsonC,
    /// JSON Lines: one JSON value per line.
    JsonL,
    /// TOML documents.
    Toml,
}

impl FileFormat {
    /// Detects the file format from the file extension.
    ///
    /// Unrecognized extensions fall back to JSON, preserving the historical
    /// behaviour of opening arbitrary files as JSON.
    pub fn from_path(path: &std::path::Path) -> Self {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("jsonc") => FileFormat::JsonC,
            Some("jsonl") | Some("ndjson") => FileFormat::JsonL,
            Some("toml") => FileFormat::Toml,
            _ => FileFormat::Json,
        }
    }

    /// Returns the default file extension for this format.
    pub fn default_extension(&self) -> &'static str {
        match self {
            FileFormat::Json => "json",
            FileFormat::JsonC => "jsonc",
            FileFormat::JsonL => "jsonl",
            FileFormat::Toml => "toml",
        }
    }

    /// Returns a human-readable name for the format.
    pub fn name(&self) -> &'static str {
        match self {
            FileFormat::Json => "JSON",
            FileFormat::JsonC => "JSONC",
            FileFormat::JsonL => "JSONL",
            FileFormat::Toml => "TOML",
        }
    }
}
