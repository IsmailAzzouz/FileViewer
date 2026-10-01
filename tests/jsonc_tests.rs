//! Parser, formatter, and end-to-end tests for the JSONC and JSON Lines
//! dialects.
//!
//! These cover the tolerance that distinguishes them from strict JSON --
//! comments and trailing commas for JSONC, one-value-per-line for JSON Lines --
//! plus the invariants that make editor/tree synchronization trustworthy:
//! spans must be absolute to the enclosing document, and a parse must never
//! silently reinterpret one dialect's syntax as another's.

use file_viewer::editor::TextBuffer;
use file_viewer::formats::json::{
    format_jsonc, format_jsonl, minify_jsonc, minify_jsonl, parse_json, parse_jsonc, parse_jsonl,
    JsonType,
};
use file_viewer::formats::node::NodeType;
use file_viewer::tree::TreeState;

// ---------------------------------------------------------------------------
// JSONC: parsing
// ---------------------------------------------------------------------------

#[test]
fn test_jsonc_accepts_comments_and_trailing_commas() {
    let src = "{\n  // a line comment\n  /* a block comment */\n  \"a\": 1,\n  \"b\": [1, 2,],\n}";
    let node = parse_jsonc(src).unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Object));
    assert_eq!(node.children.len(), 2);
    assert_eq!(node.children[0].key.as_deref(), Some("a"));
    assert_eq!(node.children[1].key.as_deref(), Some("b"));
}

#[test]
fn test_jsonc_comments_are_trivia_not_nodes() {
    // Comments must never appear in the tree, or the tree and the text would
    // disagree about what is on screen.
    let src = "{\n  // c1\n  \"a\": 1, // c2\n  /* c3 */ \"b\": 2\n}";
    let node = parse_jsonc(src).unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
    for child in &node.children {
        assert!(!child.path.contains("c1"), "comment became a node: {}", child.path);
    }
}

#[test]
fn test_jsonc_ignores_comment_markers_inside_strings() {
    let src = r#"{"url": "http://x/y", "note": "a /* not a comment */ b"}"#;
    let node = parse_jsonc(src).unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
    assert_eq!(node.children[0].value_preview, "\"http://x/y\"");
    assert_eq!(node.children[1].value_preview, "\"a /* not a comment */ b\"");
}

#[test]
fn test_jsonc_spans_are_absolute_to_the_document() {
    // Spans must point at the real source text, including after comments and
    // blank lines, or cursor-to-tree sync would select the wrong node.
    let src = "// header\n\n{\n  \"alpha\": 1,\n  \"beta\": 2\n}\n";
    let node = parse_jsonc(src).unwrap().unwrap();
    let beta = node
        .children
        .iter()
        .find(|c| c.key.as_deref() == Some("beta"))
        .unwrap();
    assert_eq!(&src[beta.span.start_byte..beta.span.end_byte], "2");
    assert_eq!(beta.span.start_line, 5);
}

#[test]
fn test_jsonc_multiline_block_comment_does_not_shift_spans() {
    let src = "{\n/* multi\n   line */\n\"k\": \"v\"\n}";
    let node = parse_jsonc(src).unwrap().unwrap();
    let k = &node.children[0];
    assert_eq!(&src[k.span.start_byte..k.span.end_byte], "\"v\"");
    assert_eq!(k.span.start_line, 4);
}

#[test]
fn test_jsonc_empty_and_comment_only_documents() {
    assert_eq!(parse_jsonc("").unwrap(), None);
    assert_eq!(parse_jsonc("  \n\t ").unwrap(), None);
    // A comment-only file is syntactically empty, not an error.
    assert_eq!(parse_jsonc("// nothing here\n/* nor here */\n").unwrap(), None);
}

#[test]
fn test_jsonc_bom_is_stripped() {
    let node = parse_jsonc("\u{FEFF}{\"a\": 1}").unwrap().unwrap();
    assert_eq!(node.children.len(), 1);
}

#[test]
fn test_jsonc_rejects_unterminated_block_comment() {
    let err = parse_jsonc("{\"a\": 1} /* oops").unwrap_err();
    assert!(
        err.message.contains("Unterminated block comment"),
        "wrong diagnostic: {}",
        err.message
    );
}

#[test]
fn test_jsonc_rejects_genuinely_invalid_syntax() {
    // Tolerance must not extend to accepting broken documents.
    assert!(parse_jsonc("{\"a\": 1,}").is_ok(), "trailing comma is legal JSONC");
    assert!(parse_jsonc("{\"a\" 1}").is_err(), "missing colon must fail");
    assert!(parse_jsonc("{a: 1}").is_err(), "unquoted key must fail");
    assert!(parse_jsonc("{\"a\": 1").is_err(), "unclosed object must fail");
    assert!(parse_jsonc("{\"a\": 1} extra").is_err(), "trailing content must fail");
}

