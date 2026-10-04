//! YAML parser with accurate source span tracking and contextual diagnostics.
//!
//! Implements the block and flow constructs that appear in real `.yml` files
//! while preserving exact start and end byte offsets, lines, and columns for
//! every key, scalar, and collection. Spans are computed during a single
//! forward scan, which is what enables two-way synchronization between the
//! editor and the tree view.
//!
//! The parser is line-oriented because YAML structure is indentation-driven.
//! Byte positions are derived from a precomputed line table, so every span is
//! absolute to the original document buffer.
//!
//! Deliberate scope decisions, documented in the working session plan:
//!
//! * Aliases (`*name`) expand to a clone of the anchored node so the tree
//!   matches resolved data. Merge keys (`<<:`) stay ordinary keys.
//! * Tags (`!!str`, `!Custom`) are recorded in the preview, not resolved.
//! * Complex keys (`? ` / `: `) are rejected with a diagnostic.

use std::collections::HashMap;

use super::model::YamlType;
use crate::formats::json::JsonSpan;
use crate::formats::node::{NodeType, TreeNode};

/// Maximum allowed nesting depth for YAML collections to prevent stack overflow.
/// 128 matches the JSON and TOML parser limits and stays safe within a standard
/// 1MB stack.
pub const MAX_PARSE_DEPTH: usize = 128;

/// Upper bound on nodes produced by alias expansion.
///
/// Without this, a small document using nested aliases (the "billion laughs"
/// pattern) would expand into an enormous tree. Alias expansion is capped and
/// reported as a diagnostic once the budget is exhausted.
const MAX_ALIAS_NODES: usize = 200_000;

/// Longest scalar preview rendered in the tree before truncation.
const MAX_PREVIEW_CHARS: usize = 60;

/// Diagnostic information about a YAML parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YamlDiagnostic {
    /// Human-readable explanation of the error.
    pub message: String,
    /// 1-based line number where the error occurred.
    pub line: usize,
    /// 1-based column number where the error occurred.
    pub column: usize,
    /// Zero-based byte offset in the source string.
    pub byte_offset: usize,
    /// Contextual lines of source code surrounding the error.
    pub context_snippet: String,
    /// Caret pointer alignment string (e.g. `      ^`).
    pub pointer: String,
}

impl std::fmt::Display for YamlDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Line {}, Column {}: {}\n{}\n{}",
            self.line, self.column, self.message, self.context_snippet, self.pointer
        )
    }
}

/// A precomputed physical line of the document.
#[derive(Debug, Clone, Copy)]
struct LineInfo {
    /// Byte offset of the first character on the line.
    start: usize,
    /// Number of leading whitespace bytes.
    indent: usize,
    /// Byte offset of the first non-whitespace character.
    content_start: usize,
    /// Byte offset just past the last character on the line, excluding the
    /// line terminator.
    end: usize,
    /// True for blank lines and comment-only lines, which carry no structure.
    skip: bool,
}

/// Recursive descent parser producing a [`TreeNode`] with source spans.
pub struct YamlParser<'a> {
    /// The source exactly as handed in, including any BOM. Diagnostics slice
    /// this so their offsets and context snippet address the original buffer.
    original: &'a str,
    /// Byte width of a stripped BOM, used to shift spans back onto `original`.
    bom_len: usize,
    source: &'a str,
    bytes: &'a [u8],
    lines: Vec<LineInfo>,
    /// Index of the line currently being parsed.
    li: usize,
    /// Exclusive line index bound of the document being parsed.
    doc_end: usize,
    next_node_id: usize,
    anchors: HashMap<String, TreeNode>,
    alias_nodes: usize,
}

