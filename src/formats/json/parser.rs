//! JSON parser with accurate source span tracking and contextual diagnostics.
//!
//! Implements RFC 8259 compliant parsing while preserving start and end byte
//! offsets, lines, and columns for all keys, values, objects, and arrays.

use super::model::{JsonSpan, JsonType};
use crate::formats::node::{NodeType, TreeNode};

/// Permissive grammar variants accepted by the JSON scanner.
///
/// All three dialects emit the same [`TreeNode`] model with [`JsonType`]
/// leaves, so the tree view, symbol index, and editor/tree synchronization
/// never learn which dialect produced a document. Only [`JsonDiagnostic`]
/// wording differs between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JsonDialect {
    /// RFC 8259 JSON: no comments and no trailing commas.
    #[default]
    Strict,
    /// JSON with Comments: `//` and `/* */` comments plus trailing commas.
    ///
    /// This is the dialect used by `tsconfig.json`, `.vscode/*.json`, and
    /// most other editor-facing JSON configuration files.
    Jsonc,
    /// JSON Lines: one strict JSON value per line, no comments.
    Jsonl,
}

impl JsonDialect {
    /// Returns true when `//` and `/* */` comments are treated as trivia.
    fn allows_comments(self) -> bool {
        matches!(self, Self::Jsonc)
    }

    /// Returns true when a comma may precede a closing `}` or `]`.
    fn allows_trailing_comma(self) -> bool {
        matches!(self, Self::Jsonc)
    }
}

/// Diagnostic information about a JSON parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonDiagnostic {
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

impl std::fmt::Display for JsonDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Line {}, Column {}: {}\n{}\n{}",
            self.line, self.column, self.message, self.context_snippet, self.pointer
        )
    }
}

/// Lexical tokens for the JSON grammar.
#[derive(Debug, Clone, PartialEq)]
enum TokenKind {
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Colon,
    Comma,
    String(String),
    Number(String),
    True,
    False,
    Null,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    kind: TokenKind,
    span: JsonSpan,
}

/// Tokenizer for scanning JSON text.
struct Scanner<'a> {
    /// Full document text, retained for diagnostic context snippets.
    source: &'a str,
    /// Full document bytes, indexed by absolute offset.
    bytes: &'a [u8],
    /// Absolute byte offset of the next byte to scan.
    cursor: usize,
    /// Exclusive byte offset at which scanning stops.
    limit: usize,
    line: usize,
    col: usize,
    dialect: JsonDialect,
}

impl<'a> Scanner<'a> {
    fn with_dialect(source: &'a str, dialect: JsonDialect) -> Self {
        let source = source.strip_prefix('\u{FEFF}').unwrap_or(source);
        Self::range(source, dialect, 0, 1, source.len())
    }