// ---------------------------------------------------------------------------
// Strict JSON must not inherit the tolerance
// ---------------------------------------------------------------------------

#[test]
fn test_strict_json_still_rejects_jsonc_syntax() {
    assert!(parse_json("{\"a\": 1,}").is_err());
    assert!(parse_json("{\"a\": 1 // c\n}").is_err());
    assert!(parse_json("{\"a\": 1 /* c */}").is_err());
    // ...and accepts what it should.
    assert!(parse_json("{\"a\": 1}").is_ok());
}

// ---------------------------------------------------------------------------
// JSONC: formatting and minification
// ---------------------------------------------------------------------------

#[test]
fn test_format_jsonc_preserves_every_comment_kind() {
    let src = "// top\n{\n  /* block */\n  \"a\": 1, // mid\n  /* multi\n     line */\n  \"b\": 2\n}\n// tail\n";
    let out = format_jsonc(src, 2).unwrap();
    assert!(out.contains("// top"), "top comment lost:\n{out}");
    assert!(out.contains("/* block */"), "block comment lost:\n{out}");
    assert!(out.contains("// mid"), "mid comment lost:\n{out}");
    assert!(out.contains("/* multi"), "multiline comment lost:\n{out}");
    assert!(out.contains("// tail"), "tail comment lost:\n{out}");
}

#[test]
fn test_format_jsonc_is_idempotent_and_reparsable() {
    let src = "{\n//  note\n\"a\":1,/*m*/\n\"b\":[1,2,],\n\"c\":{\"d\":1},\n}";
    let once = format_jsonc(src, 2).unwrap();
    let twice = format_jsonc(&once, 2).unwrap();
    assert_eq!(once, twice, "formatting is not idempotent");
    // Formatting must not corrupt the document it normalized.
    assert!(parse_jsonc(&once).unwrap().is_some(), "output no longer parses");
}

#[test]
fn test_format_jsonc_never_reflows_single_line_documents() {
    // Re-flowing would require discarding the comment attached to each value.
    let src = "{\"a\": 1, /* keep */ \"b\": 2}";
    let out = format_jsonc(src, 2).unwrap();
    assert_eq!(out.trim_end(), "{\"a\": 1, /* keep */ \"b\": 2}");
}

#[test]
fn test_format_jsonc_keeps_trailing_commas() {
    // Trailing commas are legal JSONC and express author intent.
    let out = format_jsonc("{\"a\": 1,}", 2).unwrap();
    assert!(out.contains("1,"), "trailing comma dropped:\n{out}");
}

#[test]
fn test_format_jsonc_normalizes_spacing_inside_lines() {
    let out = format_jsonc("{\"a\" :1,\"b\":[ 1 , 2 ]}", 2).unwrap();
    assert_eq!(out.trim_end(), "{\"a\": 1, \"b\": [1, 2]}");
}

#[test]
fn test_format_jsonc_rejects_invalid_source() {
    assert!(format_jsonc("{\"a\": }", 2).is_err());
    assert!(format_jsonc("not json at all", 2).is_err());
}

#[test]
fn test_minify_jsonc_yields_strict_json() {
    let src = "{\n  // drop me\n  \"a\": 1, /* drop me too */\n  \"b\": [1, 2,],\n}";
    let min = minify_jsonc(src).unwrap();
    assert!(!min.contains("//"), "line comment survived:\n{min}");
    assert!(!min.contains("/*"), "block comment survived:\n{min}");
    assert!(!min.contains('\n'), "newlines survived:\n{min}");
    // Round-trips through the strict parser, proving the output is valid JSON.
    let node = parse_json(&min).unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
}

#[test]
fn test_minify_jsonc_preserves_string_contents() {
    // Stripping must not touch text inside string literals.
    let min = minify_jsonc("{\"a\": \"http://x // y\", \"b\": \"/* not a comment */\"}").unwrap();
    assert!(min.contains("http://x // y"), "string damaged:\n{min}");
    assert!(min.contains("/* not a comment */"), "string damaged:\n{min}");
    assert!(parse_json(&min).is_ok());
}

#[test]
fn test_minify_jsonc_output_is_stable() {
    let src = "{ // c\n \"a\": 1, \"b\": [1,], }";
    let once = minify_jsonc(src).unwrap();
    let twice = minify_jsonc(&once).unwrap();
    assert_eq!(once, twice, "minification is not idempotent");
}

