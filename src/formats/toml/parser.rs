//! TOML parser with accurate source span tracking and contextual diagnostics.
//!
//! Implements the TOML v1.0.0 grammar while preserving exact start and end byte
//! offsets, lines, and columns for every key, value, table header, and array.
//! Spans are computed during a single forward scan, which is what enables
//! two-way synchronization between the editor and the tree view.

use super::model::TomlType;
use crate::formats::json::JsonSpan;
use crate::formats::node::{NodeType, TreeNode};

/// Diagnostic information about a TOML parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TomlDiagnostic {
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

impl std::fmt::Display for TomlDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Line {}, Column {}: {}\n{}\n{}",
            self.line, self.column, self.message, self.context_snippet, self.pointer
        )
    }
}

/// Maximum allowed nesting depth for TOML tables/arrays to prevent stack overflow.
/// 128 matches the JSON parser limit and stays safe within a standard 1MB stack.
pub const MAX_PARSE_DEPTH: usize = 128;

/// Recursive descent parser producing a [`TreeNode`] with source spans.
pub struct TomlParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
    line: usize,
    col: usize,
    next_node_id: usize,
    /// Current table path (as raw key segments) that bare keys are assigned into.
    current_path: Vec<String>,
}

impl<'a> TomlParser<'a> {
    /// Creates a parser for the given TOML source.
    ///
    /// Complexity: O(1).
    pub fn new(source: &'a str) -> Result<Self, TomlDiagnostic> {
        let source = source.strip_prefix('\u{FEFF}').unwrap_or(source);
        Ok(Self {
            source,
            bytes: source.as_bytes(),
            cursor: 0,
            line: 1,
            col: 1,
            next_node_id: 1,
            current_path: Vec::new(),
        })
    }

    /// Parses the entire document into an optional root [`TreeNode`].
    ///
    /// Returns `Ok(None)` when the document is empty or whitespace/comment only.
    ///
    /// Complexity: O(N) where N is the length of the source document.
    pub fn parse(mut self) -> Result<Option<TreeNode>, TomlDiagnostic> {
        if self.source.trim().is_empty() {
            return Ok(None);
        }

        let root_id = self.alloc_id();
        let mut root = TreeNode {
            id: root_id,
            key: None,
            key_span: None,
            node_type: NodeType::Toml(TomlType::Table),
            value_preview: String::new(),
            path: "$".to_string(),
            span: JsonSpan::new(0, self.bytes.len(), 1, 1, self.line, self.col),
            children: Vec::new(),
        };

        loop {
            self.skip_trivia();
            if self.cursor >= self.bytes.len() {
                break;
            }

            if self.peek() == Some(b'[') {
                self.parse_table_header(&mut root)?;
            } else {
                self.parse_key_value(&mut root)?;
            }
        }

        // A document with no statements (only comments/blank lines) has no tree.
        if root.children.is_empty() && self.current_path.is_empty() {
            return Ok(None);
        }

        root.span = JsonSpan::new(0, self.bytes.len(), 1, 1, self.line, self.col);
        Ok(Some(root))
    }

    // --- Cursor helpers ---

