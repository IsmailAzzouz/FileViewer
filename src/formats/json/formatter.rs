//! Formatting and minification utilities for JSON documents.
//!
//! Strict JSON is handled by a `serde_json` round trip, which is safe because
//! JSON carries no comments and no formatting significance. JSONC and JSON
//! Lines cannot use that path: reserializing through a value tree would
//! silently delete every comment, so those dialects are normalized by a
//! line-oriented pass that rewrites whitespace in place and leaves comment
//! text untouched.

use super::parser::{build_diagnostic, JsonDiagnostic};

/// Formats a raw JSON string with consistent indentation.
///
/// Preserves key order from the input document.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn format_json(raw: &str, indent_spaces: usize) -> Result<String, JsonDiagnostic> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| {
        let line = err.line();
        let col = err.column();
        // Calculate byte offset approximately from line and col
        let byte_offset = calculate_byte_offset(raw, line, col);
        build_diagnostic(
            raw,
            byte_offset,
            line,
            col,
            format!("Invalid JSON: {}", err),
        )
    })?;

    let indent_bytes = " ".repeat(indent_spaces).into_bytes();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent_bytes);
    let mut buffer = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
    serde::Serialize::serialize(&value, &mut serializer).map_err(|err| {
        build_diagnostic(raw, 0, 1, 1, format!("Failed to serialize JSON: {}", err))
    })?;

    String::from_utf8(buffer)
        .map_err(|err| build_diagnostic(raw, 0, 1, 1, format!("UTF-8 encoding error: {}", err)))
}

/// Minifies a raw JSON string by removing all insignificant whitespace.
///
/// Complexity: O(N) where N is the length of the JSON string.
pub fn minify_json(raw: &str) -> Result<String, JsonDiagnostic> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|err| {
        let line = err.line();
        let col = err.column();
        let byte_offset = calculate_byte_offset(raw, line, col);
        build_diagnostic(
            raw,
            byte_offset,
            line,
            col,
            format!("Invalid JSON: {}", err),
        )
    })?;

    serde_json::to_string(&value)
        .map_err(|err| build_diagnostic(raw, 0, 1, 1, format!("Failed to minify JSON: {}", err)))
}

/// One lexical piece of a source line.
///
/// A line is a sequence of these in source order, because a line may legally
/// interleave code and comment text: `1 /* mid */ , 2`. Collapsing a line to
/// "code plus one trailing comment" cannot represent that and silently drops
/// the mid-line comment.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    /// JSON code, to be whitespace-normalized.
    Code(String),
    /// Comment text, to be emitted verbatim.
    Comment(String),
}

/// What kind of separator is owed before the next token.
///
/// A bare `bool` is not enough, because `:` and `,` both introduce a *value*,
/// and a value may itself be an opener. `{"b":[1,2]}` must format as
/// `{"b": [1, 2]}` -- the `[` takes the space owed after `:` -- whereas `{` at
/// the start of a fragment or after `,` never takes a space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PendingSpace {
    /// Nothing is owed.
    None,
    /// A value is due; an opener still takes a leading space.
    Value,
    /// Only a non-opener token takes the space.
    Tight,
    /// Inside an already-emitted opener: whitespace is dropped outright, so
    /// `[ 1 ]` becomes `[1]` rather than `[ 1 ]`.
    InsideOpener,
}

impl PendingSpace {
    /// A value is due: record that and clear any tighter pending state.
    fn value() -> Self {
        PendingSpace::Value
    }

    /// Whitespace was seen; escalate to `Value` only if nothing was owed.
    fn observe_whitespace(&mut self) {
        if *self == PendingSpace::None {
            *self = PendingSpace::Tight;
        }
    }

    /// Whether an ordinary value token should be preceded by a space.
    fn owes_before_value(&self) -> bool {
        matches!(self, PendingSpace::Value | PendingSpace::Tight)
    }

    /// Whether an opener (`{` or `[`) should be preceded by a space.
    fn owes_before_opener(&self) -> bool {
        *self == PendingSpace::Value
    }
}

