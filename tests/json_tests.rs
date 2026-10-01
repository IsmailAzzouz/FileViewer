use file_viewer::formats::json::{format_json, minify_json, parse_json, JsonType};
use file_viewer::formats::node::NodeType;

#[test]
fn test_parse_empty_and_whitespace() {
    assert_eq!(parse_json("").unwrap(), None);
    assert_eq!(parse_json("   \n\t  ").unwrap(), None);
}

#[test]
fn test_parse_primitives() {
    let node = parse_json("true").unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Boolean));
    assert_eq!(node.value_preview, "true");

    let node = parse_json("false").unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Boolean));
    assert_eq!(node.value_preview, "false");

    let node = parse_json("null").unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Null));
    assert_eq!(node.value_preview, "null");

    let node = parse_json("42.5e2").unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Number));
    assert_eq!(node.value_preview, "42.5e2");

    let node = parse_json("\"Hello \\n World!\"").unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::String));
    assert_eq!(node.value_preview, "\"Hello \n World!\"");
}

#[test]
fn test_parse_complex_object_and_spans() {
    let source = r#"{
  "name": "FileViewer",
  "version": 1,
  "features": ["json", "tree"],
  "active": true
}"#;

    let root = parse_json(source).unwrap().unwrap();
    assert_eq!(root.node_type, NodeType::Json(JsonType::Object));
    assert_eq!(root.children.len(), 4);

    let name_node = &root.children[0];
    assert_eq!(name_node.key.as_deref(), Some("name"));
    assert_eq!(name_node.node_type, NodeType::Json(JsonType::String));
    assert_eq!(name_node.path, "$.name");

    let features_node = &root.children[2];
    assert_eq!(features_node.key.as_deref(), Some("features"));
    assert_eq!(features_node.node_type, NodeType::Json(JsonType::Array));
    assert_eq!(features_node.children.len(), 2);
    assert_eq!(features_node.children[0].path, "$.features[0]");
    assert_eq!(features_node.children[1].path, "$.features[1]");

    // Verify root spans encompass the whole document
    assert_eq!(root.span.start_line, 1);
    assert_eq!(root.span.start_col, 1);
    assert_eq!(root.span.end_line, 6);
}

#[test]
fn test_node_lookup_by_offset() {
    let source = r#"{"a": 10, "b": "text"}"#;
    let root = parse_json(source).unwrap().unwrap();

    // Offset inside "text"
    let text_offset = source.find("text").unwrap();
    let found = root.find_node_at_offset(text_offset).unwrap();
    assert_eq!(found.key.as_deref(), Some("b"));
    assert_eq!(found.node_type, NodeType::Json(JsonType::String));

    // Offset inside 10
    let num_offset = source.find("10").unwrap();
    let found = root.find_node_at_offset(num_offset).unwrap();
    assert_eq!(found.key.as_deref(), Some("a"));
    assert_eq!(found.node_type, NodeType::Json(JsonType::Number));
}

#[test]
fn test_error_trailing_comma_object() {
    let source = r#"{"a": 1, }"#;
    let err = parse_json(source).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("Trailing comma"));
    assert!(!err.context_snippet.is_empty());
}

#[test]
fn test_error_trailing_comma_array() {
    let source = r#"[1, 2, ]"#;
    let err = parse_json(source).unwrap_err();
    assert_eq!(err.line, 1);
    assert!(err.message.contains("Trailing comma"));
}

#[test]
fn test_error_unclosed_string() {
    let source = r#"{"name": "Alice}"#;
    let err = parse_json(source).unwrap_err();
    assert!(err.message.contains("Unterminated string"));
}

#[test]
fn test_error_multiple_roots() {
    let source = r#"{"a": 1} {"b": 2}"#;
    let err = parse_json(source).unwrap_err();
    assert!(err.message.contains("Unexpected trailing content"));
}

#[test]
fn test_formatting_and_minification() {
    let messy = "{\n\"b\":[1,2],   \"a\":   true  \n}";
    let formatted = format_json(messy, 2).unwrap();
    assert!(formatted.contains("  \"b\": [\n    1,\n    2\n  ],"));
    assert!(formatted.contains("  \"a\": true"));

    let minified = minify_json(messy).unwrap();
    assert_eq!(minified, r#"{"b":[1,2],"a":true}"#);
}

#[test]
fn test_surrogate_pair_parsing() {
    // RFC 8259 Section 7 surrogate pairs
    let source = r#"{"emoji": "\uD83D\uDE00", "globe": "\uD83C\uDF0D"}"#;
    let root = parse_json(source).unwrap().unwrap();
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.children[0].value_preview, "\"😀\"");
    assert_eq!(root.children[1].value_preview, "\"🌍\"");
}

#[test]
fn test_multibyte_error_column() {
    // 'é' is 2 bytes in UTF-8, but 1 character column.
    // Line: {"clé": 1, ?}
    // Cols: 123456789012
    let source = "{\"clé\": 1, ?}";
    let err = parse_json(source).unwrap_err();
    assert_eq!(err.line, 1);
    assert_eq!(err.column, 12);
}

#[test]
fn test_recursion_depth_limit() {
    use file_viewer::formats::json::MAX_PARSE_DEPTH;
    assert_eq!(MAX_PARSE_DEPTH, 128);

    // Within limit: depth of 50 should parse successfully
    let mut within_limit = String::new();
    for _ in 0..50 {
        within_limit.push('[');
    }
    within_limit.push_str("42");
    for _ in 0..50 {
        within_limit.push(']');
    }
    assert!(parse_json(&within_limit).is_ok());

    // Exceeding limit: depth of 150 should return depth diagnostic error without stack overflow
    let mut excessive = String::new();
    for _ in 0..150 {
        excessive.push('[');
    }
    excessive.push_str("42");
    for _ in 0..150 {
        excessive.push(']');
    }
    let err = parse_json(&excessive).unwrap_err();
    assert!(
        err.message
            .contains("Maximum JSON nesting depth of 128 exceeded"),
        "Expected depth error, got: {}",
        err.message
    );
}

#[test]
fn test_utf8_bom_stripping() {
    let bom_json = "\u{FEFF}{\"valid\": true, \"count\": 42}";
    let root = parse_json(bom_json).unwrap().unwrap();
    assert_eq!(root.node_type, NodeType::Json(JsonType::Object));
    assert_eq!(root.children.len(), 2);

    let bom_array = "\u{FEFF}[1, 2, 3]";
    let arr = parse_json(bom_array).unwrap().unwrap();
    assert_eq!(arr.node_type, NodeType::Json(JsonType::Array));
    assert_eq!(arr.children.len(), 3);
}