// ---------------------------------------------------------------------------
// JSON Lines: parsing
// ---------------------------------------------------------------------------

#[test]
fn test_jsonl_builds_one_record_per_line() {
    let src = "{\"id\": 1}\n{\"id\": 2}\n{\"id\": 3}\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(node.node_type, NodeType::Json(JsonType::Array));
    assert_eq!(node.children.len(), 3);
    for (i, child) in node.children.iter().enumerate() {
        assert_eq!(child.path, format!("$[{i}]"));
    }
}

#[test]
fn test_jsonl_paths_address_records_by_index() {
    let src = "{\"a\": 1}\n{\"b\": 2}\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(node.children[0].path, "$[0]");
    assert_eq!(node.children[1].path, "$[1]");
    assert_eq!(node.children[0].children[0].path, "$[0].a");
}

#[test]
fn test_jsonl_spans_are_absolute_to_the_document() {
    // Each record is parsed in isolation, so its spans must be offset by the
    // record's position or cursor-to-tree sync would select the wrong record.
    let src = "{\"id\": 1}\n{\"id\": 22}\n{\"id\": 333}\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(&src[node.children[0].span.start_byte..node.children[0].span.end_byte], "{\"id\": 1}");
    assert_eq!(&src[node.children[1].span.start_byte..node.children[1].span.end_byte], "{\"id\": 22}");
    assert_eq!(&src[node.children[2].span.start_byte..node.children[2].span.end_byte], "{\"id\": 333}");
    assert_eq!(node.children[0].span.start_line, 1);
    assert_eq!(node.children[1].span.start_line, 2);
    assert_eq!(node.children[2].span.start_line, 3);
}

#[test]
fn test_jsonl_accepts_mixed_value_kinds_per_line() {
    let src = "{\"a\":1}\n[1,2]\n\"str\"\n42\ntrue\nnull\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(node.children.len(), 6);
    assert_eq!(node.children[0].node_type, NodeType::Json(JsonType::Object));
    assert_eq!(node.children[1].node_type, NodeType::Json(JsonType::Array));
    assert_eq!(node.children[2].node_type, NodeType::Json(JsonType::String));
    assert_eq!(node.children[3].node_type, NodeType::Json(JsonType::Number));
    assert_eq!(node.children[4].node_type, NodeType::Json(JsonType::Boolean));
    assert_eq!(node.children[5].node_type, NodeType::Json(JsonType::Null));
}

#[test]
fn test_jsonl_skips_blank_lines_and_trailing_newline() {
    let src = "{\"a\":1}\n\n   \n{\"b\":2}\n\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(node.children.len(), 2, "blank lines became records");
}

#[test]
fn test_jsonl_accepts_crlf_line_endings() {
    let src = "{\"a\":1}\r\n{\"b\":2}\r\n";
    let node = parse_jsonl(src).unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
    assert_eq!(&src[node.children[0].span.start_byte..node.children[0].span.end_byte], "{\"a\":1}");
}

#[test]
fn test_jsonl_handles_bom_and_empty_input() {
    assert_eq!(parse_jsonl("").unwrap(), None);
    assert_eq!(parse_jsonl("  \n\n ").unwrap(), None);
    let node = parse_jsonl("\u{FEFF}{\"a\":1}\n{\"b\":2}\n").unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
}

#[test]
fn test_jsonl_rejects_multiple_values_on_one_line() {
    // One record per line is the defining constraint of the format.
    let err = parse_jsonl("{\"a\":1} {\"b\":2}\n").unwrap_err();
    assert!(
        err.message.contains("one complete value per line"),
        "wrong diagnostic: {}",
        err.message
    );
}

#[test]
fn test_jsonl_rejects_unterminated_record() {
    assert!(parse_jsonl("{\"a\":1\n{\"b\":2}\n").is_err());
}

#[test]
fn test_jsonl_error_reports_the_offending_line() {
    let err = parse_jsonl("{\"a\":1}\n{\"b\":2}\n{oops}\n").unwrap_err();
    assert_eq!(err.line, 3, "diagnostic points at the wrong line");
}

#[test]
fn test_jsonl_does_not_accept_jsonc_syntax() {
    // A `//` comment would swallow the rest of the line and break the
    // one-value-per-line rule, so JSONC tolerance must not leak in.
    assert!(parse_jsonl("{\"a\":1} // note\n").is_err());
    assert!(parse_jsonl("{\"a\":1,}\n").is_err());
}