/// Splits one line into ordered code and comment pieces.
///
/// `in_block_comment` carries multi-line `/* ... */` state across lines. The
/// returned flag reports whether the line ended *still inside* a block comment.
/// That is deliberately distinct from "a block comment ended at the end of this
/// line", which leaves the carried state clear: conflating the two makes every
/// following line get misread as comment text.
///
/// Complexity: O(L) where L is the character length of the line.
fn split_line_pieces(line: &str, in_block_comment: bool) -> (Vec<Piece>, bool) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut code = String::with_capacity(line.len());
    let mut still_in_block = false;
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0usize;
    // Local copy: a block comment opened earlier can close mid-line, after
    // which the remainder of the line is code again.
    let mut in_block = in_block_comment;

    // Flush accumulated code before emitting a comment, so comment and code
    // pieces stay in source order.
    macro_rules! flush_code {
        () => {
            if !code.trim().is_empty() {
                pieces.push(Piece::Code(std::mem::take(&mut code)));
            }
        };
    }

    while i < len {
        let c = chars[i];

        if in_block {
            // Carried-in state: search forward for the closing delimiter. The
            // comment started on an earlier line, so it begins at index 0.
            let mut j = i;
            let mut closed = false;
            while j < len {
                if chars[j] == '*' && j + 1 < len && chars[j + 1] == '/' {
                    closed = true;
                    break;
                }
                j += 1;
            }
            let end = if closed { j + 2 } else { len };
            let comment: String = chars[..end].iter().collect();
            flush_code!();
            pieces.push(Piece::Comment(comment));
            if closed {
                in_block = false;
                i = end;
            } else {
                still_in_block = true;
                i = len;
            }
            continue;
        }

        if in_string {
            code.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                code.push(c);
                i += 1;
            }
            '/' if i + 1 < len && chars[i + 1] == '/' => {
                // Line comment: everything to end of line.
                let comment: String = chars[i..].iter().collect();
                flush_code!();
                pieces.push(Piece::Comment(comment));
                i = len;
            }
            '/' if i + 1 < len && chars[i + 1] == '*' => {
                // Find the closing delimiter. Its absence is what makes this
                // comment span into later lines; finding it exactly at
                // end-of-line is a *closed* comment and must not set that flag.
                let mut j = i + 2;
                let mut closed = false;
                while j < len {
                    if chars[j] == '*' && j + 1 < len && chars[j + 1] == '/' {
                        closed = true;
                        break;
                    }
                    j += 1;
                }
                let end = if closed { j + 2 } else { len };
                let comment: String = chars[i..end].iter().collect();
                flush_code!();
                pieces.push(Piece::Comment(comment));
                if closed {
                    i = j + 2;
                } else {
                    still_in_block = true;
                    i = len;
                }
                in_block = !closed;
            }
            _ => {
                code.push(c);
                i += 1;
            }
        }
    }

    flush_code!();

    (pieces, still_in_block)
}