impl<'a> YamlParser<'a> {
    /// Creates a parser for the given YAML source.
    ///
    /// Complexity: O(N) where N is the length of the source.
    pub fn new(source: &'a str) -> Result<Self, YamlDiagnostic> {
        let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);
        let bom_len = source.len() - stripped.len();
        Ok(Self {
            original: source,
            bom_len,
            source: stripped,
            bytes: stripped.as_bytes(),
            lines: build_lines(stripped),
            li: 0,
            doc_end: 0,
            next_node_id: 1,
            anchors: HashMap::new(),
            alias_nodes: 0,
        })
    }

    /// Parses the entire document into an optional root [`TreeNode`].
    ///
    /// Returns `Ok(None)` when the document is empty or comment only. A
    /// multi-document stream (`---` separated) yields a synthetic root sequence
    /// with one child per document, mirroring how JSON Lines is presented.
    ///
    /// Complexity: O(N * D) where N is the source length and D the tree depth,
    /// because node spans are resolved through a line-table lookup.
    pub fn parse(mut self) -> Result<Option<TreeNode>, YamlDiagnostic> {
        if self.source.trim().is_empty() {
            return Ok(None);
        }

        let mut ranges = self.document_ranges()?;
        ranges.retain(|(start, end)| self.range_has_content(*start, *end));
        if ranges.is_empty() {
            return Ok(None);
        }

        let multi = ranges.len() > 1;
        let mut docs: Vec<TreeNode> = Vec::with_capacity(ranges.len());

        for (index, (start, end)) in ranges.iter().copied().enumerate() {
            self.li = start;
            self.doc_end = end;
            self.anchors.clear();
            self.alias_nodes = 0;

            // Land on the first line with content so an empty leading region
            // does not produce a null root.
            while self.li < self.doc_end && self.lines[self.li].skip {
                self.li += 1;
            }
            if self.li >= self.doc_end {
                continue;
            }

            let path = if multi {
                format!("$[{}]", index)
            } else {
                "$".to_string()
            };
            let indent = self.lines[self.li].indent;
            docs.push(self.parse_block_node(indent, 0, &path)?);
        }

        if docs.is_empty() {
            return Ok(None);
        }

        if !multi {
            let mut root = docs.remove(0);
            // Widen the root to the whole document, as the TOML parser does, so
            // offsets inside leading comments or directives still resolve to the
            // root node during editor/tree sync. A single document is the file.
            let (end_line, end_col) = self.line_col_at(self.source.len());
            root.span = JsonSpan::new(
                0,
                self.source.len() + self.bom_len,
                1,
                1,
                end_line,
                end_col,
            );
            return Ok(Some(root));
        }

        let (end_line, end_col) = self.line_col_at(self.source.len());
        Ok(Some(TreeNode {
            id: 0,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(YamlType::Sequence),
            value_preview: format!("[ {} documents ]", docs.len()),
            path: "$".to_string(),
            span: JsonSpan::new(
                0,
                self.source.len() + self.bom_len,
                1,
                1,
                end_line,
                end_col,
            ),
            children: docs,
        }))
    }

    // --- Line table ---

    /// Returns true if the half-open line range `[start, end)` holds a line
    /// that carries structure.
    fn range_has_content(&self, start: usize, end: usize) -> bool {
        (start..end.min(self.lines.len()))
            .any(|i| !self.lines[i].skip)
    }

    /// Splits the source into per-document line ranges on `---` / `...` markers.
    ///
    /// `%` directives are trivia and never start a document.
    fn document_ranges(&mut self) -> Result<Vec<(usize, usize)>, YamlDiagnostic> {
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        let mut current: Option<usize> = None;

        for i in 0..self.lines.len() {
            let line = self.lines[i];
            if line.skip {
                continue;
            }
            let text = &self.source[line.content_start..line.end];

            if text.starts_with('%') {
                continue;
            }

            if let Some(rest) = text.strip_prefix("---") {
                if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                    if let Some(start) = current.take() {
                        if start < i {
                            ranges.push((start, i));
                        }
                    }
                    // `--- key: value` starts its document on the marker line,
                    // so re-point the line past the marker.
                    let trimmed_len = rest.len() - rest.trim_start().len();
                    if rest.trim().is_empty() {
                        current = Some(i + 1);
                    } else {
                        self.lines[i].content_start = line.content_start + 3 + trimmed_len;
                        self.lines[i].indent = self.lines[i].content_start - line.start;
                        current = Some(i);
                    }
                    continue;
                }
            }

            if let Some(rest) = text.strip_prefix("...") {
                if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                    if let Some(start) = current.take() {
                        if start <= i {
                            ranges.push((start, i));
                        }
                    }
                    continue;
                }
            }

            if current.is_none() {
                current = Some(i);
            }
        }

        if let Some(start) = current {
            if start < self.lines.len() {
                ranges.push((start, self.lines.len()));
            }
        }

        Ok(ranges)
    }

    /// Returns 1-based (line, column) for a byte offset, resolving the line
    /// through the precomputed table.
    fn line_col_at(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.bytes.len());
        let index = match self.lines.binary_search_by(|l| l.start.cmp(&offset)) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
        let start = self.lines[index].start;
        // Offsets always land on character boundaries because scanning
        // advances whole UTF-8 sequences.
        let column = self.source[start..offset].chars().count() + 1;
        (index + 1, column)
    }

    /// Builds a span covering the byte range `[start, end)`.
    fn span_for_range(&self, start: usize, end: usize) -> JsonSpan {
        let (start_line, start_col) = self.line_col_at(start);
        let (end_line, end_col) = self.line_col_at(end);
        // A leading BOM is stripped before parsing, but spans must index the
        // original buffer so a caller holding the original text can slice them
        // directly. Shift the byte offsets by the BOM width only: the columns
        // describe the document as parsed, which is what the editor buffer
        // holds, since `AppView::load_file` strips the BOM before loading.
        JsonSpan::new(
            start + self.bom_len,
            end + self.bom_len,
            start_line,
            start_col,
            end_line,
            end_col,
        )
    }

    fn err_at(&self, offset: usize, message: &str) -> YamlDiagnostic {
        let (line, col) = self.line_col_at(offset);
        build_yaml_diagnostic(self.original, offset + self.bom_len, line, col, message.to_string())
    }

    /// Points `li` at the line containing `offset`, so flow collections that
    /// span lines resume block parsing at the right place.
    fn sync_line(&mut self, offset: usize) {
        let offset = offset.min(self.bytes.len());
        self.li = match self.lines.binary_search_by(|l| l.start.cmp(&offset)) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        };
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn at_end(&self) -> bool {
        self.li >= self.doc_end
    }

    /// Advances the line cursor past the lines a value occupied.
    ///
    /// The value parsers track byte offsets but leave the line cursor alone, so
    /// an inline value never consumes its own key line. The mapping and
    /// sequence loops require forward progress: without this, `a: 1` re-reads
    /// line 0 on every iteration and the `children` vector grows until the
    /// allocation fails.
    ///
    /// `line_col_at` reports a 1-based line, so the 0-based index of the first
    /// line *after* the value is the line number it returns. The `max`
    /// guarantees progress even when a value ends on its own starting line,
    /// and the final clamp is to the document end rather than to the last line
    /// index: a file without a trailing newline has no sentinel line, so
    /// clamping to `len - 1` would pin the cursor on the final line and the
    /// enclosing loop would re-read it forever. Landing on `limit` is safe
    /// because `at_end` is `li >= doc_end`, which stops every loop before it
    /// indexes.
    fn advance_past_value(&mut self, end_byte: usize) {
        let after = self.line_col_at(end_byte).0;
        let limit = self.doc_end.min(self.lines.len());
        self.li = after
            .min(limit)
            .max(self.li + 1)
            .min(limit);
    }

    /// Advances `li` past blank and comment-only lines.
    fn skip_to_content(&mut self) {
        while self.li < self.doc_end && self.lines[self.li].skip {
            self.li += 1;
        }
    }

    /// Returns true when the line is a block sequence entry: a `-` followed by
    /// a space, a tab, or the end of the line. A bare `-1` is a scalar.
    fn is_seq_entry(&self, index: usize) -> bool {
        let line = self.lines[index];
        if line.content_start >= line.end || self.bytes[line.content_start] != b'-' {
            return false;
        }
        matches!(
            self.bytes.get(line.content_start + 1),
            None | Some(b' ' | b'\t' | b'\r')
        )
    }

    /// Finds the byte offset of the `:` that separates a key from its value on
    /// the given line, ignoring colons inside quotes or flow collections.
    fn find_key_colon(&self, index: usize) -> Option<usize> {
        let line = self.lines[index];
        let mut i = line.content_start;
        let mut quote: Option<u8> = None;
        let mut flow_depth = 0usize;

        while i < line.end {
            let b = self.bytes[i];
            match quote {
                Some(q) => {
                    if q == b'"' && b == b'\\' {
                        i += 2;
                        continue;
                    }
                    if b == q {
                        // `''` is an escaped quote inside a single-quoted scalar.
                        if q == b'\'' && self.bytes.get(i + 1) == Some(&b'\'') {
                            i += 2;
                            continue;
                        }
                        quote = None;
                    }
                }
                None => match b {
                    b'"' | b'\'' => quote = Some(b),
                    b'[' | b'{' => flow_depth += 1,
                    b']' | b'}' => flow_depth = flow_depth.saturating_sub(1),
                    b'#' if i > line.content_start
                        && matches!(self.bytes[i - 1], b' ' | b'\t') =>
                    {
                        return None
                    }
                    b':' if flow_depth == 0 => {
                        // A key separator is a colon at end of line, or one
                        // followed by whitespace. `i + 1 >= line.end` must be
                        // tested explicitly: the scan is bounded by `line.end`,
                        // so a colon that ends the line still sees the newline
                        // byte rather than `None`, and the key would be
                        // rejected.
                        let next = self.bytes.get(i + 1);
                        if i + 1 >= line.end
                            || next.is_none()
                            || matches!(next, Some(b' ') | Some(b'\t') | Some(b'\r'))
                        {
                            return Some(i);
                        }
                    }
                    _ => {}
                },
            }
            i += 1;
        }
        None
    }

    // --- Block structure ---

    /// Parses whichever block node begins on the current line at `indent`.
    fn parse_block_node(
        &mut self,
        indent: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        if depth > MAX_PARSE_DEPTH {
            return Err(self.err_at(
                self.lines[self.li.min(self.lines.len() - 1)].content_start,
                &format!("YAML nesting exceeds the maximum depth of {MAX_PARSE_DEPTH}"),
            ));
        }

        self.skip_to_content();
        if self.at_end() {
            let offset = self.lines[self.li.saturating_sub(1).min(self.lines.len() - 1)].end;
            return Ok(self.null_node(offset, path));
        }

        let line = self.lines[self.li];
        if line.indent < indent {
            let offset = line.content_start;
            return Ok(self.null_node(offset, path));
        }

        if self.is_seq_entry(self.li) {
            return self.parse_block_sequence(line.indent, depth, path);
        }

        if self.find_key_colon(self.li).is_some() {
            return self.parse_block_mapping(line.indent, depth, path);
        }

        // A bare scalar or flow collection occupying the whole node.
        let start = line.content_start;
        self.parse_value_after_properties(start, line.content_start, indent, depth, path, true)
    }

    /// Parses a block mapping whose keys sit at column `indent`.
    fn parse_block_mapping(
        &mut self,
        indent: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let start = self.lines[self.li].content_start;
        let mut children: Vec<TreeNode> = Vec::new();
        // Entries contributed by merge keys, applied once the mapping has been
        // read in full.
        let mut merged_entries: Vec<TreeNode> = Vec::new();
        let mut end = start;
        loop {
            self.skip_to_content();
            if self.at_end() {
                break;
            }
            let line = self.lines[self.li];
            if line.indent < indent {
                break;
            }
            // A deeper line here means the previous entry was malformed, since a
            // nested block is consumed as that entry's value.
            if line.indent > indent {
                return Err(self.err_at(
                    line.content_start,
                    "Unexpected indentation: expected a key at this level",
                ));
            }
            // A sequence at key indentation is the value of the entry that was
            // just parsed, so it terminates this mapping.
            if self.is_seq_entry(self.li) {
                break;
            }
            let Some(colon) = self.find_key_colon(self.li) else {
                break;
            };

            let (key, key_span) = self.parse_key(self.li, colon)?;
            let child_path = format!("{}.{}", path, key);
            let value = self.parse_mapping_value(colon, indent, depth, &child_path)?;
            // `end` stays in BOM-stripped coordinates; the shift is applied
            // once, in `span_for_range`, below.
            end = value.span.end_byte - self.bom_len;
            // A merge key `<<: *anchor` splices the anchored mapping's entries
            // into this one instead of appearing as a node of its own. Entries
            // are held back until the mapping is fully read, because YAML gives
            // a key written here precedence over a merged one no matter which
            // comes first in the document.
            if key == "<<" {
                let merged: Vec<TreeNode> = match &value.node_type {
                    NodeType::Yaml(YamlType::Mapping) => value.children.clone(),
                    NodeType::Yaml(YamlType::Sequence) => value
                        .children
                        .iter()
                        .filter(|c| c.node_type == NodeType::Yaml(YamlType::Mapping))
                        .flat_map(|c| c.children.clone())
                        .collect(),
                    _ => Vec::new(),
                };
                merged_entries.extend(merged);
                continue;
            }
            children.push(TreeNode {
                id: 0,
                key: Some(key),
                key_span: Some(key_span),
                node_type: value.node_type,
                value_preview: value.value_preview,
                path: child_path,
                span: value.span,
                children: value.children,
            });
        }

        for mut entry in merged_entries {
            let Some(entry_key) = entry.key.clone() else {
                continue;
            };
            if children.iter().any(|c| c.key.as_deref() == Some(entry_key.as_str())) {
                continue;
            }
            repath(&mut entry, &format!("{path}.{entry_key}"));
            children.push(entry);
        }

        let id = self.alloc_id();
        for child in &mut children {
            child.id = id;
        }
        // Re-allocate in document order so ids are unique and stable.
        let mut counter = id;
        for child in &mut children {
            assign_ids(child, &mut counter);
        }

        let span = self.span_for_range(start, end.max(start));
        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(YamlType::Mapping),
            value_preview: mapping_preview(children.len()),
            path: path.to_string(),
            span,
            children,
        })
    }

    /// Parses a block sequence whose `-` markers sit at column `indent`.
    fn parse_block_sequence(
        &mut self,
        indent: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let start = self.lines[self.li].content_start;
        let mut children: Vec<TreeNode> = Vec::new();
        let mut end = start;

        loop {
            self.skip_to_content();
            if self.at_end() {
                break;
            }
            let line = self.lines[self.li];
            if line.indent < indent {
                break;
            }
            if line.indent > indent {
                return Err(self.err_at(
                    line.content_start,
                    "Unexpected indentation in block sequence",
                ));
            }
            if !self.is_seq_entry(self.li) {
                break;
            }

            let dash = line.content_start;
            let mut item_start = dash + 1;
            while item_start < line.end
                && matches!(self.bytes[item_start], b' ' | b'\t')
            {
                item_start += 1;
            }

            let item_path = format!("{}[{}]", path, children.len());
            let item = if item_start >= line.end || self.bytes[item_start] == b'#' {
                // Value lives on the following, more indented lines.
                self.parse_block_child(parent_indent_of(dash, &self.lines[self.li], indent), depth, &item_path)?
            } else {
                // Compact notation: the entry begins mid-line, so re-point the
                // line and let the block parser treat it as a node in its own
                // right.
                let column = item_start - line.start;
                self.lines[self.li].content_start = item_start;
                self.lines[self.li].indent = column;
                self.parse_block_node(column, depth + 1, &item_path)?
            };

            end = (item.span.end_byte - self.bom_len).max(end);
            children.push(item);
        }

        let id = self.alloc_id();
        let mut counter = id;
        for child in &mut children {
            assign_ids(child, &mut counter);
        }

        let span = self.span_for_range(start, end.max(start));
        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(YamlType::Sequence),
            value_preview: sequence_preview(children.len()),
            path: path.to_string(),
            span,
            children,
        })
    }

    /// Extracts the key text and its exact span from a mapping line.
    fn parse_key(&self, index: usize, colon: usize) -> Result<(String, JsonSpan), YamlDiagnostic> {
        let line = self.lines[index];
        let start = line.content_start;
        let first = self.bytes[start];

        if first == b'"' || first == b'\'' {
            let quote = first;
            let mut i = start + 1;
            while i < colon {
                if quote == b'"' && self.bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if self.bytes[i] == quote {
                    if quote == b'\'' && self.bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            let key = decode_quoted(&self.source[start..i.min(colon)])?;
            return Ok((key, self.span_for_range(start, i.min(colon))));
        }

        let mut key_end = colon;
        while key_end > start && matches!(self.bytes[key_end - 1], b' ' | b'\t') {
            key_end -= 1;
        }
        if key_end == start {
            return Err(self.err_at(start, "Mapping key is empty"));
        }
        let key = self.source[start..key_end].to_string();
        Ok((key, self.span_for_range(start, key_end)))
    }

    /// Parses the value that follows a `key:` on the same line, either inline or
    /// as a nested block.
    fn parse_mapping_value(
        &mut self,
        colon: usize,
        key_indent: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let line_index = self.li;
        let line = self.lines[line_index];
        let mut after = colon + 1;
        while after < line.end && matches!(self.bytes[after], b' ' | b'\t') {
            after += 1;
        }

        if after >= line.end || self.bytes[after] == b'#' {
            // No inline value: the entry is null unless a nested block follows.
            return self.parse_nested_or_null(after, key_indent, depth, path, false);
        }

        self.parse_value_after_properties(after, line.content_start, key_indent, depth, path, false)
    }

    /// Parses a value that has no inline content on the key's line: either a
    /// nested block on the following lines, or a null node when the next
    /// content line belongs to an enclosing mapping.
    fn parse_nested_or_null(
        &mut self,
        offset: usize,
        parent_indent: usize,
        depth: usize,
        path: &str,
        _standalone: bool,
    ) -> Result<TreeNode, YamlDiagnostic> {
        // Step past the key line first. `skip_to_content` only skips blank
        // and comment lines, so it would otherwise leave the cursor on the
        // key itself, the nested block would never be seen, and restoring
        // `self.li` afterwards would re-read this same key forever.
        self.li = (self.li + 1).min(self.doc_end);
        self.skip_to_content();
        if self.at_end() {
            return Ok(self.null_node(offset, path));
        }
        let next = self.lines[self.li];
        // A sequence is allowed to sit at the same column as its key; a nested
        // mapping is not, since that column belongs to the parent mapping.
        let is_seq = next.indent == parent_indent && self.is_seq_entry(self.li);
        if next.indent > parent_indent || is_seq {
            return self.parse_block_node(next.indent, depth + 1, path);
        }
        // The next content line is a sibling key, so it belongs to the
        // enclosing loop: leave the cursor there rather than rewinding.
        Ok(self.null_node(offset, path))
    }

    /// Parses a block child that must be indented past `parent_indent`.
    fn parse_block_child(
        &mut self,
        parent_indent: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let save = self.li;
        self.skip_to_content();
        if self.at_end() {
            self.li = save;
            let offset = self.lines[save].end;
            return Ok(self.null_node(offset, path));
        }
        let next = self.lines[self.li];
        if next.indent <= parent_indent {
            self.li = save;
            let offset = self.lines[save].end;
            return Ok(self.null_node(offset, path));
        }
        self.parse_block_node(next.indent, depth + 1, path)
    }

    // --- Values ---

    /// Parses a value that starts at `start`, consuming any `&anchor`, `!tag`,
    /// and `*alias` node properties first.
    fn parse_value_after_properties(
        &mut self,
        start: usize,
        _node_start: usize,
        parent_indent: usize,
        depth: usize,
        path: &str,
        standalone: bool,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let mut anchor: Option<String> = None;
        let mut tag: Option<String> = None;
        let mut cursor = start;
        let line = self.lines[self.li];

        loop {
            while cursor < line.end && matches!(self.bytes[cursor], b' ' | b'\t') {
                cursor += 1;
            }
            match self.bytes.get(cursor) {
                Some(b'&') => {
                    let name_start = cursor + 1;
                    let mut i = name_start;
                    while i < line.end
                        && !matches!(self.bytes[i], b' ' | b'\t' | b'#')
                    {
                        i += 1;
                    }
                    anchor = Some(self.source[name_start..i].to_string());
                    cursor = i;
                }
                Some(b'!') => {
                    let tag_start = cursor;
                    let mut i = cursor + 1;
                    while i < line.end
                        && !matches!(self.bytes[i], b' ' | b'\t' | b'#')
                    {
                        i += 1;
                    }
                    tag = Some(self.source[tag_start..i].to_string());
                    cursor = i;
                }
                _ => break,
            }
        }

        while cursor < line.end && matches!(self.bytes[cursor], b' ' | b'\t') {
            cursor += 1;
        }

        if self.bytes.get(cursor) == Some(&b'*') {
            let name_start = cursor + 1;
            let mut i = name_start;
            while i < line.end && !matches!(self.bytes[i], b' ' | b'\t' | b',' | b'}' | b']') {
                i += 1;
            }
            let name = self.source[name_start..i].to_string();
            let node = self.resolve_alias(&name, start, path)?;
            // The line table indexes the BOM-stripped source.
            self.advance_past_value(node.span.end_byte - self.bom_len);
            return Ok(node);
        }

        let mut parsed_inline = true;
        let mut node = if cursor >= line.end || self.bytes.get(cursor) == Some(&b'#') {
            // Properties with nothing after them on the line: the value is a
            // nested block on the following lines, as in
            //   base: &defaults
            //     retries: 3
            // Without this the nested block is never consumed and the enclosing
            // mapping loop then reports the indented lines as stray.
            parsed_inline = false;
            self.parse_nested_or_null(start, parent_indent, depth, path, standalone)?
        } else {
            self.parse_value_body(cursor, parent_indent, depth, path, standalone)?
        };

        if let Some(name) = anchor {
            self.anchors.insert(name, node.clone());
        }
        if let Some(tag) = tag {
            node.value_preview = format!("{} {}", tag, node.value_preview);
        }
        if node.span.start_byte > start + self.bom_len {
            // Properties are part of the value expression, so the span starts
            // at the anchor or tag rather than at the scalar itself. Both ends
            // go back through `span_for_range` so the BOM shift stays uniform.
            node.span = self.span_for_range(start, node.span.end_byte - self.bom_len);
        }
        // A nested block already leaves the cursor on the next sibling line, so
        // advancing again here would step over it and truncate the mapping.
        if parsed_inline {
            // The line table indexes the BOM-stripped source.
            self.advance_past_value(node.span.end_byte - self.bom_len);
        }
        Ok(node)
    }

    /// Expands `*name` into a clone of the anchored node.
    fn resolve_alias(
        &mut self,
        name: &str,
        alias_start: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let Some(anchored) = self.anchors.get(name).cloned() else {
            return Err(self.err_at(alias_start, &format!("Unknown alias '*{name}'")));
        };

        self.alias_nodes += anchored.total_node_count();
        if self.alias_nodes > MAX_ALIAS_NODES {
            return Err(self.err_at(
                alias_start,
                "Alias expansion exceeds the node budget; the document may be self-referential",
            ));
        }

        let mut node = anchored;
        let mut counter = self.next_node_id;
        assign_ids(&mut node, &mut counter);
        self.next_node_id = counter;
        repath(&mut node, path);

        // The alias token itself is what the user clicked on, so it owns the
        // span; descendants keep the anchored definition's spans.
        let line = self.lines[self.li];
        let end = alias_start + 1 + name.len();
        node.span = self.span_for_range(alias_start, end.min(line.end).max(end.min(line.end)));
        Ok(node)
    }

    /// Parses the concrete value form at `cursor`.
    fn parse_value_body(
        &mut self,
        cursor: usize,
        parent_indent: usize,
        depth: usize,
        path: &str,
        standalone: bool,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let line = self.lines[self.li];
        let b = self.bytes.get(cursor).copied();

        match b {
            Some(b'{') | Some(b'[') => self.parse_flow(cursor, depth, path),
            Some(b'|') | Some(b'>') => self.parse_block_scalar(cursor, parent_indent, path),
            Some(b'"') | Some(b'\'') => {
                let (text, end) = self.scan_quoted(cursor)?;
                let id = self.alloc_id();
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Yaml(YamlType::String),
                    value_preview: preview(&text),
                    path: path.to_string(),
                    span: self.span_for_range(cursor, end),
                    children: Vec::new(),
                })
            }
            Some(b'?') if standalone => Err(self.err_at(
                cursor,
                "Explicit complex keys ('?') are not supported",
            )),
            Some(_) => {
                let (text, end) = self.scan_plain(cursor, line.end, parent_indent);
                let id = self.alloc_id();
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Yaml(infer_scalar_type(&text)),
                    value_preview: preview(&text),
                    path: path.to_string(),
                    span: self.span_for_range(cursor, end),
                    children: Vec::new(),
                })
            }
            None => {
                let id = self.alloc_id();
                Ok(self.null_node_with_id(id, cursor, path))
            }
        }
    }

    /// Parses a flow collection (`{...}` / `[...]`), which may span lines.
    fn parse_flow(
        &mut self,
        start: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        if depth > MAX_PARSE_DEPTH {
            return Err(self.err_at(
                start,
                &format!("YAML nesting exceeds the maximum depth of {MAX_PARSE_DEPTH}"),
            ));
        }

        let is_map = self.bytes[start] == b'{';
        let id = self.alloc_id();
        let mut children: Vec<TreeNode> = Vec::new();
        let mut cursor = start + 1;

        loop {
            self.flow_skip_ws(&mut cursor);
            if cursor >= self.bytes.len() {
                return Err(self.err_at(start, "Unterminated flow collection"));
            }
            match self.bytes[cursor] {
                b'}' | b']' => {
                    cursor += 1;
                    break;
                }
                b',' => {
                    cursor += 1;
                    continue;
                }
                _ => {}
            }

            if is_map {
                // A quoted key runs to its closing quote; a plain key runs to
                // the first `:`, which separates key from value in flow
                // context even with no space after it. That is what makes
                // `{k:1}` legal, while `flow_token_end`'s stricter rule keeps
                // `http://example.com` a single token in value position.
                let key_end = match self.bytes.get(cursor) {
                    Some(b'"') | Some(b'\'') => self.scan_flow_token(cursor).1,
                    _ => self.flow_key_end(cursor),
                };
                if key_end == cursor {
                    return Err(self.err_at(cursor, "Expected a key in flow mapping"));
                }
                let key = decode_flow_key(&self.source[cursor..key_end]);
                let key_span = self.span_for_range(cursor, key_end);

                // Step past the key token before looking for its separator;
                // skipping whitespace from the token's *start* leaves the
                // cursor on the key itself, so the colon is never found.
                cursor = key_end;
                self.flow_skip_ws(&mut cursor);
                if self.bytes.get(cursor) != Some(&b':') {
                    return Err(self.err_at(cursor, "Expected ':' after a flow mapping key"));
                }
                cursor += 1;
                self.flow_skip_ws(&mut cursor);

                let child_path = format!("{}.{}", path, key);
                let value = self.parse_flow_value(cursor, depth, &child_path)?;
                children.push(TreeNode {
                    id: 0,
                    key: Some(key),
                    key_span: Some(key_span),
                    node_type: value.node_type,
                    value_preview: value.value_preview,
                    path: child_path,
                    span: value.span,
                    children: value.children,
                });
                // The flow cursor indexes the BOM-stripped source.
                cursor = value.span.end_byte - self.bom_len;
            } else {
                let child_path = format!("{}[{}]", path, children.len());
                let first = self.parse_flow_value(cursor, depth, &child_path)?;
                let mut after_first = first.span.end_byte - self.bom_len;
                self.flow_skip_ws(&mut after_first);
                // `[a: 1]` is a single-pair mapping, so promote it.
                let value = if self.bytes.get(after_first) == Some(&b':') {
                    let mut cursor_after_colon = after_first + 1;
                    self.flow_skip_ws(&mut cursor_after_colon);
                    let second = self.parse_flow_value(cursor_after_colon, depth + 1, &child_path)?;
                    // The mapping spans from the key token through the value.
                    let span = self.span_for_range(
                        first.span.start_byte - self.bom_len,
                        second.span.end_byte - self.bom_len,
                    );
                    TreeNode {
                        id: 0,
                        key: first.key.clone(),
                        key_span: first.key_span,
                        node_type: NodeType::Yaml(YamlType::Mapping),
                        value_preview: mapping_preview(1),
                        path: child_path.clone(),
                        span,
                        children: vec![first, second],
                    }
                } else {
                    first
                };
                // The flow cursor indexes the BOM-stripped source.
                let end = value.span.end_byte - self.bom_len;
                children.push(value);
                cursor = end;
            }

            self.flow_skip_ws(&mut cursor);
        }

        self.sync_line(cursor);
        let mut counter = id;
        for child in &mut children {
            assign_ids(child, &mut counter);
        }
        self.next_node_id = counter;

        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(if is_map {
                YamlType::Mapping
            } else {
                YamlType::Sequence
            }),
            value_preview: if is_map {
                mapping_preview(children.len())
            } else {
                sequence_preview(children.len())
            },
            path: path.to_string(),
            span: self.span_for_range(start, cursor),
            children,
        })
    }

    /// Parses a value inside a flow collection.
    fn parse_flow_value(
        &mut self,
        cursor: usize,
        depth: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        match self.bytes.get(cursor).copied() {
            Some(b'{') | Some(b'[') => self.parse_flow(cursor, depth + 1, path),
            Some(b'"') | Some(b'\'') => {
                let (text, end) = self.scan_quoted(cursor)?;
                let id = self.alloc_id();
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Yaml(YamlType::String),
                    value_preview: preview(&text),
                    path: path.to_string(),
                    span: self.span_for_range(cursor, end),
                    children: Vec::new(),
                })
            }
            Some(b'*') => {
                let mut i = cursor + 1;
                while i < self.bytes.len()
                    && !matches!(self.bytes[i], b' ' | b'\t' | b',' | b'}' | b']' | b'\n')
                {
                    i += 1;
                }
                let name = self.source[cursor + 1..i].to_string();
                self.resolve_alias(&name, cursor, path)
            }
            Some(_) => {
                let end = self.flow_token_end(cursor);
                let text = self.source[cursor..end].to_string();
                let id = self.alloc_id();
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Yaml(infer_scalar_type(&text)),
                    value_preview: preview(&text),
                    path: path.to_string(),
                    span: self.span_for_range(cursor, end),
                    children: Vec::new(),
                })
            }
            None => Err(self.err_at(cursor, "Unterminated flow collection")),
        }
    }

    /// Returns the end offset of a plain scalar inside a flow collection.
    fn flow_token_end(&self, cursor: usize) -> usize {
        let mut i = cursor;
        while i < self.bytes.len() {
            match self.bytes[i] {
                b',' | b'}' | b']' | b'\n' | b'\r' => break,
                b':' if matches!(self.bytes.get(i + 1), None | Some(b' ') | Some(b'\t')
                    | Some(b',') | Some(b'}') | Some(b']')) =>
                {
                    break
                }
                b'#' if i > cursor && matches!(self.bytes[i - 1], b' ' | b'\t') => break,
                _ => i += 1,
            }
        }
        while i > cursor && matches!(self.bytes[i - 1], b' ' | b'\t') {
            i -= 1;
        }
        i.max(cursor)
    }

    /// Returns the end offset of a plain key inside a flow mapping.
    fn flow_key_end(&self, cursor: usize) -> usize {
        let mut i = cursor;
        while i < self.bytes.len() {
            match self.bytes[i] {
                b',' | b'}' | b']' | b'\n' | b'\r' | b':' => break,
                _ => i += 1,
            }
        }
        while i > cursor && matches!(self.bytes[i - 1], b' ' | b'\t') {
            i -= 1;
        }
        i.max(cursor)
    }

    /// Skips whitespace, newlines, and comments inside a flow collection.
    fn flow_skip_ws(&self, cursor: &mut usize) {
        while *cursor < self.bytes.len() {
            match self.bytes[*cursor] {
                b' ' | b'\t' | b'\n' | b'\r' => *cursor += 1,
                b'#' => {
                    while *cursor < self.bytes.len() && self.bytes[*cursor] != b'\n' {
                        *cursor += 1;
                    }
                }
                _ => return,
            }
        }
    }

    /// Scans a flow mapping key, returning the raw text and its end offset.
    fn scan_flow_token(&self, cursor: usize) -> (String, usize) {
        if matches!(self.bytes.get(cursor), Some(b'"') | Some(b'\'')) {
            if let Ok((_, end)) = self.scan_quoted(cursor) {
                return (self.source[cursor..end].to_string(), end);
            }
        }
        let end = self.flow_token_end(cursor);
        (self.source[cursor..end].to_string(), end)
    }

    /// Scans a single- or double-quoted scalar, returning the decoded text and
    /// the offset just past the closing quote.
    ///
    /// Complexity: O(N) where N is the number of bytes until the closing quote.
    fn scan_quoted(&self, start: usize) -> Result<(String, usize), YamlDiagnostic> {
        let quote = self.bytes[start];
        let mut i = start + 1;
        while i < self.bytes.len() {
            let b = self.bytes[i];
            if quote == b'"' && b == b'\\' {
                if self.bytes.get(i + 1) == Some(&b'\r') && self.bytes.get(i + 2) == Some(&b'\n') {
                    i += 3;
                } else {
                    i += 2;
                }
                continue;
            }
            if b == quote {
                if quote == b'\'' && self.bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                i += 1;
                return Ok((decode_quoted(&self.source[start..i])?, i));
            }
            i += 1;
        }
        Err(self.err_at(start, "Unterminated quoted scalar"))
    }

    /// Scans a plain scalar from `start` to `limit`, folding YAML continuation
    /// lines that are indented past the owning key.
    fn scan_plain(&self, start: usize, limit: usize, parent_indent: usize) -> (String, usize) {
        let mut end = start;
        while end < limit {
            if self.bytes[end] == b'#'
                && end > start
                && matches!(self.bytes[end - 1], b' ' | b'\t')
            {
                break;
            }
            end += 1;
        }
        while end > start && matches!(self.bytes[end - 1], b' ' | b'\t' | b'\r') {
            end -= 1;
        }

        let mut text = self.source[start..end].to_string();
        let mut last_end = end;

        // Fold continuation lines: deeper indentation, not a new entry.
        let mut index = self.line_index_of(start) + 1;
        while index < self.doc_end {
            let line = self.lines[index];
            if line.skip {
                index += 1;
                continue;
            }
            if line.indent <= parent_indent {
                break;
            }
            if self.is_seq_entry(index) || self.find_key_colon(index).is_some() {
                break;
            }
            let mut cont_end = line.content_start;
            let scan_end = line.end;
            while cont_end < scan_end {
                if self.bytes[cont_end] == b'#'
                    && cont_end > line.content_start
                    && matches!(self.bytes[cont_end - 1], b' ' | b'\t')
                {
                    break;
                }
                cont_end += 1;
            }
            while cont_end > line.content_start
                && matches!(self.bytes[cont_end - 1], b' ' | b'\t' | b'\r')
            {
                cont_end -= 1;
            }
            if cont_end > line.content_start {
                text.push(' ');
                text.push_str(&self.source[line.content_start..cont_end]);
                last_end = cont_end;
            }
            index += 1;
        }

        (text, last_end)
    }

    /// Returns the index of the line containing `offset`.
    fn line_index_of(&self, offset: usize) -> usize {
        let offset = offset.min(self.bytes.len());
        match self.lines.binary_search_by(|l| l.start.cmp(&offset)) {
            Ok(i) => i,
            Err(0) => 0,
            Err(i) => i - 1,
        }
    }

    /// Parses a block scalar (`|` literal or `>` folded), honouring the
    /// explicit indent indicator and the `+`/`-` chomping indicators.
    fn parse_block_scalar(
        &mut self,
        start: usize,
        parent_indent: usize,
        path: &str,
    ) -> Result<TreeNode, YamlDiagnostic> {
        let line_index = self.li;
        let line = self.lines[line_index];
        let style = self.bytes[start];

        // Header: style, then an optional indent digit and chomping flags in
        // either order, then only whitespace and a comment.
        let mut i = start + 1;
        let mut chomp = Chomp::Clip;
        let mut explicit_indent: Option<usize> = None;
        while i < line.end {
            match self.bytes[i] {
                b'+' => {
                    chomp = Chomp::Keep;
                    i += 1;
                }
                b'-' => {
                    chomp = Chomp::Strip;
                    i += 1;
                }
                b'0'..=b'9' => {
                    explicit_indent = Some((self.bytes[i] - b'0') as usize);
                    i += 1;
                }
                b' ' | b'\t' => break,
                b'#' => break,
                _ => {
                    return Err(self.err_at(i, "Invalid block scalar header"));
                }
            }
        }
        // Only whitespace and a comment may follow the header, and reaching
        // end-of-line is valid: a bare `|` is the common form.
        if i < line.end && !matches!(self.bytes.get(i), Some(b' ') | Some(b'\t') | Some(b'#')) {
            return Err(self.err_at(i, "Invalid block scalar header"));
        }

        // Collect body lines: every line indented past the parent, plus blank
        // lines that are followed by more body content.
        let mut body: Vec<(usize, usize)> = Vec::new();
        let mut index = line_index + 1;
        let mut content_indent = explicit_indent.map(|n| parent_indent + n);

        while index < self.doc_end {
            let current = self.lines[index];
            // `build_lines` emits a zero-width sentinel line at the position just
            // past a trailing newline. It is not a body line, and counting it
            // would append a spurious newline under `+` chomping.
            if current.start >= self.source.len() {
                break;
            }
            // A line indented no further than the parent ends the body. Blank
            // lines still belong to it, since trailing newlines are content
            // under `+` chomping. A comment is different: `build_lines` marks
            // comments `skip` too, but a comment at this indentation is
            // document trivia rather than scalar text, so it terminates the
            // body. Comments genuinely inside the body are indented past the
            // parent and still reach the value below.
            let is_comment =
                current.content_start < current.end && self.bytes[current.content_start] == b'#';
            if current.indent <= parent_indent && (!current.skip || is_comment) {
                break;
            }
            body.push((current.content_start, current.end));
            if content_indent.is_none() && !current.skip {
                content_indent = Some(current.indent);
            }
            index += 1;
        }

        let indent = content_indent.unwrap_or(parent_indent + 1);

        // Trailing blank lines only belong to the scalar under `+` chomping.
        let mut last_content = body.len();
        while last_content > 0 {
            let (from, to) = body[last_content - 1];
            if self.source[from..to].trim().is_empty() {
                last_content -= 1;
            } else {
                break;
            }
        }
        let included = if chomp == Chomp::Keep {
            body.len()
        } else {
            last_content
        };

        let mut value = String::new();
        let mut previous_blank = false;
        for (from, to) in body.iter().take(included) {
            let raw_end = (*to).min(self.bytes.len());
            let slice = &self.source[(*from).min(raw_end)..raw_end];
            let is_blank = slice.trim().is_empty();
            let line_text = if is_blank {
                String::new()
            } else {
                let lead = indent.min(slice.len().saturating_sub(slice.trim_start().len()));
                slice[lead..].trim_end().to_string()
            };

            if value.is_empty() && is_blank {
                continue;
            }
            if is_blank {
                value.push('\n');
                previous_blank = true;
                continue;
            }
            if !value.is_empty() && !previous_blank {
                if style == b'>' {
                    value.push(' ');
                } else {
                    value.push('\n');
                }
            } else if !value.is_empty() && previous_blank {
                value.push_str(&line_text);
                previous_blank = false;
                continue;
            }
            value.push_str(&line_text);
        }

        match chomp {
            Chomp::Strip => {}
            Chomp::Clip => {
                if !value.is_empty() {
                    value.push('\n');
                }
            }
            Chomp::Keep => {
                // The loop above already emitted one '\n' per body line break,
                // including the trailing blank lines that `included` preserved.
                // Keep only needs the final line's own terminator.
                if !value.is_empty() {
                    value.push('\n');
                }
            }
        }

        let end = if included == 0 {
            self.lines[line_index].end
        } else {
            body[included - 1].1
        };

        // The line cursor is deliberately left alone here. `end` is the end of
        // the last body line, and the caller's `advance_past_value` derives the
        // first line after the scalar from it. Pre-advancing as well would step
        // one line too far and swallow the key that follows the block scalar.
        let id = self.alloc_id();
        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(YamlType::String),
            value_preview: preview(&value),
            path: path.to_string(),
            span: self.span_for_range(start, end.max(start)),
            children: Vec::new(),
        })
    }

    fn null_node(&self, offset: usize, path: &str) -> TreeNode {
        let id = 0;
        self.null_node_with_id(id, offset, path)
    }

    fn null_node_with_id(&self, id: usize, offset: usize, path: &str) -> TreeNode {
        TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Yaml(YamlType::Null),
            value_preview: "null".to_string(),
            path: path.to_string(),
            span: self.span_for_range(offset, offset),
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chomp {
    /// Single trailing newline (the default).
    Clip,
    /// No trailing newline.
    Strip,
    /// All trailing newlines.
    Keep,
}

/// Recursively assigns fresh ids to a subtree in document order.
fn assign_ids(node: &mut TreeNode, counter: &mut usize) {
    node.id = *counter;
    *counter += 1;
    for child in &mut node.children {
        assign_ids(child, counter);
    }
}

/// Rewrites every path in a subtree so an expanded alias reports the path of
/// the alias rather than the anchor it was defined at.
fn repath(node: &mut TreeNode, path: &str) {
    node.path = path.to_string();
    for (index, child) in node.children.iter_mut().enumerate() {
        let child_path = match (&child.key, node.node_type) {
            (Some(key), NodeType::Yaml(YamlType::Mapping)) => format!("{}.{}", path, key),
            _ => format!("{}[{}]", path, index),
        };
        repath(child, &child_path);
    }
}

/// Returns the indentation a block child of a sequence entry must exceed.
fn parent_indent_of(_dash: usize, line: &LineInfo, sequence_indent: usize) -> usize {
    // A nested block under `-` must be indented past the dash column, so the
    // dash column itself is the boundary.
    line.indent.max(sequence_indent)
}

fn mapping_preview(len: usize) -> String {
    if len == 0 {
        "{ 0 items }".to_string()
    } else {
        format!("{{ {} items }}", len)
    }
}

fn sequence_preview(len: usize) -> String {
    if len == 0 {
        "[ 0 items ]".to_string()
    } else {
        format!("[ {} items ]", len)
    }
}

/// Renders a scalar for the tree preview, escaping newlines so a multi-line
/// block scalar stays on one row.
fn preview(text: &str) -> String {
    let flat = text.replace('\n', "\\n");
    if flat.chars().count() > MAX_PREVIEW_CHARS {
        let truncated: String = flat.chars().take(MAX_PREVIEW_CHARS).collect();
        format!("{truncated}...")
    } else {
        flat
    }
}

/// Decodes a single-quoted scalar, folding multiline line breaks and unescaping doubled quotes.
///
/// Complexity: O(N) where N is the length of `inner`.
fn decode_single_quoted(inner: &str) -> String {
    let unescaped = inner.replace("''", "'");
    if !unescaped.contains('\n') && !unescaped.contains('\r') {
        return unescaped;
    }
    let mut out = String::with_capacity(unescaped.len());
    let mut chars = unescaped.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' || ch == '\n' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            let mut newline_count = 1;
            loop {
                while let Some(&ws) = chars.peek() {
                    if ws == ' ' || ws == '\t' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'\r') {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    newline_count += 1;
                } else if chars.peek() == Some(&'\n') {
                    chars.next();
                    newline_count += 1;
                } else {
                    break;
                }
            }
            if newline_count == 1 {
                out.push(' ');
            } else {
                for _ in 0..(newline_count - 1) {
                    out.push('\n');
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Decodes a double-quoted scalar, supporting escape codes, escaped line breaks, and multiline folding.
///
/// Complexity: O(N) where N is the length of `inner`.
fn decode_double_quoted(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.peek().copied() {
                Some('\r') => {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    // Escaped line break: strip leading whitespace on next line
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch == ' ' || next_ch == '\t' {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                Some('\n') => {
                    chars.next();
                    // Escaped line break: strip leading whitespace on next line
                    while let Some(&next_ch) = chars.peek() {
                        if next_ch == ' ' || next_ch == '\t' {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                Some('n') => { chars.next(); out.push('\n'); }
                Some('t') => { chars.next(); out.push('\t'); }
                Some('r') => { chars.next(); out.push('\r'); }
                Some('0') => { chars.next(); out.push('\0'); }
                Some('a') => { chars.next(); out.push('\u{7}'); }
                Some('b') => { chars.next(); out.push('\u{8}'); }
                Some('f') => { chars.next(); out.push('\u{c}'); }
                Some('v') => { chars.next(); out.push('\u{b}'); }
                Some('e') => { chars.next(); out.push('\u{1b}'); }
                Some('\\') => { chars.next(); out.push('\\'); }
                Some('"') => { chars.next(); out.push('"'); }
                Some('/') => { chars.next(); out.push('/'); }
                Some(' ') => { chars.next(); out.push(' '); }
                Some('_') => { chars.next(); out.push('\u{a0}'); }
                Some('N') => { chars.next(); out.push('\u{85}'); }
                Some('L') => { chars.next(); out.push('\u{2028}'); }
                Some('P') => { chars.next(); out.push('\u{2029}'); }
                Some('x') => {
                    chars.next();
                    let mut hex = String::new();
                    for _ in 0..2 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                chars.next();
                            }
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(val) {
                            out.push(c);
                        }
                    }
                }
                Some('u') => {
                    chars.next();
                    let mut hex = String::new();
                    for _ in 0..4 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                chars.next();
                            }
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(val) {
                            out.push(c);
                        }
                    }
                }
                Some('U') => {
                    chars.next();
                    let mut hex = String::new();
                    for _ in 0..8 {
                        if let Some(&h) = chars.peek() {
                            if h.is_ascii_hexdigit() {
                                hex.push(h);
                                chars.next();
                            }
                        }
                    }
                    if let Ok(val) = u32::from_str_radix(&hex, 16) {
                        if let Some(c) = char::from_u32(val) {
                            out.push(c);
                        }
                    }
                }
                Some(other) => {
                    chars.next();
                    out.push(other);
                }
                None => {
                    out.push('\\');
                }
            }
        } else if ch == '\r' || ch == '\n' {
            if ch == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            let mut newline_count = 1;
            loop {
                while let Some(&ws) = chars.peek() {
                    if ws == ' ' || ws == '\t' {
                        chars.next();
                    } else {
                        break;
                    }
                }
                if chars.peek() == Some(&'\r') {
                    chars.next();
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    newline_count += 1;
                } else if chars.peek() == Some(&'\n') {
                    chars.next();
                    newline_count += 1;
                } else {
                    break;
                }
            }
            if newline_count == 1 {
                out.push(' ');
            } else {
                for _ in 0..(newline_count - 1) {
                    out.push('\n');
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

/// Decodes a quoted scalar, honouring single/double-quoted escape sequences and line folding.
fn decode_quoted(raw: &str) -> Result<String, YamlDiagnostic> {
    let bytes = raw.as_bytes();
    if bytes.len() < 2 {
        return Ok(raw.to_string());
    }
    let quote = bytes[0];
    if bytes[0] != quote || *bytes.last().unwrap() != quote {
        return Ok(raw.to_string());
    }
    let inner = &raw[1..raw.len() - 1];

    if quote == b'\'' {
        Ok(decode_single_quoted(inner))
    } else {
        Ok(decode_double_quoted(inner))
    }
}

/// Decodes a flow mapping key, which may or may not be quoted.
fn decode_flow_key(raw: &str) -> String {
    if raw.starts_with('"') || raw.starts_with('\'') {
        decode_quoted(raw).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    }
}

/// Infers a YAML scalar type from its text using the core schema.
fn infer_scalar_type(text: &str) -> YamlType {
    let t = text.trim();
    if t.is_empty() || t == "~" || matches!(t, "null" | "Null" | "NULL") {
        return YamlType::Null;
    }
    if matches!(
        t,
        "true" | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
            | "yes"
            | "Yes"
            | "YES"
            | "no"
            | "No"
            | "NO"
            | "on"
            | "On"
            | "ON"
            | "off"
            | "Off"
            | "OFF"
    ) {
        return YamlType::Boolean;
    }

    let (sign, digits) = match t.strip_prefix('-') {
        Some(rest) => (-1i64, rest),
        None => (1i64, t.strip_prefix('+').unwrap_or(t)),
    };
    let _ = sign;

    let cleaned: String = digits.chars().filter(|c| *c != '_').collect();
    if cleaned.is_empty() {
        return YamlType::String;
    }

    let radix_parsed = if let Some(hex) = cleaned.strip_prefix("0x").or(cleaned.strip_prefix("0X"))
    {
        i64::from_str_radix(hex, 16).ok()
    } else if let Some(oct) = cleaned.strip_prefix("0o").or(cleaned.strip_prefix("0O")) {
        i64::from_str_radix(oct, 8).ok()
    } else if let Some(bin) = cleaned.strip_prefix("0b").or(cleaned.strip_prefix("0B")) {
        i64::from_str_radix(bin, 2).ok()
    } else {
        None
    };
    if radix_parsed.is_some() {
        return YamlType::Integer;
    }

    if cleaned.chars().all(|c| c.is_ascii_digit()) {
        return YamlType::Integer;
    }

    let lower = cleaned.to_ascii_lowercase();
    if matches!(lower.as_str(), ".inf" | "-.inf" | "+.inf" | ".nan") {
        return YamlType::Float;
    }
    if cleaned.parse::<f64>().is_ok() && cleaned.chars().any(|c| c == '.' || c == 'e' || c == 'E')
    {
        return YamlType::Float;
    }

    YamlType::String
}

/// Builds a `YamlDiagnostic` with surrounding source context and a caret pointer.
pub fn build_yaml_diagnostic(
    source: &str,
    byte_offset: usize,
    line: usize,
    col: usize,
    message: String,
) -> YamlDiagnostic {
    let zero_line = line.saturating_sub(1);
    let start_idx = zero_line.saturating_sub(1);
    let end_idx = zero_line + 2;

    let mut snippet = String::new();
    for (idx, line_content) in source.lines().enumerate() {
        if idx >= end_idx {
            break;
        }
        if idx >= start_idx {
            let line_num = idx + 1;
            let prefix = if line_num == line { ">" } else { " " };
            snippet.push_str(&format!("{prefix} {line_num:4} | {line_content}\n"));
        }
    }

    let spaces = " ".repeat(col.saturating_sub(1) + 9);
    let pointer = format!("{spaces}^");

    YamlDiagnostic {
        message,
        line,
        column: col,
        byte_offset,
        context_snippet: snippet.trim_end().to_string(),
        pointer,
    }
}

/// Splits the source into a line table used for indentation and span lookups.
fn build_lines(source: &str) -> Vec<LineInfo> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;

    loop {
        let mut raw_end = start;
        while raw_end < bytes.len() && bytes[raw_end] != b'\n' {
            raw_end += 1;
        }
        let mut end = raw_end;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }

        let mut content_start = start;
        while content_start < end && matches!(bytes[content_start], b' ' | b'\t') {
            content_start += 1;
        }
        let skip = content_start >= end || bytes[content_start] == b'#';

        lines.push(LineInfo {
            start,
            indent: content_start - start,
            content_start,
            end,
            skip,
        });

        if raw_end >= bytes.len() {
            break;
        }
        start = raw_end + 1;
    }

    lines
}