#[test]
fn test_jsonl_formatted_output_still_parses() {
    let src = "  {\"a\" : 1}  \n\n{\"b\":[1,  2]}\n";
    let out = format_jsonl(src).unwrap();
    let node = parse_jsonl(&out).unwrap().unwrap();
    assert_eq!(node.children.len(), 2);
}

#[test]
fn test_minify_jsonl_keeps_one_record_per_line() {
    let src = " {\"a\" : 1} \n\n{\"b\":[1,  2]}\n";
    let min = minify_jsonl(src).unwrap();
    assert_eq!(min.lines().count(), 2, "record count changed:\n{min}");
    assert_eq!(min.lines().next().unwrap(), "{\"a\":1}");
    assert_eq!(min.lines().nth(1).unwrap(), "{\"b\":[1,2]}");
}

#[test]
fn test_minify_jsonl_is_stable() {
    let src = " {\"a\" : 1} \n{\"b\":[1,  2]}\n";
    let once = minify_jsonl(src).unwrap();
    assert_eq!(minify_jsonl(&once).unwrap(), once);
}

// ---------------------------------------------------------------------------
// Byte-order marks
//
// The parser skips a leading BOM. Every formatter entry point must skip it too,
// or the BOM is emitted as content -- or as a zero-width line of its own, which
// silently corrupts the output.
// ---------------------------------------------------------------------------

#[test]
fn test_bom_is_stripped_by_every_formatter() {
    let jsonc = "\u{FEFF}{// c\n\"a\": 1}";
    let jsonl = "\u{FEFF}{\"a\":1}\n{\"b\":2}\n";

    assert_eq!(
        format_jsonc(jsonc, 2).unwrap(),
        format_jsonc("{// c\n\"a\": 1}", 2).unwrap()
    );
    // Minify normalizes intra-line spacing, so compare against the no-BOM
    // result rather than a hand-written literal.
    assert_eq!(
        minify_jsonc(jsonc).unwrap(),
        minify_jsonc("{// c\n\"a\": 1}").unwrap()
    );
    assert_eq!(format_jsonl(jsonl).unwrap(), "{\"a\":1}\n{\"b\":2}\n");
    assert_eq!(minify_jsonl(jsonl).unwrap(), "{\"a\":1}\n{\"b\":2}\n");

    // Nothing that comes back may still carry the mark.
    for out in [
        format_jsonc(jsonc, 2).unwrap(),
        minify_jsonc(jsonc).unwrap(),
        format_jsonl(jsonl).unwrap(),
        minify_jsonl(jsonl).unwrap(),
    ] {
        assert!(
            !out.contains('\u{FEFF}'),
            "BOM survived into output: {out:?}"
        );
    }
}

#[test]
fn test_bom_does_not_become_its_own_line() {
    // The failure mode this guards: splitting on the raw source makes the BOM a
    // line containing no visible text, which yields a blank first line.
    let out = format_jsonc("\u{FEFF}{// c\n\"a\": 1}", 2).unwrap();
    assert_eq!(
        out.lines().next(),
        Some("{ // c"),
        "BOM became a leading blank line: {out:?}"
    );
}

// ---------------------------------------------------------------------------
// Tree integration: the whole point of the shared model
// ---------------------------------------------------------------------------

#[test]
fn test_tree_state_indexes_jsonc_nodes() {
    let src = "{\n  // note\n  \"alpha\": {\"x\": 1},\n  \"beta\": [1, 2]\n}";
    let root = parse_jsonc(src).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root.clone()));
    tree.expand_all();

    // Expansion must not be blocked by the comment line above "alpha".
    assert!(tree.cached_rows().len() > root.children.len());

    // Offset lookup resolves through comments to the leaf it actually covers.
    let leaf = &root.children[0].children[0];
    assert_eq!(leaf.key.as_deref(), Some("x"));
    let found = root.find_node_at_offset(leaf.span.start_byte).unwrap();
    assert_eq!(found.path, leaf.path);
}

#[test]
fn test_jsonc_editor_tree_sync_uses_absolute_spans() {
    let src = "// header comment\n{\n  \"service\": \"auth\",\n  \"port\": 8080\n}\n";
    let mut buffer = TextBuffer::new(src);
    let root = parse_jsonc(src).unwrap().unwrap();

    // Tree-to-editor: selecting the port must highlight exactly `8080`, which
    // is only true if spans are absolute to the whole document rather than to
    // the object.
    let port = &root.children[1];
    assert_eq!(port.key.as_deref(), Some("port"));
    buffer.set_selection(port.span.start_byte, port.span.end_byte);
    let (start, end) = buffer.selection().unwrap();
    assert_eq!(&buffer.text()[start..end], "8080");

    // Editor-to-tree: a cursor inside the comment text must not resolve to a
    // bogus node, while one inside the value must.
    let inside = src.find("8080").unwrap() + 2;
    assert_eq!(root.find_node_at_offset(inside).unwrap().key.as_deref(), Some("port"));
}