/// Rewrites insignificant whitespace in a code fragment.
///
/// Collapses whitespace runs to a single space and emits exactly one space
/// after `:` and `,`. Whitespace inside string literals is preserved verbatim,
/// so string contents are never altered.
///
/// `pending` is threaded in and out so spacing stays correct across the code
/// pieces of a line, which matters when a comment splits the code:
/// `1/*c*/,2` must still emit `1, 2`, not `1 ,2`.
///
/// `strip_trailing_commas` is set only for minification, where the output must
/// be strict JSON; JSONC permits trailing commas and must keep them.
///
/// The transformation is a pure function of its input, which is what makes JSONC
/// formatting idempotent.
///
/// Complexity: O(L) where L is the character length of `code`.
fn normalize_code_into(
    code: &str,
    out: &mut String,
    pending: &mut PendingSpace,
    strip_trailing_commas: bool,
) {
    let mut in_string = false;
    let mut escaped = false;

    for c in code.chars() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                // Entering a string literal. Everything up to the matching
                // close quote is copied verbatim, so `//`, `/*` and repeated
                // spaces inside a string are never treated as code.
                if pending.owes_before_value() {
                    out.push(' ');
                }
                out.push(c);
                *pending = PendingSpace::None;
                in_string = true;
            }
            '{' | '[' => {
                if pending.owes_before_opener() {
                    out.push(' ');
                }
                out.push(c);
                // Anything up to the first value is an opener's own padding,
                // not a separator, so it must not survive as a space.
                *pending = PendingSpace::InsideOpener;
            }
            '}' | ']' => {
                // A trailing comma before a closer is legal JSONC but not strict
                // JSON, so minification drops it here.
                if strip_trailing_commas && out.ends_with(',') {
                    out.pop();
                }
                out.push(c);
                *pending = PendingSpace::InsideOpener;
            }
            ',' => {
                // No space before the comma itself: `a , b` is `a, b`.
                out.push(c);
                *pending = PendingSpace::value();
            }
            ':' => {
                // Likewise `a : b` is `a: b`.
                out.push(c);
                *pending = PendingSpace::value();
            }
            c if c.is_whitespace() => {
                pending.observe_whitespace();
            }
            _ => {
                if pending.owes_before_value() {
                    out.push(' ');
                }
                out.push(c);
                *pending = PendingSpace::None;
            }
        }
    }
}

/// Convenience wrapper around [`normalize_code_into`] for a standalone fragment.
///
/// Complexity: O(L) where L is the character length of `code`.
#[cfg(test)]
fn normalize_code(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    let mut pending = PendingSpace::None;
    normalize_code_into(code, &mut out, &mut pending, false);
    out
}

/// Counts structural characters in a code fragment and measures the leading run
/// of closing brackets.
///
/// `leading_closers` tells the caller how far to dedent a line before placing
/// it, which is what keeps `},` and `}]` aligned with their parent rather than
/// one level too deep.
///
/// Only code should be passed here: comment text may legally contain braces that
/// would otherwise corrupt the depth count.
///
/// Complexity: O(L) where L is the character length of `code`.
fn scan_structure(code: &str) -> (usize, usize, usize) {
    let mut leading_closers = 0usize;
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut seen_content = false;

    for c in code.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        match c {
            '"' => {
                in_string = true;
                seen_content = true;
            }
            '{' | '[' => {
                opens += 1;
                seen_content = true;
            }
            '}' | ']' => {
                closes += 1;
                if !seen_content {
                    leading_closers += 1;
                }
                seen_content = true;
            }
            c if c.is_whitespace() => {}
            _ => seen_content = true,
        }
    }

    (opens, closes, leading_closers)
}

