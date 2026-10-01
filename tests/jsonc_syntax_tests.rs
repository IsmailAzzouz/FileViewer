//! Syntax tokenizer tests for JSON, JSONC, and JSON Lines.

use file_viewer::formats::json::{tokenize_json_line, tokenize_jsonc_line, StyledSegment};
use file_viewer::theme::{
    SYNTAX_BOOLEAN, SYNTAX_COMMENT, SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_STRING,
};

/// Concatenated text of a segment list, to assert nothing was lost.
fn text_of(segments: &[StyledSegment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect()
}

/// The single segment whose text contains `needle`.
fn segment_with<'a>(segments: &'a [StyledSegment], needle: &str) -> &'a StyledSegment {
    segments
        .iter()
        .find(|s| s.text.contains(needle))
        .unwrap_or_else(|| panic!("no segment contains {needle:?} in {segments:?}"))
}

#[test]
fn tokenize_json_line_covers_core_types() {
    let segments = tokenize_json_line(r#"{"key": 1, "s": "v", "t": true, "n": null}"#);
    assert_eq!(text_of(&segments), r#"{"key": 1, "s": "v", "t": true, "n": null}"#);
    assert_eq!(segment_with(&segments, "\"key\"").color, SYNTAX_KEY);
    assert_eq!(segment_with(&segments, "\"v\"").color, SYNTAX_STRING);
    assert_eq!(segment_with(&segments, "1").color, SYNTAX_NUMBER);
    assert_eq!(segment_with(&segments, "true").color, SYNTAX_BOOLEAN);
    assert_eq!(segment_with(&segments, "null").color, SYNTAX_NULL);
}

#[test]
fn tokenize_json_line_is_lossless_for_punctuation() {
    let line = r#"[[{"a":-1.5e+10}]]"#;
    let segments = tokenize_json_line(line);
    assert_eq!(text_of(&segments), line);
}

#[test]
fn tokenize_jsonc_line_colors_line_comments() {
    let segments = tokenize_jsonc_line(r#"  "a": 1, // trailing note"#);
    assert_eq!(text_of(&segments), r#"  "a": 1, // trailing note"#);
    assert_eq!(segment_with(&segments, "// trailing note").color, SYNTAX_COMMENT);
    assert_eq!(segment_with(&segments, "\"a\"").color, SYNTAX_KEY);
}

#[test]
fn tokenize_jsonc_line_colors_block_comments() {
    let segments = tokenize_jsonc_line("/* header */ \"a\": 1");
    assert_eq!(text_of(&segments), "/* header */ \"a\": 1");
    assert_eq!(segment_with(&segments, "/* header */").color, SYNTAX_COMMENT);
}

#[test]
fn tokenize_jsonc_line_colors_comment_ending_at_eol() {
    // The closing delimiter is the last thing on the line. It must still be
    // consumed as a comment, not left to the malformed-token fallback.
    let segments = tokenize_jsonc_line("\"a\": 1 /* tail */");
    assert_eq!(text_of(&segments), "\"a\": 1 /* tail */");
    assert_eq!(segment_with(&segments, "/* tail */").color, SYNTAX_COMMENT);
}

#[test]
fn tokenize_jsonc_line_keeps_mid_line_comment() {
    let segments = tokenize_jsonc_line("1 /* mid */, 2");
    assert_eq!(text_of(&segments), "1 /* mid */, 2");
    assert_eq!(segment_with(&segments, "/* mid */").color, SYNTAX_COMMENT);
}

#[test]
fn tokenize_jsonc_line_ignores_comment_markers_in_strings() {
    // The `//` and `/*` are string content, so the whole literal is one
    // SYNTAX_STRING segment and nothing is colored as a comment.
    let segments = tokenize_jsonc_line(r#"{"a": "http://x // y /* z */"}"#);
    assert_eq!(text_of(&segments), r#"{"a": "http://x // y /* z */"}"#);
    let literal = segment_with(&segments, "http://x");
    assert_eq!(literal.color, SYNTAX_STRING);
    assert!(
        !segments.iter().any(|s| s.color == SYNTAX_COMMENT),
        "string content miscolored as comment: {segments:?}"
    );
}

#[test]
fn tokenize_jsonc_line_recognizes_key_across_a_comment() {
    // A key whose colon is separated by a block comment is still a key.
    let segments = tokenize_jsonc_line("\"a\" /* why */ : 1");
    assert_eq!(segment_with(&segments, "\"a\"").color, SYNTAX_KEY);
}

#[test]
fn tokenize_jsonc_line_colors_unterminated_block_comment_to_eol() {
    // Each line is tokenized in isolation, so an unterminated `/*` colors to
    // end of line; the parser is what reports it as an error.
    let segments = tokenize_jsonc_line("/* opens");
    assert_eq!(text_of(&segments), "/* opens");
    assert_eq!(segments[0].color, SYNTAX_COMMENT);
}

#[test]
fn tokenize_jsonc_line_handles_jsonl_records() {
    // Each JSON Lines record is a self-contained line, tokenized independently.
    let segments = tokenize_jsonc_line(r#"{"id":1,"name":"a"}"#);
    assert_eq!(text_of(&segments), r#"{"id":1,"name":"a"}"#);
    assert_eq!(segment_with(&segments, "\"id\"").color, SYNTAX_KEY);
    assert_eq!(segment_with(&segments, "1").color, SYNTAX_NUMBER);
}