    /// Creates a scanner over `[start_byte, limit)` of `source` that still
    /// reports absolute positions.
    ///
    /// This lets a single JSON Lines record be parsed straight out of the
    /// enclosing document, so record spans need no second offset pass.
    fn range(
        source: &'a str,
        dialect: JsonDialect,
        start_byte: usize,
        start_line: usize,
        limit: usize,
    ) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            cursor: start_byte,
            limit,
            line: start_line,
            col: 1,
            dialect,
        }
    }

    /// Returns the exclusive byte offset at which scanning stops.
    #[inline]
    fn end(&self) -> usize {
        self.limit
    }

    /// Returns true when the scan range is exhausted.
    #[inline]
    fn at_end(&self) -> bool {
        self.cursor >= self.limit
    }

    /// Skips whitespace and, when the dialect allows it, comments.
    ///
    /// Complexity: O(W + C) where W and C are the counts of skipped
    /// whitespace and comment bytes.
    fn skip_trivia(&mut self) -> Result<(), JsonDiagnostic> {
        loop {
            while self.cursor < self.limit {
                match self.bytes[self.cursor] {
                    b' ' | b'\t' | b'\r' => {
                        self.cursor += 1;
                        self.col += 1;
                    }
                    b'\n' => {
                        self.cursor += 1;
                        self.line += 1;
                        self.col = 1;
                    }
                    _ => break,
                }
            }

            if !self.dialect.allows_comments() || self.cursor + 1 >= self.limit {
                return Ok(());
            }
            if self.bytes[self.cursor] != b'/' {
                return Ok(());
            }

            match self.bytes[self.cursor + 1] {
                // Line comment runs to the end of the line.
                b'/' => {
                    while self.cursor < self.limit && self.bytes[self.cursor] != b'\n' {
                        self.advance();
                    }
                }
                // Block comment runs to the closing delimiter.
                b'*' => {
                    let (start_line, start_col) = (self.line, self.col);
                    self.advance();
                    self.advance();
                    loop {
                        if self.at_end() {
                            return Err(build_diagnostic(
                                self.source,
                                self.cursor,
                                start_line,
                                start_col,
                                "Unterminated block comment: missing closing '*/'".to_string(),
                            ));
                        }
                        if self.bytes[self.cursor] == b'*'
                            && self.cursor + 1 < self.limit
                            && self.bytes[self.cursor + 1] == b'/'
                        {
                            self.advance();
                            self.advance();
                            break;
                        }
                        self.advance();
                    }
                }
                // A lone '/' is not trivia; let the caller report it.
                _ => return Ok(()),
            }
        }
    }

    /// Scans the next token.
    ///
    /// Complexity: O(L) where L is the length of the scanned token.
    fn next_token(&mut self) -> Result<Token, JsonDiagnostic> {
        self.skip_trivia()?;

        if self.at_end() {
            let span = JsonSpan::new(
                self.cursor,
                self.cursor,
                self.line,
                self.col,
                self.line,
                self.col,
            );
            return Ok(Token {
                kind: TokenKind::Eof,
                span,
            });
        }

        let start_byte = self.cursor;
        let start_line = self.line;
        let start_col = self.col;
        let b = self.bytes[self.cursor];

        match b {
            b'{' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftBrace,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b'}' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::RightBrace,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b'[' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::LeftBracket,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b']' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::RightBracket,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b':' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Colon,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b',' => {
                self.advance();
                Ok(Token {
                    kind: TokenKind::Comma,
                    span: JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    ),
                })
            }
            b'"' => self.scan_string(start_byte, start_line, start_col),
            b'-' | b'0'..=b'9' => self.scan_number(start_byte, start_line, start_col),
            b't' | b'f' | b'n' => self.scan_keyword(start_byte, start_line, start_col),
            unexpected => {
                let char_desc = if unexpected.is_ascii_graphic() {
                    format!("'{}'", unexpected as char)
                } else {
                    format!("0x{:02X}", unexpected)
                };
                Err(build_diagnostic(
                    self.source,
                    start_byte,
                    start_line,
                    start_col,
                    format!("Unexpected character {}", char_desc),
                ))
            }
        }
    }

    fn advance(&mut self) {
        if self.cursor < self.end() {
            if self.bytes[self.cursor] == b'\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            self.cursor += 1;
        }
    }

    fn advance_char(&mut self, ch: char) {
        if ch == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        self.cursor += ch.len_utf8();
    }

    /// Scans a string literal with escape sequence validation.
    ///
    /// Complexity: O(S) where S is the length of the string literal.
    fn scan_string(
        &mut self,
        start_byte: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<Token, JsonDiagnostic> {
        self.advance(); // Skip opening quote
        let mut string_content = String::new();

        while self.cursor < self.end() {
            let b = self.bytes[self.cursor];
            match b {
                b'"' => {
                    self.advance(); // Skip closing quote
                    let span = JsonSpan::new(
                        start_byte,
                        self.cursor,
                        start_line,
                        start_col,
                        self.line,
                        self.col,
                    );
                    return Ok(Token {
                        kind: TokenKind::String(string_content),
                        span,
                    });
                }
                b'\\' => {
                    self.advance();
                    if self.cursor >= self.end() {
                        return Err(build_diagnostic(
                            self.source,
                            start_byte,
                            start_line,
                            start_col,
                            "Unterminated escape sequence in string literal".to_string(),
                        ));
                    }
                    let escaped = self.bytes[self.cursor];
                    match escaped {
                        b'"' => string_content.push('"'),
                        b'\\' => string_content.push('\\'),
                        b'/' => string_content.push('/'),
                        b'b' => string_content.push('\x08'),
                        b'f' => string_content.push('\x0C'),
                        b'n' => string_content.push('\n'),
                        b'r' => string_content.push('\r'),
                        b't' => string_content.push('\t'),
                        b'u' => {
                            self.advance();
                            let hex_chars =
                                self.scan_hex_digits(4, start_byte, start_line, start_col)?;
                            // Support UTF-16 surrogate pairs (RFC 8259 Section 7)
                            if (0xD800..=0xDBFF).contains(&hex_chars)
                                && self.cursor + 1 < self.end()
                                && self.bytes[self.cursor] == b'\\'
                                && self.bytes[self.cursor + 1] == b'u'
                            {
                                self.advance(); // consume '\'
                                self.advance(); // consume 'u'
                                let low_surrogate =
                                    self.scan_hex_digits(4, start_byte, start_line, start_col)?;
                                if (0xDC00..=0xDFFF).contains(&low_surrogate) {
                                    let code_point = 0x10000
                                        + (((hex_chars - 0xD800) << 10) | (low_surrogate - 0xDC00));
                                    if let Some(ch) = char::from_u32(code_point) {
                                        string_content.push(ch);
                                        continue;
                                    }
                                }
                            }
                            if let Some(ch) = char::from_u32(hex_chars) {
                                string_content.push(ch);
                            } else {
                                return Err(build_diagnostic(
                                    self.source,
                                    start_byte,
                                    start_line,
                                    start_col,
                                    "Invalid Unicode code point in string escape".to_string(),
                                ));
                            }
                            continue;
                        }
                        other => {
                            return Err(build_diagnostic(
                                self.source,
                                self.cursor,
                                self.line,
                                self.col,
                                format!("Invalid escape character '\\{}'", other as char),
                            ));
                        }
                    }
                    self.advance();
                }
                0x00..=0x1F => {
                    return Err(build_diagnostic(
                        self.source,
                        self.cursor,
                        self.line,
                        self.col,
                        "Unescaped control character in string literal".to_string(),
                    ));
                }
                _ => {
                    // Safe UTF-8 character decode advancing character column accurately
                    let slice = &self.source[self.cursor..];
                    if let Some(ch) = slice.chars().next() {
                        string_content.push(ch);
                        self.advance_char(ch);
                    } else {
                        self.advance();
                    }
                }
            }
        }

        Err(build_diagnostic(
            self.source,
            start_byte,
            start_line,
            start_col,
            "Unterminated string literal: missing closing '\"'".to_string(),
        ))
    }

    fn scan_hex_digits(
        &mut self,
        count: usize,
        start_byte: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<u32, JsonDiagnostic> {
        let mut value = 0u32;
        for _ in 0..count {
            if self.cursor >= self.end() {
                return Err(build_diagnostic(
                    self.source,
                    start_byte,
                    start_line,
                    start_col,
                    "Expected 4 hexadecimal digits after '\\u'".to_string(),
                ));
            }
            let digit = match self.bytes[self.cursor] {
                b'0'..=b'9' => (self.bytes[self.cursor] - b'0') as u32,
                b'a'..=b'f' => (self.bytes[self.cursor] - b'a' + 10) as u32,
                b'A'..=b'F' => (self.bytes[self.cursor] - b'A' + 10) as u32,
                _ => {
                    return Err(build_diagnostic(
                        self.source,
                        self.cursor,
                        self.line,
                        self.col,
                        "Invalid hexadecimal digit in '\\u' escape".to_string(),
                    ));
                }
            };
            value = (value << 4) | digit;
            self.advance();
        }
        Ok(value)
    }

    /// Scans a numeric literal conforming to RFC 8259.
    ///
    /// Complexity: O(K) where K is the number of digits in the number.
    fn scan_number(
        &mut self,
        start_byte: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<Token, JsonDiagnostic> {
        if self.bytes[self.cursor] == b'-' {
            self.advance();
            if self.cursor >= self.end() || !self.bytes[self.cursor].is_ascii_digit() {
                return Err(build_diagnostic(
                    self.source,
                    self.cursor,
                    self.line,
                    self.col,
                    "Expected digit after '-' in number".to_string(),
                ));
            }
        }

        // Integer part
        if self.bytes[self.cursor] == b'0' {
            self.advance();
            if self.cursor < self.end() && self.bytes[self.cursor].is_ascii_digit() {
                return Err(build_diagnostic(
                    self.source,
                    self.cursor,
                    self.line,
                    self.col,
                    "Leading zeroes are not permitted in JSON numbers".to_string(),
                ));
            }
        } else {
            while self.cursor < self.end() && self.bytes[self.cursor].is_ascii_digit() {
                self.advance();
            }
        }

        // Fractional part
        if self.cursor < self.end() && self.bytes[self.cursor] == b'.' {
            self.advance();
            if self.cursor >= self.end() || !self.bytes[self.cursor].is_ascii_digit() {
                return Err(build_diagnostic(
                    self.source,
                    self.cursor,
                    self.line,
                    self.col,
                    "Expected digits after decimal point '.'".to_string(),
                ));
            }
            while self.cursor < self.end() && self.bytes[self.cursor].is_ascii_digit() {
                self.advance();
            }
        }

        // Exponent part
        if self.cursor < self.end()
            && (self.bytes[self.cursor] == b'e' || self.bytes[self.cursor] == b'E')
        {
            self.advance();
            if self.cursor < self.end()
                && (self.bytes[self.cursor] == b'+' || self.bytes[self.cursor] == b'-')
            {
                self.advance();
            }
            if self.cursor >= self.end() || !self.bytes[self.cursor].is_ascii_digit() {
                return Err(build_diagnostic(
                    self.source,
                    self.cursor,
                    self.line,
                    self.col,
                    "Expected digits in exponent".to_string(),
                ));
            }
            while self.cursor < self.end() && self.bytes[self.cursor].is_ascii_digit() {
                self.advance();
            }
        }

        let num_str = self.source[start_byte..self.cursor].to_string();
        let span = JsonSpan::new(
            start_byte,
            self.cursor,
            start_line,
            start_col,
            self.line,
            self.col,
        );
        Ok(Token {
            kind: TokenKind::Number(num_str),
            span,
        })
    }

    /// Scans keyword literals: true, false, null.
    ///
    /// Complexity: O(1).
    fn scan_keyword(
        &mut self,
        start_byte: usize,
        start_line: usize,
        start_col: usize,
    ) -> Result<Token, JsonDiagnostic> {
        let remainder = &self.source[start_byte..];
        if remainder.starts_with("true") {
            for _ in 0..4 {
                self.advance();
            }
            Ok(Token {
                kind: TokenKind::True,
                span: JsonSpan::new(
                    start_byte,
                    self.cursor,
                    start_line,
                    start_col,
                    self.line,
                    self.col,
                ),
            })
        } else if remainder.starts_with("false") {
            for _ in 0..5 {
                self.advance();
            }
            Ok(Token {
                kind: TokenKind::False,
                span: JsonSpan::new(
                    start_byte,
                    self.cursor,
                    start_line,
                    start_col,
                    self.line,
                    self.col,
                ),
            })
        } else if remainder.starts_with("null") {
            for _ in 0..4 {
                self.advance();
            }
            Ok(Token {
                kind: TokenKind::Null,
                span: JsonSpan::new(
                    start_byte,
                    self.cursor,
                    start_line,
                    start_col,
                    self.line,
                    self.col,
                ),
            })
        } else {
            Err(build_diagnostic(
                self.source,
                start_byte,
                start_line,
                start_col,
                "Unknown identifier, expected true, false, or null".to_string(),
            ))
        }
    }
}

/// Maximum allowed JSON nesting depth to prevent stack overflow on untrusted input.
/// 128 conforms to standard production JSON parsers (e.g. serde_json) and prevents stack exhaustion
/// within the standard 1MB Windows thread stack limit.
pub const MAX_PARSE_DEPTH: usize = 128;

/// Recursive descent parser building the shared [`TreeNode`] tree.
pub struct JsonParser<'a> {
    scanner: Scanner<'a>,
    current_token: Token,
    next_node_id: usize,
    depth: usize,
    path_buf: String,
    dialect: JsonDialect,
}

impl<'a> JsonParser<'a> {
    /// Creates a strict RFC 8259 parser for the given JSON source.
    ///
    /// Complexity: O(1).
    pub fn new(source: &'a str) -> Result<Self, JsonDiagnostic> {
        Self::with_dialect(source, JsonDialect::default())
    }

    /// Creates a parser for the given source using a permissive dialect.
    ///
    /// Complexity: O(1).
    pub fn with_dialect(source: &'a str, dialect: JsonDialect) -> Result<Self, JsonDiagnostic> {
        Self::build(Scanner::with_dialect(source, dialect), dialect)
    }

    /// Creates a parser restricted to `[start_byte, limit)` of `source`.
    ///
    /// Used to parse one JSON Lines record while keeping every span
    /// absolute to the enclosing document.
    ///
    /// Complexity: O(1).
    pub fn range(
        source: &'a str,
        start_byte: usize,
        start_line: usize,
        limit: usize,
    ) -> Result<Self, JsonDiagnostic> {
        let dialect = JsonDialect::Jsonl;
        Self::build(Scanner::range(source, dialect, start_byte, start_line, limit), dialect)
    }

    fn build(mut scanner: Scanner<'a>, dialect: JsonDialect) -> Result<Self, JsonDiagnostic> {
        let current_token = scanner.next_token()?;
        Ok(Self {
            scanner,
            current_token,
            next_node_id: 1,
            depth: 0,
            path_buf: String::with_capacity(128),
            dialect,
        })
    }

    /// Parses the entire document into an optional root `TreeNode` at `$`.
    /// Returns Ok(None) if the document is completely empty or only whitespace.
    ///
    /// Complexity: O(N) where N is the length of the source document.
    pub fn parse(self) -> Result<Option<TreeNode>, JsonDiagnostic> {
        self.parse_root("$")
    }

    /// Parses a single value and reports it at a caller-chosen root path.
    ///
    /// JSON Lines uses this to place each record at `$[index]`, so tree
    /// paths and symbol navigation read naturally for a multi-document file.
    ///
    /// Complexity: O(N) where N is the length of the scanned range.
    pub fn parse_root(mut self, root_path: &str) -> Result<Option<TreeNode>, JsonDiagnostic> {
        if self.current_token.kind == TokenKind::Eof {
            return Ok(None);
        }

        self.path_buf.push_str(root_path);
        let root = self.parse_value(None).map_err(|boxed| *boxed)?;

        if self.current_token.kind != TokenKind::Eof {
            return Err(build_diagnostic(
                self.scanner.source,
                self.current_token.span.start_byte,
                self.current_token.span.start_line,
                self.current_token.span.start_col,
                if self.dialect == JsonDialect::Jsonl {
                    "Unexpected content after JSON value: JSON Lines requires exactly one \
                     complete value per line"
                        .to_string()
                } else {
                    "Unexpected trailing content after JSON root element".to_string()
                },
            ));
        }

        Ok(Some(*root))
    }

    fn advance(&mut self) -> Result<Token, Box<JsonDiagnostic>> {
        let next = self.scanner.next_token().map_err(Box::new)?;
        Ok(std::mem::replace(&mut self.current_token, next))
    }

    /// Parses a single JSON value node (object, array, string, number, bool, null).
    /// Inlined array and object processing avoids duplicate stack frames per recursion depth.
    ///
    /// Complexity: O(V) where V is the subtree size.
    fn parse_value(
        &mut self,
        key: Option<String>,
    ) -> Result<Box<TreeNode>, Box<JsonDiagnostic>> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Err(Box::new(build_diagnostic(
                self.scanner.source,
                self.current_token.span.start_byte,
                self.current_token.span.start_line,
                self.current_token.span.start_col,
                format!("Maximum JSON nesting depth of {} exceeded", MAX_PARSE_DEPTH),
            )));
        }

        let id = self.alloc_id();
        let current_path = self.path_buf.clone();

        match &self.current_token.kind {
            TokenKind::LeftBrace => {
                let open_token = self.advance()?;
                let mut children = Vec::new();

                if self.current_token.kind == TokenKind::RightBrace {
                    let close_token = self.advance()?;
                    let span = JsonSpan::new(
                        open_token.span.start_byte,
                        close_token.span.end_byte,
                        open_token.span.start_line,
                        open_token.span.start_col,
                        close_token.span.end_line,
                        close_token.span.end_col,
                    );
                    return Ok(Box::new(TreeNode {
                        id,
                        key,
                        key_span: None,
                        node_type: NodeType::Json(JsonType::Object),
                        value_preview: "{ 0 items }".to_string(),
                        path: current_path,
                        span,
                        children,
                    }));
                }

                loop {
                    let (entry_key, key_span) = match &self.current_token.kind {
                        TokenKind::String(k) => {
                            let k_val = k.clone();
                            let span = self.current_token.span;
                            self.advance()?;
                            (k_val, span)
                        }
                        TokenKind::RightBrace => {
                            // A comma directly before the closing brace is a
                            // trailing comma: rejected by strict JSON,
                            // accepted by JSONC.
                            if self.dialect.allows_trailing_comma() {
                                break;
                            }
                            return Err(Box::new(build_diagnostic(
                                self.scanner.source,
                                self.current_token.span.start_byte,
                                self.current_token.span.start_line,
                                self.current_token.span.start_col,
                                "Trailing comma before '}' is not allowed in standard JSON"
                                    .to_string(),
                            )));
                        }
                        _ => {
                            return Err(Box::new(build_diagnostic(
                                self.scanner.source,
                                self.current_token.span.start_byte,
                                self.current_token.span.start_line,
                                self.current_token.span.start_col,
                                "Expected string for object key".to_string(),
                            )));
                        }
                    };

                    if self.current_token.kind != TokenKind::Colon {
                        return Err(Box::new(build_diagnostic(
                            self.scanner.source,
                            self.current_token.span.start_byte,
                            self.current_token.span.start_line,
                            self.current_token.span.start_col,
                            "Expected ':' after object key".to_string(),
                        )));
                    }
                    self.advance()?;

                    let prev_path_len = self.path_buf.len();
                    self.path_buf.push('.');
                    self.path_buf.push_str(&entry_key);

                    self.depth += 1;
                    let child_res = self.parse_value(Some(entry_key));
                    self.depth -= 1;
                    self.path_buf.truncate(prev_path_len);

                    let mut child_box = child_res?;
                    child_box.key_span = Some(key_span);
                    children.push(*child_box);

                    match self.current_token.kind {
                        TokenKind::Comma => {
                            self.advance()?;
                        }
                        TokenKind::RightBrace => break,
                        _ => {
                            return Err(Box::new(build_diagnostic(
                                self.scanner.source,
                                self.current_token.span.start_byte,
                                self.current_token.span.start_line,
                                self.current_token.span.start_col,
                                "Expected ',' or '}' in object".to_string(),
                            )));
                        }
                    }
                }

                let close_token = self.advance()?;
                let span = JsonSpan::new(
                    open_token.span.start_byte,
                    close_token.span.end_byte,
                    open_token.span.start_line,
                    open_token.span.start_col,
                    close_token.span.end_line,
                    close_token.span.end_col,
                );
                let preview = format!("{{ {} items }}", children.len());
                Ok(Box::new(TreeNode {
                    id,
                    key,
                    key_span: None,
                    node_type: NodeType::Json(JsonType::Object),
                    value_preview: preview,
                    path: current_path,
                    span,
                    children,
                }))
            }
            TokenKind::LeftBracket => {
                let open_token = self.advance()?;
                let mut children = Vec::new();

                if self.current_token.kind == TokenKind::RightBracket {
                    let close_token = self.advance()?;
                    let span = JsonSpan::new(
                        open_token.span.start_byte,
                        close_token.span.end_byte,
                        open_token.span.start_line,
                        open_token.span.start_col,
                        close_token.span.end_line,
                        close_token.span.end_col,
                    );
                    return Ok(Box::new(TreeNode {
                        id,
                        key,
                        key_span: None,
                        node_type: NodeType::Json(JsonType::Array),
                        value_preview: "[ 0 items ]".to_string(),
                        path: current_path,
                        span,
                        children,
                    }));
                }

                let mut index = 0usize;
                loop {
                    if self.current_token.kind == TokenKind::RightBracket {
                        // Trailing comma: permitted by JSONC, rejected by
                        // strict JSON.
                        if self.dialect.allows_trailing_comma() {
                            break;
                        }
                        return Err(Box::new(build_diagnostic(
                            self.scanner.source,
                            self.current_token.span.start_byte,
                            self.current_token.span.start_line,
                            self.current_token.span.start_col,
                            "Trailing comma before ']' is not allowed in standard JSON".to_string(),
                        )));
                    }

                    let prev_path_len = self.path_buf.len();
                    use std::fmt::Write;
                    let _ = write!(self.path_buf, "[{}]", index);

                    self.depth += 1;
                    let child_res = self.parse_value(None);
                    self.depth -= 1;
                    self.path_buf.truncate(prev_path_len);

                    children.push(*child_res?);
                    index += 1;

                    match self.current_token.kind {
                        TokenKind::Comma => {
                            self.advance()?;
                        }
                        TokenKind::RightBracket => break,
                        _ => {
                            return Err(Box::new(build_diagnostic(
                                self.scanner.source,
                                self.current_token.span.start_byte,
                                self.current_token.span.start_line,
                                self.current_token.span.start_col,
                                "Expected ',' or ']' in array".to_string(),
                            )));
                        }
                    }
                }

                let close_token = self.advance()?;
                let span = JsonSpan::new(
                    open_token.span.start_byte,
                    close_token.span.end_byte,
                    open_token.span.start_line,
                    open_token.span.start_col,
                    close_token.span.end_line,
                    close_token.span.end_col,
                );
                let preview = format!("[ {} items ]", children.len());
                Ok(Box::new(TreeNode {
                    id,
                    key,
                    key_span: None,
                    node_type: NodeType::Json(JsonType::Array),
                    value_preview: preview,
                    path: current_path,
                    span,
                    children,
                }))
            }
            TokenKind::String(_) => {
                let token = self.advance()?;
                if let TokenKind::String(val) = token.kind {
                    let preview = format!("\"{}\"", truncate_preview(&val, 32));
                    Ok(Box::new(TreeNode {
                        id,
                        key,
                        key_span: None,
                        node_type: NodeType::Json(JsonType::String),
                        value_preview: preview,
                        path: current_path,
                        span: token.span,
                        children: Vec::new(),
                    }))
                } else {
                    unreachable!()
                }
            }
            TokenKind::Number(_) => {
                let token = self.advance()?;
                if let TokenKind::Number(val) = token.kind {
                    Ok(Box::new(TreeNode {
                        id,
                        key,
                        key_span: None,
                        node_type: NodeType::Json(JsonType::Number),
                        value_preview: val,
                        path: current_path,
                        span: token.span,
                        children: Vec::new(),
                    }))
                } else {
                    unreachable!()
                }
            }
            TokenKind::True => {
                let token = self.advance()?;
                Ok(Box::new(TreeNode {
                    id,
                    key,
                    key_span: None,
                    node_type: NodeType::Json(JsonType::Boolean),
                    value_preview: "true".to_string(),
                    path: current_path,
                    span: token.span,
                    children: Vec::new(),
                }))
            }
            TokenKind::False => {
                let token = self.advance()?;
                Ok(Box::new(TreeNode {
                    id,
                    key,
                    key_span: None,
                    node_type: NodeType::Json(JsonType::Boolean),
                    value_preview: "false".to_string(),
                    path: current_path,
                    span: token.span,
                    children: Vec::new(),
                }))
            }
            TokenKind::Null => {
                let token = self.advance()?;
                Ok(Box::new(TreeNode {
                    id,
                    key,
                    key_span: None,
                    node_type: NodeType::Json(JsonType::Null),
                    value_preview: "null".to_string(),
                    path: current_path,
                    span: token.span,
                    children: Vec::new(),
                }))
            }
            _ => Err(Box::new(build_diagnostic(
                self.scanner.source,
                self.current_token.span.start_byte,
                self.current_token.span.start_line,
                self.current_token.span.start_col,
                "Expected JSON value (object, array, string, number, bool, or null)".to_string(),
            ))),
        }
    }

    fn alloc_id(&mut self) -> usize {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }
}