/// Formats a JSON with Comments document without destroying comments.
///
/// Indentation and intra-line spacing are normalized while comments, blank
/// lines, and the author's line structure are preserved. This deliberately does
/// not re-flow a single-line document across several lines: doing so would
/// require dropping the comment attached to each value. It normalizes in place
/// instead.
///
/// Trailing commas are left in place, since JSONC permits them and the author's
/// intent is preserved by not removing them.
///
/// Complexity: O(N) where N is the length of the source.
pub fn format_jsonc(source: &str, indent_spaces: usize) -> Result<String, JsonDiagnostic> {
    // Validate first so invalid documents report a diagnostic instead of being
    // rewritten into a shape that merely looks formatted.
    super::parse_jsonc(source)?;

    let unit = " ".repeat(indent_spaces);
    let mut out = String::with_capacity(source.len() + source.len() / 8);
    let mut depth: usize = 0;
    let mut in_block_comment = false;
    // Match the parser, which skips a leading BOM. Leaving it in place would
    // make it a zero-width line of its own and corrupt the output.
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);

    for raw_line in stripped.lines() {
        if raw_line.trim().is_empty() {
            out.push('\n');
            continue;
        }

        let (pieces, still_in_block) = split_line_pieces(raw_line, in_block_comment);
        in_block_comment = still_in_block;

        // Depth accounting uses only code, since comments may contain braces.
        let code_only: String = pieces
            .iter()
            .filter_map(|p| match p {
                Piece::Code(c) => Some(c.as_str()),
                Piece::Comment(_) => None,
            })
            .collect();
        let (opens, closes, leading_closers) = scan_structure(&code_only);
        let line_depth = depth.saturating_sub(leading_closers);

        for _ in 0..line_depth {
            out.push_str(&unit);
        }

        // Comment-only and code lines share one path. `wrote_content` keeps a
        // leading comment from picking up stray spacing, and a comment in the
        // middle of a line both gains a leading space and leaves one owed to
        // the code that follows it.
        let mut pending = PendingSpace::None;
        let mut wrote_content = false;
        for piece in &pieces {
            match piece {
                Piece::Code(c) => {
                    // Indentation was already emitted above, so the piece's
                    // own leading whitespace is redundant. Leaving it in would
                    // re-arm `pending` and inject a stray space before the
                    // first token, which breaks idempotence.
                    let c = c.trim();
                    let before = out.len();
                    normalize_code_into(c, &mut out, &mut pending, false);
                    if out.len() != before {
                        wrote_content = true;
                    }
                }
                Piece::Comment(c) => {
                    if wrote_content && !out.ends_with(' ') {
                        out.push(' ');
                    }
                    out.push_str(c);
                    // A comment is transparent: it neither consumes nor
                    // introduces a pending space. Whatever separator was owed
                    // before it is still owed after, and a comment directly
                    // after an opener does not become `{ /* c */ "a"` with a
                    // stray space.
                    wrote_content = true;
                }
            }
        }

        out.push('\n');
        depth = depth + opens - closes;
    }

    Ok(out)
}

/// Minifies a JSON with Comments document into strict JSON.
///
/// This is deliberately lossy: minification removes comments and trailing
/// commas, yielding a document that parses as RFC 8259 JSON. Comments cannot
/// survive minification, so this is the destructive counterpart of
/// [`format_jsonc`], which preserves them.
///
/// The output is re-validated through `serde_json` before being returned, so
/// minified JSONC is always valid strict JSON.
///
/// Complexity: O(N) where N is the length of the source.
pub fn minify_jsonc(source: &str) -> Result<String, JsonDiagnostic> {
    super::parse_jsonc(source)?;

    // Reusing the piece splitter and the shared normalizer keeps comment
    // handling identical between the two paths; the only difference is that
    // comments are dropped and trailing commas are stripped.
    let mut out = String::with_capacity(source.len());
    let mut pending = PendingSpace::None;
    let mut in_block_comment = false;
    // Match the parser, which skips a leading BOM.
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);

    for raw_line in stripped.lines() {
        let (pieces, still_in_block) = split_line_pieces(raw_line, in_block_comment);
        in_block_comment = still_in_block;
        for piece in &pieces {
            match piece {
                // Trim for the same reason as `format_jsonc`: the piece's own
                // leading whitespace would otherwise become a stray space.
                Piece::Code(c) => normalize_code_into(c.trim(), &mut out, &mut pending, true),
                Piece::Comment(_) => {
                    // Dropped entirely, and transparent to spacing: the
                    // separator owed before the comment is still owed after.
                }
            }
        }
    }

    // Re-validating guarantees the comment stripper and comma removal produced
    // a well-formed document rather than something subtly truncated.
    serde_json::from_str::<serde_json::Value>(&out)
        .map_err(|err| build_diagnostic(source, 0, 1, 1, format!("Failed to minify: {}", err)))?;

    Ok(out)
}

/// Formats a JSON Lines document.
///
/// Each record is validated and emitted on exactly one line, which is the
/// defining constraint of the format. Record contents and order are preserved;
/// blank lines and trailing whitespace are dropped.
///
/// Complexity: O(N) where N is the length of the source.
pub fn format_jsonl(source: &str) -> Result<String, JsonDiagnostic> {
    super::parse_jsonl(source)?;

    let mut out = String::with_capacity(source.len());
    // Match the parser, which skips a leading BOM; otherwise the first record
    // is emitted with the BOM still attached.
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    for raw_line in stripped.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }

    Ok(out)
}

