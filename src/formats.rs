//! Document format implementations.
//!
//! Each format module owns its own parser, formatter, and syntax highlighter,
//! while emitting the shared [`node::TreeNode`] model so the rest of the
//! application stays format-agnostic.

pub mod diagnostic;
pub mod json;
pub mod node;
pub mod toml;
pub mod yaml;

pub use diagnostic::{Diagnostic, FileFormat};
pub use node::{NodeType, TreeNode};