/// Helper function to build a structured diagnostic with code snippet context.
///
/// Complexity: O(L) where L is the line number of the error.
pub fn build_diagnostic(
    source: &str,
    byte_offset: usize,
    line: usize,
    col: usize,
    message: String,
) -> JsonDiagnostic {
    let zero_line = if line > 0 { line - 1 } else { 0 };
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
            snippet.push_str(&format!("{} {:4} | {}\n", prefix, line_num, line_content));
        }
    }

    // Caret pointer alignment
    let spaces = " ".repeat(col.saturating_sub(1) + 9);
    let pointer = format!("{}^", spaces);

    JsonDiagnostic {
        message,
        line,
        column: col,
        byte_offset,
        context_snippet: snippet.trim_end().to_string(),
        pointer,
    }
}

fn truncate_preview(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{}...", truncated)
    } else {
        s.to_string()
    }
}

/// Renumbers a subtree with globally unique ids in pre-order.
///
/// Each JSON Lines record is parsed by its own parser, so ids restart at 1 per
/// record and would otherwise collide in the merged tree. `TreeState` relies on
/// ids being unique for expansion state and selection tracking.
///
/// Complexity: O(N) where N is the number of nodes in the subtree.
fn renumber(node: &mut TreeNode, next_id: &mut usize) {
    node.id = *next_id;
    *next_id += 1;
    for child in &mut node.children {
        renumber(child, next_id);
    }
}