/// Minifies a JSON Lines document to one compact record per line.
///
/// Each record is re-serialized compactly, so whitespace inside a record is
/// removed while the one-record-per-line structure is preserved.
///
/// Complexity: O(N) where N is the length of the source.
pub fn minify_jsonl(source: &str) -> Result<String, JsonDiagnostic> {
    super::parse_jsonl(source)?;

    let mut out = String::with_capacity(source.len());
    // Match the parser, which skips a leading BOM.
    let stripped = source.strip_prefix('\u{FEFF}').unwrap_or(source);
    for raw_line in stripped.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let value: serde_json::Value = serde_json::from_str(line).map_err(|err| {
            let byte_offset = calculate_byte_offset(stripped, err.line(), err.column());
            build_diagnostic(
                source,
                byte_offset,
                err.line(),
                err.column(),
                format!("Invalid JSON Lines record: {}", err),
            )
        })?;

        let compact = serde_json::to_string(&value).map_err(|err| {
            build_diagnostic(source, 0, 1, 1, format!("Failed to minify record: {}", err))
        })?;

        out.push_str(&compact);
        out.push('\n');
    }

    Ok(out)
}

/// Helper function to compute byte offset from 1-based line and column.
///
/// Complexity: O(N).
fn calculate_byte_offset(source: &str, line: usize, col: usize) -> usize {
    let mut current_line = 1;
    let mut current_col = 1;

    for (offset, ch) in source.char_indices() {
        if current_line == line && current_col >= col {
            return offset;
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }
    source.len()
}

#[cfg(test)]
mod tests {
    use super::super::parse_json;
    use super::*;

    #[test]
    fn split_line_keeps_mid_line_block_comment() {
        let (pieces, still) = split_line_pieces("1 /* mid */ , 2", false);
        assert!(!still, "a closed comment must not leave block state set");
        assert_eq!(
            pieces,
            vec![
                Piece::Code("1 ".to_string()),
                Piece::Comment("/* mid */".to_string()),
                Piece::Code(" , 2".to_string()),
            ]
        );
    }

    #[test]
    fn split_line_block_comment_ending_at_eol_is_closed() {
        // The closing delimiter is the last thing on the line. This must not be
        // confused with an unterminated comment, or every later line is
        // misread as comment text.
        let (pieces, still) = split_line_pieces(r#""a": 1, /* tail */"#, false);
        assert!(!still, "a comment closed at end-of-line is not unterminated");
        assert_eq!(
            pieces.last(),
            Some(&Piece::Comment("/* tail */".to_string()))
        );
    }

    #[test]
    fn split_line_unterminated_block_comment_carries_state() {
        let (pieces, still) = split_line_pieces("/* opens", false);
        assert!(still, "unterminated block comment must carry to the next line");
        assert_eq!(pieces, vec![Piece::Comment("/* opens".to_string())]);

        let (pieces, still) = split_line_pieces("still text */ 5", true);
        assert!(!still, "closing delimiter must clear the carried state");
        assert_eq!(
            pieces,
            vec![
                Piece::Comment("still text */".to_string()),
                Piece::Code(" 5".to_string()),
            ]
        );
    }

    #[test]
    fn split_line_ignores_comment_markers_inside_strings() {
        let (pieces, still) = split_line_pieces(r#""a // b /* c */ d", 1"#, false);
        assert!(!still);
        assert_eq!(
            pieces,
            vec![Piece::Code(r#""a // b /* c */ d", 1"#.to_string())]
        );
    }

    #[test]
    fn normalize_code_keeps_string_contents_verbatim() {
        assert_eq!(normalize_code(r#""a   b""#), r#""a   b""#);
        assert_eq!(normalize_code(r#""x":"y""#), r#""x": "y""#);
    }

    #[test]
    fn normalize_code_drops_space_before_comma_and_colon() {
        assert_eq!(normalize_code("a , b"), "a, b");
        assert_eq!(normalize_code("a : b"), "a: b");
    }

    #[test]
    fn normalize_code_opens_value_with_a_space() {
        // The `[` takes the space owed after `:`.
        assert_eq!(normalize_code(r#"{"a":[1,2]}"#), r#"{"a": [1, 2]}"#);
        // A comment owed a space defers to the opener too.
        assert_eq!(normalize_code("[,1]"), "[, 1]");
    }

    #[test]
    fn format_jsonc_preserves_every_comment() {
        let src = "{\n  // line\n  /* block */ \"a\": 1, /* mid */\n  \"b\": 2\n}";
        let out = format_jsonc(src, 2).unwrap();
        assert!(out.contains("// line"), "line comment dropped:\n{out}");
        assert!(out.contains("/* block */"), "block comment dropped:\n{out}");
        assert!(out.contains("/* mid */"), "mid-line comment dropped:\n{out}");
    }

    #[test]
    fn format_jsonc_normalizes_across_a_mid_line_comment() {
        // The comment splits the array elements; spacing must survive the
        // split, so `1,c` is not emitted as `1 ,c`.
        let out = format_jsonc("{\"a\": [1 /*c*/,2]}", 2).unwrap();
        assert_eq!(out.trim_end(), "{\"a\": [1 /*c*/, 2]}");
    }

    #[test]
    fn format_jsonc_handles_multiline_block_comment() {
        let src = "{\n/* start\n   middle\n   end */\n\"a\": 1\n}";
        let out = format_jsonc(src, 2).unwrap();
        assert!(out.contains("/* start"), "opening lost:\n{out}");
        assert!(out.contains("end */"), "closing lost:\n{out}");
        // Code after the comment must be treated as code, not comment text.
        assert!(out.contains("\"a\": 1"), "code after block comment lost:\n{out}");
    }

    #[test]
    fn format_jsonc_keeps_trailing_commas() {
        // Trailing commas are legal JSONC and are part of the author's intent.
        let out = format_jsonc("{\"a\": 1, /* keep */ }", 2).unwrap();
        assert!(out.contains("/* keep */"));
        assert!(out.contains("1,"), "trailing comma dropped:\n{out}");
    }

    #[test]
    fn format_jsonc_is_idempotent() {
        let src = "{\n//  c\n\"a\": 1,/*m*/\n\"b\": [1, 2],\n}";
        let once = format_jsonc(src, 2).unwrap();
        let twice = format_jsonc(&once, 2).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn minify_jsonc_yields_valid_strict_json() {
        let src = "{\n // c\n \"a\": 1, /* d */\n \"b\": [1, 2,],\n}";
        let min = minify_jsonc(src).unwrap();
        assert!(!min.contains("//"), "line comment survived:\n{min}");
        assert!(!min.contains("/*"), "block comment survived:\n{min}");
        let v: serde_json::Value = serde_json::from_str(&min).unwrap();
        assert_eq!(
            v["b"][1],
            serde_json::json!(2),
            "trailing comma broke structure:\n{min}"
        );
    }

    #[test]
    fn minify_jsonc_keeps_comment_markers_inside_strings() {
        let min = minify_jsonc("{\"a\": \"http://x // y\"}").unwrap();
        assert!(min.contains("http://x // y"), "string content damaged:\n{min}");
    }

    #[test]
    fn jsonl_records_stay_one_per_line() {
        let src = " {\"a\" : 1} \n\n{\"b\":[1,  2]}\n";
        let fmt = format_jsonl(src).unwrap();
        assert_eq!(fmt.lines().count(), 2);
        let min = minify_jsonl(src).unwrap();
        assert_eq!(min.lines().count(), 2);
        assert_eq!(min.lines().next().unwrap(), "{\"a\":1}");
        assert_eq!(min.lines().nth(1).unwrap(), "{\"b\":[1,2]}");
    }

    #[test]
    fn strict_json_still_rejects_jsonc_syntax() {
        // Tolerance must not leak into the strict path.
        assert!(parse_json("{\"a\":1,}").is_err());
        assert!(parse_json("{\"a\":1 // c\n}").is_err());
    }
}