    fn alloc_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.cursor + ahead).copied()
    }

    /// Advances one byte, maintaining line/column counters.
    fn bump(&mut self) {
        if let Some(b) = self.peek() {
            self.cursor += 1;
            if b == b'\n' {
                self.line += 1;
                self.col = 1;
            } else if b & 0xC0 != 0x80 {
                // Count UTF-8 characters, not continuation bytes.
                self.col += 1;
            }
        }
    }

    fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            self.bump();
        }
    }

    /// Builds a span from a byte range by scanning the source for line breaks.
    fn span_for_range(&self, start: usize, end: usize) -> JsonSpan {
        let (start_line, start_col) = line_col_at(self.source, start);
        let (end_line, end_col) = line_col_at(self.source, end);
        JsonSpan::new(start, end, start_line, start_col, end_line, end_col)
    }

    fn err_at(&self, offset: usize, message: String) -> TomlDiagnostic {
        let (line, col) = line_col_at(self.source, offset);
        build_toml_diagnostic(self.source, offset, line, col, message)
    }

    /// Skips whitespace, comments, and newlines between statements.
    fn skip_trivia(&mut self) {
        while let Some(b) = self.peek() {
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => self.bump(),
                b'#' => {
                    while let Some(c) = self.peek() {
                        if c == b'\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    /// Skips spaces and tabs only (used inside a statement).
    fn skip_inline_ws(&mut self) {
        while matches!(self.peek(), Some(b' ') | Some(b'\t')) {
            self.bump();
        }
    }

    // --- Table headers ---

    /// Parses `[table.path]` and `[[array.of.tables]]` headers.
    fn parse_table_header(&mut self, root: &mut TreeNode) -> Result<(), TomlDiagnostic> {
        let header_start = self.cursor;
        self.bump(); // consume '['
        let is_array = self.peek() == Some(b'[');
        if is_array {
            self.bump();
        }

        self.skip_inline_ws();
        let (segments, _key_start) = self.parse_key_path()?;
        self.skip_inline_ws();

        if self.peek() != Some(b']') {
            return Err(self.err_at(self.cursor, "expected `]` to close table header".into()));
        }
        self.bump();
        if is_array {
            if self.peek() != Some(b']') {
                return Err(self
                    .err_at(self.cursor, "expected `]]` to close array-of-tables header".into()));
            }
            self.bump();
        }
        let header_end = self.cursor;

        // Only whitespace and an optional comment may follow a header.
        self.skip_inline_ws();
        if let Some(b'#') = self.peek() {
            while let Some(c) = self.peek() {
                if c == b'\n' {
                    break;
                }
                self.bump();
            }
        } else if self.peek().is_some() && self.peek() != Some(b'\n') {
            return Err(self.err_at(
                self.cursor,
                "unexpected content after table header".into(),
            ));
        }

        if segments.is_empty() {
            return Err(self.err_at(header_start, "table header has no key".into()));
        }

        self.current_path = segments.clone();

        if is_array {
            // Descend to the parent, then push a new table into the target array.
            let (parent_path, last) = segments.split_at(segments.len() - 1);
            let parent = self.resolve_table(root, parent_path, header_start)?;
            let key = &last[0];
            self.push_array_of_tables(parent, key, header_start, header_end)?;
        } else {
            self.resolve_table(root, &segments, header_start)?;
        }

        Ok(())
    }

    /// Ensures a table exists at `segments` and returns a mutable handle to it.
    fn resolve_table<'t>(
        &mut self,
        root: &'t mut TreeNode,
        segments: &[String],
        key_start: usize,
    ) -> Result<&'t mut TreeNode, TomlDiagnostic> {
        let mut node = root;
        for (i, seg) in segments.iter().enumerate() {
            let is_last = i + 1 == segments.len();
            let child_path = append_segment(&node.path, seg);

            let existing = node
                .children
                .iter()
                .position(|c| c.key.as_deref() == Some(seg.as_str()));

            match existing {
                Some(pos)
                    if matches!(
                        node.children[pos].node_type,
                        NodeType::Toml(TomlType::Table | TomlType::InlineTable)
                    ) =>
                {
                    node = &mut node.children[pos];
                }
                Some(pos)
                    if is_last && matches!(node.children[pos].node_type, NodeType::Toml(TomlType::Array)) =>
                {
                    // `[[a]]` followed by `[a.b]` targets the most recent element.
                    let last_idx = node.children[pos].children.len().saturating_sub(1);
                    if last_idx == 0 && node.children[pos].children.is_empty() {
                        return Err(self.err_at(
                            key_start,
                            format!("table `{}` was not defined as an array of tables", seg),
                        ));
                    }
                    node = &mut node.children[pos].children[last_idx];
                }
                Some(_) => {
                    return Err(self.err_at(
                        key_start,
                        format!("cannot redefine `{}` as a table", seg),
                    ));
                }
                None => {
                    let id = self.alloc_id();
                    node.children.push(TreeNode {
                        id,
                        key: Some(seg.clone()),
                        key_span: None,
                        node_type: NodeType::Toml(TomlType::Table),
                        value_preview: String::new(),
                        path: child_path,
                        span: JsonSpan::new(key_start, key_start, 1, 1, 1, 1),
                        children: Vec::new(),
                    });
                    let pos = node.children.len() - 1;
                    node = &mut node.children[pos];
                }
            }
            let _ = is_last;
        }
        Ok(node)
    }

    /// Appends a new table entry to the array-of-tables at `key`.
    fn push_array_of_tables<'t>(
        &mut self,
        parent: &'t mut TreeNode,
        key: &str,
        key_start: usize,
        header_end: usize,
    ) -> Result<&'t mut TreeNode, TomlDiagnostic> {
        let array_path = append_segment(&parent.path, key);

        let pos = parent
            .children
            .iter()
            .position(|c| c.key.as_deref() == Some(key));

        let array_pos = match pos {
            Some(p) => {
                if !matches!(parent.children[p].node_type, NodeType::Toml(TomlType::Array)) {
                    return Err(self.err_at(
                        key_start,
                        format!("cannot redefine `{key}` as an array of tables"),
                    ));
                }
                p
            }
            None => {
                let id = self.alloc_id();
                parent.children.push(TreeNode {
                    id,
                    key: Some(key.to_string()),
                    key_span: None,
                    node_type: NodeType::Toml(TomlType::Array),
                    value_preview: String::new(),
                    path: array_path.clone(),
                    span: JsonSpan::new(key_start, key_start, 1, 1, 1, 1),
                    children: Vec::new(),
                });
                parent.children.len() - 1
            }
        };

        let elem_index = parent.children[array_pos].children.len();
        let id = self.alloc_id();
        let elem_path = format!("{array_path}[{elem_index}]");
        parent.children[array_pos].children.push(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Toml(TomlType::Table),
            value_preview: String::new(),
            path: elem_path,
            span: JsonSpan::new(key_start, header_end, 1, 1, 1, 1),
            children: Vec::new(),
        });

        let pos = parent.children[array_pos].children.len() - 1;
        Ok(&mut parent.children[array_pos].children[pos])
    }

    // --- Key/value pairs ---

    /// Parses a `key.path = value` statement into the current table.
    fn parse_key_value(&mut self, root: &mut TreeNode) -> Result<(), TomlDiagnostic> {
        let stmt_start = self.cursor;
        let (segments, key_start) = self.parse_key_path()?;
        if segments.is_empty() {
            return Err(self.err_at(stmt_start, "expected a key".into()));
        }

        self.skip_inline_ws();
        if self.peek() != Some(b'=') {
            return Err(self.err_at(self.cursor, "expected `=` after key".into()));
        }
        self.skip_inline_ws();
        if self.peek() != Some(b'=') {
            return Err(self.err_at(self.cursor, "expected `=` after key".into()));
        }
        self.bump();
        self.skip_inline_ws();

        // Resolve (and create) the dotted key path within the current table.
        let current = self.current_path.clone();
        let table = self.resolve_table(root, &current, stmt_start)?;

        let key = segments[segments.len() - 1].clone();
        let key_span = self.span_for_range(key_start, self.cursor);
        let parent_path: Vec<String> = segments[..segments.len() - 1].to_vec();

        let table = if parent_path.is_empty() {
            table
        } else {
            self.resolve_table(table, &parent_path, key_start)?
        };

        // Duplicate keys within the same table are a hard error in TOML.
        if table.children.iter().any(|c| c.key.as_deref() == Some(key.as_str())) {
            return Err(self.err_at(key_start, "duplicate key in table".into()));
        }

        let value_path = append_segment(&table.path, &key);
        let mut value = self.parse_value_at(&value_path, 0)?;

        // Attach the key and its span now that both offsets are known.
        value.key = Some(key);
        value.key_span = Some(key_span);

        table.children.push(value);
        Ok(())
    }

    // --- Values ---

    /// Parses a single value at the current cursor, placing it at `path`.
    fn parse_value_at(&mut self, path: &str, depth: usize) -> Result<TreeNode, TomlDiagnostic> {
        if depth > MAX_PARSE_DEPTH {
            return Err(self.err_at(
                self.cursor,
                format!("maximum TOML nesting depth of {MAX_PARSE_DEPTH} exceeded"),
            ));
        }

        let start = self.cursor;
        let id = self.alloc_id();

        let Some(b) = self.peek() else {
            return Err(self.err_at(start, "expected a value".into()));
        };

        match b {
            b'"' | b'\'' => {
                let (toml_type, end) = self.scan_string(start)?;
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Toml(toml_type),
                    value_preview: preview_from_source(&self.source[start..end]),
                    path: path.to_string(),
                    span: self.span_for_range(start, end),
                    children: Vec::new(),
                })
            }
            b'[' => self.parse_array(start, path, id, depth),
            b'{' => self.parse_inline_table(start, path, id, depth),
            b't' | b'f' => {
                let end = self.scan_bare_token(start);
                let text = &self.source[start..end];
                let toml_type = if text == "true" || text == "false" {
                    TomlType::Boolean
                } else {
                    return Err(self.err_at(start, "invalid value".into()));
                };
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Toml(toml_type),
                    value_preview: text.to_string(),
                    path: path.to_string(),
                    span: self.span_for_range(start, end),
                    children: Vec::new(),
                })
            }
            _ => {
                let end = self.scan_bare_token(start);
                if end == start {
                    return Err(self.err_at(start, "expected a value".into()));
                }
                let text = &self.source[start..end];
                let toml_type = classify_bare_value(text)
                    .ok_or_else(|| self.err_at(start, format!("invalid value `{text}`")))?;
                Ok(TreeNode {
                    id,
                    key: None,
                    key_span: None,
                    node_type: NodeType::Toml(toml_type),
                    value_preview: text.to_string(),
                    path: path.to_string(),
                    span: self.span_for_range(start, end),
                    children: Vec::new(),
                })
            }
        }
    }

    /// Parses a `[ ... ]` array value, including nested arrays and inline tables.
    fn parse_array(
        &mut self,
        start: usize,
        path: &str,
        id: usize,
        depth: usize,
    ) -> Result<TreeNode, TomlDiagnostic> {
        self.bump(); // consume '['
        let mut children = Vec::new();

        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Err(self.err_at(start, "unterminated array".into())),
                Some(b']') => {
                    self.bump();
                    break;
                }
                _ => {}
            }

            let idx = children.len();
            let elem = self.parse_value_at(&format!("{path}[{idx}]"), depth + 1)?;
            children.push(elem);

            self.skip_trivia();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b']') => {
                    self.bump();
                    break;
                }
                _ => return Err(self.err_at(self.cursor, "expected `,` or `]` in array".into())),
            }
        }

        let end = self.cursor;
        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Toml(TomlType::Array),
            value_preview: format!("[ {} items ]", children.len()),
            path: path.to_string(),
            span: self.span_for_range(start, end),
            children,
        })
    }

    /// Parses a `{ key = value }` inline table.
    fn parse_inline_table(
        &mut self,
        start: usize,
        path: &str,
        id: usize,
        depth: usize,
    ) -> Result<TreeNode, TomlDiagnostic> {
        self.bump(); // consume '{'
        let mut children = Vec::new();

        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Err(self.err_at(start, "unterminated inline table".into())),
                Some(b'}') => {
                    self.bump();
                    break;
                }
                _ => {}
            }

            let key_start = self.cursor;
            let (segments, _seg_start) = self.parse_key_path()?;
            if segments.is_empty() {
                return Err(self.err_at(key_start, "expected a key in inline table".into()));
            }
            self.skip_inline_ws();
            if self.peek() != Some(b'=') {
                return Err(self.err_at(self.cursor, "expected `=` in inline table".into()));
            }
            self.bump();
            self.skip_trivia();

            let key = segments[segments.len() - 1].clone();
            let key_span = self.span_for_range(key_start, self.cursor);
            let value_path = append_segment(path, &key);
            let mut value = self.parse_value_at(&value_path, depth + 1)?;
            value.key = Some(key);
            value.key_span = Some(key_span);
            children.push(value);

            self.skip_trivia();
            match self.peek() {
                Some(b',') => {
                    self.bump();
                }
                Some(b'}') => {
                    self.bump();
                    break;
                }
                _ => {
                    return Err(self
                        .err_at(self.cursor, "expected `,` or `}` in inline table".into()))
                }
            }
        }

        let end = self.cursor;
        Ok(TreeNode {
            id,
            key: None,
            key_span: None,
            node_type: NodeType::Toml(TomlType::InlineTable),
            value_preview: format!("{{ {} items }}", children.len()),
            path: path.to_string(),
            span: self.span_for_range(start, end),
            children,
        })
    }

    // --- Lexing helpers ---

    /// Parses a possibly dotted, possibly quoted key into its segments.
    fn parse_key_path(&mut self) -> Result<(Vec<String>, usize), TomlDiagnostic> {
        let mut segments = Vec::new();
        let mut first_start = self.cursor;

        loop {
            self.skip_inline_ws();
            let seg_start = self.cursor;
            if segments.is_empty() {
                first_start = seg_start;
            }

            let seg = match self.peek() {
                Some(b'"') | Some(b'\'') => {
                    let quote = self.peek().unwrap();
                    self.bump();
                    let mut value = String::new();
                    loop {
                        match self.peek() {
                            None | Some(b'\n') => {
                                return Err(self.err_at(seg_start, "unterminated quoted key".into()))
                            }
                            Some(c) if c == quote => {
                                self.bump();
                                break;
                            }
                            Some(_) => {
                                let ch = self.current_char();
                                value.push(ch);
                                self.bump();
                            }
                        }
                    }
                    value
                }
                Some(b) if is_bare_key_byte(b) => {
                    while let Some(c) = self.peek() {
                        if is_bare_key_byte(c) {
                            self.bump();
                        } else {
                            break;
                        }
                    }
                    self.source[seg_start..self.cursor].to_string()
                }
                _ => {
                    return Err(self.err_at(self.cursor, "expected a key".into()));
                }
            };

            if seg.is_empty() {
                return Err(self.err_at(seg_start, "empty key segment".into()));
            }
            segments.push(seg);

            self.skip_inline_ws();
            if self.peek() == Some(b'.') {
                self.bump();
                continue;
            }
            break;
        }

        Ok((segments, first_start))
    }

    /// Returns the character at the cursor without consuming it.
    fn current_char(&self) -> char {
        self.source[self.cursor..].chars().next().unwrap_or('\0')
    }

    /// Scans a quoted string value, returning its type and end offset.
    fn scan_string(&mut self, start: usize) -> Result<(TomlType, usize), TomlDiagnostic> {
        let quote = self.peek().unwrap();

        // Multi-line strings use a tripled delimiter.
        let is_multiline = self.peek_at(1) == Some(quote) && self.peek_at(2) == Some(quote);
        if is_multiline {
            self.bump_n(3);
            loop {
                if self.cursor >= self.bytes.len() {
                    return Err(self.err_at(start, "unterminated multi-line string".into()));
                }
                if self.peek() == Some(quote)
                    && self.peek_at(1) == Some(quote)
                    && self.peek_at(2) == Some(quote)
                {
                    self.bump_n(3);
                    break;
                }
                if quote == b'"' && self.peek() == Some(b'\\') {
                    self.bump_n(2);
                    continue;
                }
                self.bump();
            }
            return Ok((TomlType::String, self.cursor));
        }

        self.bump();
        loop {
            match self.peek() {
                None | Some(b'\n') => {
                    return Err(self.err_at(start, "unterminated string".into()))
                }
                Some(b'\\') if quote == b'"' => {
                    self.bump_n(2);
                }
                Some(c) if c == quote => {
                    self.bump();
                    break;
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
        Ok((TomlType::String, self.cursor))
    }

    /// Scans a number, boolean, or datetime token up to a value delimiter.
    fn scan_bare_token(&mut self, start: usize) -> usize {
        while let Some(b) = self.peek() {
            match b {
                b',' | b']' | b'}' | b'#' | b'\n' | b'\r' => break,
                _ => self.bump(),
            }
        }
        // Trailing whitespace is not part of the value.
        let mut end = self.cursor;
        while end > start && matches!(self.bytes[end - 1], b' ' | b'\t') {
            end -= 1;
        }
        self.cursor = end;
        while self.cursor > start
            && matches!(self.bytes[self.cursor - 1], b' ' | b'\t' | b'\n' | b'\r')
        {
            self.cursor -= 1;
            self.col -= 1;
        }
        self.cursor
    }
}

/// Builds a `TomlDiagnostic` with surrounding source context and a caret pointer.
pub fn build_toml_diagnostic(
    source: &str,
    byte_offset: usize,
    line: usize,
    col: usize,
    message: String,
) -> TomlDiagnostic {
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

    TomlDiagnostic {
        message,
        line,
        column: col,
        byte_offset,
        context_snippet: snippet.trim_end().to_string(),
        pointer,
    }
}

/// Returns 1-based (line, column) for a byte offset in `source`.
fn line_col_at(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Returns true if `b` may appear in a bare TOML key.
fn is_bare_key_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

/// Classifies a bare token as a TOML number, boolean, or datetime.
fn classify_bare_value(text: &str) -> Option<TomlType> {
    if text.is_empty() {
        return None;
    }
    if text == "true" || text == "false" {
        return Some(TomlType::Boolean);
    }

    let is_datetime = {
        // RFC 3339 date-time, local date, or local time.
        let bytes = text.as_bytes();
        let date_like = bytes.len() >= 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes[..4].iter().all(u8::is_ascii_digit);
        let time_like = text.contains(':');
        date_like || time_like
    };
    if is_datetime {
        return Some(TomlType::Datetime);
    }

    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let lowered = cleaned.to_ascii_lowercase();

    if lowered == "inf" || lowered == "+inf" || lowered == "-inf" || lowered == "nan"
        || lowered == "+nan" || lowered == "-nan"
    {
        return Some(TomlType::Float);
    }

    // Hex / octal / binary integers.
    for prefix in ["0x", "0o", "0b"] {
        if lowered.starts_with(prefix) {
            let digits = &lowered[prefix.len()..];
            let radix = match prefix {
                "0x" => 16,
                "0o" => 8,
                _ => 2,
            };
            if !digits.is_empty() && digits.chars().all(|c| c.is_digit(radix)) {
                return Some(TomlType::Integer);
            }
            return None;
        }
    }

    let numeric_body = lowered.strip_prefix('+').or_else(|| lowered.strip_prefix('-'));
    let body = numeric_body.unwrap_or(&lowered);
    if body.is_empty() {
        return None;
    }

    if body.contains('.') || body.contains('e') {
        // Must look like a float: digits with at most one exponent marker.
        let mut seen_exp = false;
        for (i, c) in body.chars().enumerate() {
            let is_last = i + 1 == body.len();
            match c {
                'e' => {
                    if seen_exp {
                        return None;
                    }
                    seen_exp = true;
                    if !is_last && !matches!(body.as_bytes()[i + 1], b'+' | b'-') && !body.as_bytes()[i + 1].is_ascii_digit() {
                        return None;
                    }
                }
                '+' | '-' => {
                    // Only valid directly after `e` or as a leading sign.
                    if i != 0 && !seen_exp {
                        return None;
                    }
                    if i == 0 && !lowered.starts_with('-') && !lowered.starts_with('+') {
                        return None;
                    }
                }
                '.' => {
                    if seen_exp {
                        return None;
                    }
                }
                d if d.is_ascii_digit() => {}
                _ => return None,
            }
        }
        let without_exp: String = body.chars().take_while(|c| *c != 'e').collect();
        if without_exp.is_empty() {
            return None;
        }
        if !without_exp.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return None;
        }
        return Some(TomlType::Float);
    }

    if body.chars().all(|c| c.is_ascii_digit()) {
        return Some(TomlType::Integer);
    }

    None
}

/// Appends a single dotted segment to a path.
fn append_segment(path: &str, seg: &str) -> String {
    if path == "$" {
        format!("$.{seg}")
    } else {
        format!("{path}.{seg}")
    }
}

/// Renders a short preview for a string value, truncating long content.
fn preview_from_source(text: &str) -> String {
    const MAX: usize = 60;
    let mut out = String::new();
    let mut count = 0usize;
    for ch in text.chars() {
        if count >= MAX {
            out.push_str("...");
            break;
        }
        out.push(ch);
        count += 1;
    }
    out
}