/// Shifts every span in the subtree forward by `offset` bytes.
///
/// Spans are produced relative to BOM-stripped text, but callers index into the
/// original buffer, so a leading BOM would otherwise shift every span by its own
/// length and make each span point at the wrong text.
///
/// Complexity: O(N) where N is the number of nodes in the subtree.
fn shift_spans(node: &mut TreeNode, offset: usize) {
    node.span.start_byte += offset;
    node.span.end_byte += offset;
    if let Some(key) = &mut node.key_span {
        key.start_byte += offset;
        key.end_byte += offset;
    }
    for child in &mut node.children {
        shift_spans(child, offset);
    }
}

/// Parses a JSON with Comments document.
///
/// Accepts `//` and `/* */` comments and trailing commas, which is the dialect
/// used by `tsconfig.json` and `.vscode/*.json`. Comments are trivia: they are
/// skipped and never become tree nodes.
///
/// Returns `Ok(None)` if the source is empty, whitespace, or comment-only.
///
/// Complexity: O(N) where N is the length of the source.
pub fn parse_jsonc(source: &str) -> Result<Option<TreeNode>, JsonDiagnostic> {
    JsonParser::with_dialect(source, JsonDialect::Jsonc)?.parse()
}

/// Parses a JSON Lines (`.jsonl` / `.ndjson`) document.
///
/// Each non-blank line must hold exactly one complete JSON value, which is the
/// defining constraint of the format. Records become children of a synthetic
/// array root so the tree view, filter, and symbol index work unchanged. Record
/// `i` is reported at path `$[i]`, and every span stays absolute to the
/// enclosing document, so editor/tree synchronization selects the right record.
///
/// Returns `Ok(None)` if the source is empty or whitespace only.
///
/// Complexity: O(N) where N is the length of the source.
pub fn parse_jsonl(source: &str) -> Result<Option<TreeNode>, JsonDiagnostic> {
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    if stripped.trim().is_empty() {
        return Ok(None);
    }

    let mut children: Vec<TreeNode> = Vec::new();
    let mut next_id = 1usize;
    let mut byte = 0usize;
    // Byte length of any stripped BOM; spans are made absolute to `source` so
    // that callers indexing the original buffer stay in sync.
    let bom_len = source.len() - stripped.len();

    for (line_idx, raw_line) in stripped.split_inclusive('\n').enumerate() {
        let line_no = line_idx + 1;
        let content = raw_line.trim_end_matches(['\n', '\r']);
        let line_start = byte;
        byte += raw_line.len();

        if !content.trim().is_empty() {
            let root_path = format!("$[{}]", children.len());
            let limit = line_start + content.len();
            // Parse the BOM-stripped text: the byte offsets above are already
            // relative to it, and leaving the BOM in would make the scanner
            // report it as a stray character in the first record.
            let parser = JsonParser::range(stripped, line_start, line_no, limit)?;
            if let Some(mut record) = parser.parse_root(&root_path)? {
                renumber(&mut record, &mut next_id);
                shift_spans(&mut record, bom_len);
                children.push(record);
            }
        }
    }

    if children.is_empty() {
        return Ok(None);
    }

    let total_lines = stripped.lines().count();
    let last_line_len = stripped.lines().last().unwrap_or("").chars().count();

    Ok(Some(TreeNode {
        id: 0,
        key: None,
        key_span: None,
        node_type: NodeType::Json(JsonType::Array),
        value_preview: format!("[ {} records ]", children.len()),
        path: "$".to_string(),
        // The synthetic root spans the whole original document, BOM included:
        // it contains every record, and its start stays at 0 so selecting the
        // root highlights from the very first byte of the file.
        span: JsonSpan::new(
            0,
            source.len(),
            1,
            1,
            total_lines,
            last_line_len + 1,
        ),
        children,
    }))
}