#[test]
fn test_tree_state_indexes_jsonl_records() {
    let src = "{\"id\": 1, \"n\": \"a\"}\n{\"id\": 2, \"n\": \"bb\"}\n{\"id\": 3, \"n\": \"ccc\"}\n";
    let root = parse_jsonl(src).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root.clone()));
    tree.expand_all();

    assert_eq!(root.children.len(), 3);
    for (i, record) in root.children.iter().enumerate() {
        assert_eq!(record.path, format!("$[{i}]"));
    }

    // Each record's span must point at its own line, in ascending order.
    for w in root.children.windows(2) {
        assert!(
            w[0].span.start_byte < w[1].span.start_byte,
            "record spans are not ordered"
        );
    }
    assert_eq!(&src[root.children[2].span.start_byte..root.children[2].span.end_byte], "{\"id\": 3, \"n\": \"ccc\"}");
}

#[test]
fn test_jsonl_editor_tree_sync_selects_the_right_record() {
    let src = "{\"id\": 1}\n{\"id\": 2}\n{\"id\": 3}\n";
    let mut buffer = TextBuffer::new(src);
    let root = parse_jsonl(src).unwrap().unwrap();

    // A cursor in the third line must select the third record, which only holds
    // if each record's spans were offset by its position in the document.
    let inside = src.rfind("3}").unwrap();
    let found = root.find_node_at_offset(inside).unwrap();
    // The offset sits on the `id` value, so the innermost node is the more
    // precise answer; what matters is that it belongs to the third record
    // rather than to a record parsed at the wrong byte offset.
    assert_eq!(found.path, "$[2].id");

    // And tree-to-editor round-trips back to the same text, using the record's
    // own span rather than the nested leaf's.
    let record = &root.children[2];
    buffer.set_selection(record.span.start_byte, record.span.end_byte);
    let (start, end) = buffer.selection().unwrap();
    assert_eq!(&buffer.text()[start..end], "{\"id\": 3}");
}

#[test]
fn test_tree_filter_matches_nested_keys_in_both_dialects() {
    let jsonc = "{\n // note\n \"outer\": {\"needle\": 1, \"other\": 2}\n}";
    let root = parse_jsonc(jsonc).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root.clone()));
    tree.expand_all();
    tree.set_filter_query("needle");
    let visible = tree.visible_nodes();
    assert!(!visible.is_empty(), "filter matched nothing in JSONC");
    assert!(
        visible.iter().any(|(n, _)| n.key.as_deref() == Some("needle")),
        "matching key not visible"
    );

    let jsonl = "{\"id\":1,\"needle\":\"a\"}\n{\"id\":2,\"needle\":\"b\"}\n";
    let root = parse_jsonl(jsonl).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));
    tree.expand_all();
    tree.set_filter_query("needle");
    assert!(!tree.visible_nodes().is_empty(), "filter matched nothing in JSONL");
}

#[test]
fn test_symbol_lookup_works_for_jsonl_records() {
    let src = "{\"id\": 1}\n{\"id\": 2}\n";
    let root = parse_jsonl(src).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));
    tree.expand_all();

    // Record-level symbols are what jump-to-definition navigates between.
    let inside = src.rfind("2}").unwrap();
    assert!(
        tree.find_symbol_at_offset(inside).is_some(),
        "no symbol found for record 2"
    );
}

// ---------------------------------------------------------------------------

#[test]
fn test_bom_jsonl_spans_index_the_original_buffer() {
    // The parser works on BOM-stripped text, so its spans must be shifted back
    // out to absolute offsets -- otherwise every span is short by the BOM length
    // and the tree view highlights the wrong text.
    let raw = "\u{FEFF}{\"a\":1}\n{\"b\":2}\n";
    let root = parse_jsonl(raw).unwrap().expect("two records");
    assert_eq!(root.children.len(), 2);

    for (i, record) in root.children.iter().enumerate() {
        let line = raw.lines().nth(i).expect("line");
        let expected = line.trim_start_matches('\u{FEFF}');
        assert_eq!(
            &raw[record.span.start_byte..record.span.end_byte],
            expected,
            "record {i} span does not match its line"
        );
    }

    // The synthetic root covers the whole original document, BOM included.
    assert_eq!(root.span.start_byte, 0);
    assert_eq!(root.span.end_byte, raw.len());
}
