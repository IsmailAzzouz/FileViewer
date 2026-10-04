use file_viewer::formats::node::{NodeType, TreeNode};
use file_viewer::formats::yaml::{format_yaml, minify_yaml, parse_yaml, YamlType};
use file_viewer::tree::TreeState;

/// Finds a node by its path, panicking with the available paths if missing.
fn find_by_path<'a>(root: &'a TreeNode, path: &str) -> &'a TreeNode {
    fn search<'a>(node: &'a TreeNode, path: &str) -> Option<&'a TreeNode> {
        if node.path == path {
            return Some(node);
        }
        node.children.iter().find_map(|c| search(c, path))
    }

    search(root, path).unwrap_or_else(|| {
        let mut available = Vec::new();
        collect_paths(root, &mut available);
        panic!("path {path} not found; available: {available:?}");
    })
}

fn collect_paths(node: &TreeNode, out: &mut Vec<String>) {
    out.push(node.path.clone());
    for child in &node.children {
        collect_paths(child, out);
    }
}

/// Asserts the node type, panicking with the actual value on mismatch.
fn assert_type(node: &TreeNode, expected: YamlType) {
    match node.node_type {
        NodeType::Yaml(actual) => assert_eq!(actual, expected, "unexpected type at {}", node.path),
        other => panic!("expected YAML type, got {other:?} at {}", node.path),
    }
}

/// Asserts a span covers exactly `expected` text in `source`.
///
/// This is the check that catches column arithmetic drifting: a span that is
/// off by a space still parses, but points the tree at the wrong text.
fn assert_span_text(source: &str, node: &TreeNode, expected: &str) {
    let actual = &source[node.span.start_byte..node.span.end_byte];
    assert_eq!(
        actual, expected,
        "span text mismatch for {} at bytes {}..{}",
        node.path,
        node.span.start_byte,
        node.span.end_byte
    );
}

#[test]
fn test_yaml_scalar_inference() {
    let source = r#"
name: FileViewer
count: 42
negative: -7
ratio: 0.5
exponent: 1.5e3
hex: 0x1F
enabled: true
disabled: no
missing: null
tilde: ~
blank:
quoted: "123"
"#;

    let root = parse_yaml(source).unwrap().unwrap();
    assert_type(&root, YamlType::Mapping);

    assert_type(find_by_path(&root, "$.name"), YamlType::String);
    assert_type(find_by_path(&root, "$.count"), YamlType::Integer);
    assert_type(find_by_path(&root, "$.negative"), YamlType::Integer);
    assert_type(find_by_path(&root, "$.ratio"), YamlType::Float);
    assert_type(find_by_path(&root, "$.exponent"), YamlType::Float);
    assert_type(find_by_path(&root, "$.hex"), YamlType::Integer);
    assert_type(find_by_path(&root, "$.enabled"), YamlType::Boolean);
    assert_type(find_by_path(&root, "$.disabled"), YamlType::Boolean);
    assert_type(find_by_path(&root, "$.missing"), YamlType::Null);
    assert_type(find_by_path(&root, "$.tilde"), YamlType::Null);
    assert_type(find_by_path(&root, "$.blank"), YamlType::Null);
    // A quoted number stays a string: quoting is the author's override.
    assert_type(find_by_path(&root, "$.quoted"), YamlType::String);
}

#[test]
fn test_yaml_scalar_spans_are_exact() {
    let source = "name: FileViewer\ncount: 42\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let name = find_by_path(&root, "$.name");
    assert_span_text(source, name, "FileViewer");
    assert_eq!(name.span.start_line, 1);
    assert_eq!(name.span.start_col, 7);

    let count = find_by_path(&root, "$.count");
    assert_span_text(source, count, "42");
    assert_eq!(count.span.start_line, 2);
    assert_eq!(count.span.start_col, 8);

    // The key span stops before the value so tree selection lands on the key.
    let key_span = count.key_span.expect("key span");
    assert_eq!(&source[key_span.start_byte..key_span.end_byte], "count");
}

#[test]
fn test_yaml_quoted_scalar_spans_and_escapes() {
    let source = "a: \"line\\nbreak\"\nb: 'it''s here'\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let a = find_by_path(&root, "$.a");
    // The span covers the quotes, while the preview shows the decoded value.
    assert_span_text(source, a, "\"line\\nbreak\"");
    assert_eq!(a.value_preview, "line\\nbreak");

    let b = find_by_path(&root, "$.b");
    assert_span_text(source, b, "'it''s here'");
    assert_eq!(b.value_preview, "it's here");
}

#[test]
fn test_yaml_nested_mappings_and_paths() {
    let source = r#"
metadata:
  name: file-viewer
  labels:
    app: viewer
    tier: backend
"#;
    let root = parse_yaml(source).unwrap().unwrap();
    assert_type(&root, YamlType::Mapping);
    assert_type(find_by_path(&root, "$.metadata"), YamlType::Mapping);
    assert_type(find_by_path(&root, "$.metadata.labels"), YamlType::Mapping);
    assert_eq!(
        find_by_path(&root, "$.metadata.labels.tier").value_preview,
        "backend"
    );
}

#[test]
fn test_yaml_sequences_and_indices() {
    let source = r#"
items:
  - first
  - second
mixed:
- a
- b
"#;
    let root = parse_yaml(source).unwrap().unwrap();

    let items = find_by_path(&root, "$.items");
    assert_type(items, YamlType::Sequence);
    assert_eq!(items.children.len(), 2);
    assert_eq!(items.value_preview, "[ 2 items ]");
    assert_eq!(find_by_path(&root, "$.items[0]").value_preview, "first");
    assert_eq!(find_by_path(&root, "$.items[1]").value_preview, "second");

    // A sequence may sit at the same column as its key.
    let mixed = find_by_path(&root, "$.mixed");
    assert_type(mixed, YamlType::Sequence);
    assert_eq!(mixed.children.len(), 2);
    assert_eq!(find_by_path(&root, "$.mixed[1]").value_preview, "b");
}

#[test]
fn test_yaml_sequence_of_mappings() {
    let source = r#"
containers:
  - name: app
    image: app:1.0
    ports:
      - 8080
      - 9090
"#;
    let root = parse_yaml(source).unwrap().unwrap();

    let first = find_by_path(&root, "$.containers[0]");
    assert_type(first, YamlType::Mapping);
    // name, image, and ports are all keys of the same mapping.
    assert_eq!(first.children.len(), 3);
    assert_eq!(find_by_path(&root, "$.containers[0].image").value_preview, "app:1.0");
    assert_type(find_by_path(&root, "$.containers[0].ports"), YamlType::Sequence);
    assert_eq!(find_by_path(&root, "$.containers[0].ports[1]").value_preview, "9090");
}

#[test]
fn test_yaml_sequence_item_spans() {
    let source = "items:\n  - first\n  - second\n";
    let root = parse_yaml(source).unwrap().unwrap();
    let items = find_by_path(&root, "$.items");

    assert_span_text(source, &items.children[0], "first");
    assert_span_text(source, &items.children[1], "second");
    // The sequence span runs from the first dash to the last value.
    assert_eq!(
        &source[items.span.start_byte..items.span.end_byte],
        "- first\n  - second"
    );
}

#[test]
fn test_yaml_flow_collections() {
    let source = r#"
flow_map: {a: 1, b: two}
flow_seq: [1, 2, 3]
nested: {outer: {inner: [x, y]}}
"#;
    let root = parse_yaml(source).unwrap().unwrap();

    let flow_map = find_by_path(&root, "$.flow_map");
    assert_type(flow_map, YamlType::Mapping);
    assert_span_text(source, flow_map, "{a: 1, b: two}");
    assert_eq!(find_by_path(&root, "$.flow_map.b").value_preview, "two");

    let flow_seq = find_by_path(&root, "$.flow_seq");
    assert_type(flow_seq, YamlType::Sequence);
    assert_span_text(source, flow_seq, "[1, 2, 3]");
    assert_eq!(find_by_path(&root, "$.flow_seq[2]").value_preview, "3");

    assert_eq!(
        find_by_path(&root, "$.nested.outer.inner[1]").value_preview,
        "y"
    );
}

#[test]
fn test_yaml_block_scalar_literal() {
    let source = "script: |\n  line one\n  line two\nafter: done\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let script = find_by_path(&root, "$.script");
    assert_type(script, YamlType::String);
    // Literal style keeps the newline between lines.
    assert_eq!(script.value_preview, "line one\\nline two\\n");
    // The span covers the header and the whole body.
    assert_eq!(
        &source[script.span.start_byte..script.span.end_byte],
        "|\n  line one\n  line two"
    );
    // Parsing resumes correctly after the block scalar.
    assert_eq!(find_by_path(&root, "$.after").value_preview, "done");
}

/// A comment at the parent's indentation ends a block scalar; one indented
/// with the body is literal text. `build_lines` marks both as skippable, so
/// this guards against the outer comment being swallowed into the value.
#[test]
fn test_yaml_comment_after_block_scalar_ends_the_body() {
    let source =
        "body: |\n  line one\n  # indented, so part of the body\n  line two\n# outer\nafter: done\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let body = find_by_path(&root, "$.body");
    assert!(
        body.value_preview.contains("# indented"),
        "in-body comment was dropped: {:?}",
        body.value_preview
    );
    // The span must stop before the outer comment, or the following section
    // gets pulled into the scalar.
    let sliced = &source[body.span.start_byte..body.span.end_byte];
    assert!(
        !sliced.contains("# outer"),
        "span swallowed the outer comment: {sliced:?}"
    );
    // Parsing resumes on the next real key.
    assert_eq!(find_by_path(&root, "$.after").value_preview, "done");
}

#[test]
fn test_yaml_block_scalar_chomping() {
    // `-` strips the trailing newline, the default clips to one.
    let source = "strip: |-\n  a\nclip: |\n  b\nkeep: |+\n  c\n\n\nnext: 1\n";
    let root = parse_yaml(source).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.strip").value_preview, "a");
    assert_eq!(find_by_path(&root, "$.clip").value_preview, "b\\n");
    // `+` keeps the trailing newlines that follow the body.
    assert_eq!(find_by_path(&root, "$.keep").value_preview, "c\\n\\n\\n");
    // Parsing resumes after the scalar regardless of chomping.
    assert_eq!(find_by_path(&root, "$.next").value_preview, "1");
}

#[test]
fn test_yaml_block_scalar_folded() {
    let source = "text: >-\n  folded line\n  continues here\nafter: 1\n";
    let root = parse_yaml(source).unwrap().unwrap();

    // Folded style joins lines with a space.
    assert_eq!(
        find_by_path(&root, "$.text").value_preview,
        "folded line continues here"
    );
    assert_eq!(find_by_path(&root, "$.after").value_preview, "1");
}

#[test]
fn test_yaml_alias_expands_to_anchor() {
    let source = r#"
defaults: &defaults
  adapter: postgres
  host: localhost
development:
  <<: *defaults
  host: dev.local
"#;
    let root = parse_yaml(source).unwrap().unwrap();

    // The anchor is registered under its own path.
    let defaults = find_by_path(&root, "$.defaults");
    assert_type(defaults, YamlType::Mapping);
    assert_eq!(find_by_path(&root, "$.defaults.adapter").value_preview, "postgres");

    // The alias expands to a clone of the anchored mapping, so the merged
    // keys are visible.
    let development = find_by_path(&root, "$.development");
    assert_type(development, YamlType::Mapping);
    assert_eq!(
        find_by_path(&root, "$.development.adapter").value_preview,
        "postgres"
    );
    // The local key wins because the merge key itself is left unexpanded.
    assert_eq!(find_by_path(&root, "$.development.host").value_preview, "dev.local");
}

#[test]
fn test_yaml_alias_of_scalar() {
    let source = "base: &v FileViewer\ncopy: *v\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let copy = find_by_path(&root, "$.copy");
    assert_type(copy, YamlType::String);
    assert_eq!(copy.value_preview, "FileViewer");
    // The alias token itself owns the span.
    assert_span_text(source, copy, "*v");
}

#[test]
fn test_yaml_unknown_alias_is_a_diagnostic() {
    let source = "a: *missing\n";
    let err = parse_yaml(source).unwrap_err();
    assert!(
        err.message.contains("Unknown alias"),
        "unexpected message: {}",
        err.message
    );
    assert_eq!(err.line, 1);
}

/// A "billion laughs" document expands to far more nodes than it contains
/// characters. Expansion has to be bounded, or a few hundred bytes of input
/// exhausts memory instead of producing a diagnostic.
#[test]
fn test_yaml_alias_expansion_bomb_is_bounded() {
    let mut source = String::from("a: &a [\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]\n");
    let mut previous = 'a';
    for name in ['b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'] {
        let refs: String = std::iter::repeat_n(format!("*{previous}"), 9).collect::<Vec<_>>().join(",");
        source.push_str(&format!("{name}: &{name} [{refs}]\n"));
        previous = name;
    }

    let err = parse_yaml(&source).unwrap_err();
    assert!(
        err.message.contains("budget"),
        "expected the expansion budget to stop the parse, got: {}",
        err.message
    );
}

/// A merge key expands entries, so it can amplify the same way an alias does.
#[test]
fn test_yaml_merge_key_expansion_bomb_is_bounded() {
    let mut source = String::from("a: &a [\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\",\"x\"]\n");
    let mut previous = 'a';
    for name in ['b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j'] {
        let refs: String = std::iter::repeat_n(format!("*{previous}"), 9).collect::<Vec<_>>().join(",");
        source.push_str(&format!("{name}: &{name} [{refs}]\n"));
        previous = name;
    }
    source.push_str("top:\n  <<: *j\n");

    let err = parse_yaml(&source).unwrap_err();
    assert!(
        err.message.contains("budget"),
        "expected the expansion budget to stop the parse, got: {}",
        err.message
    );
}

#[test]
fn test_yaml_tag_recorded_in_preview() {
    let source = "a: !!str 123\nb: 123\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let a = find_by_path(&root, "$.a");
    assert_eq!(a.value_preview, "!!str 123");
    // The span includes the tag, since the tag is part of the value.
    assert_span_text(source, a, "!!str 123");
    // The tag does not change the inferred type.
    assert_type(find_by_path(&root, "$.b"), YamlType::Integer);
}

#[test]
fn test_yaml_multi_document_stream() {
    let source = r#"
---
name: first
---
name: second
"#;
    let root = parse_yaml(source).unwrap().unwrap();

    // A stream becomes a synthetic root sequence, like JSON Lines.
    assert_type(&root, YamlType::Sequence);
    assert_eq!(root.children.len(), 2);
    assert_eq!(root.value_preview, "[ 2 documents ]");
    assert_eq!(find_by_path(&root, "$[0].name").value_preview, "first");
    assert_eq!(find_by_path(&root, "$[1].name").value_preview, "second");
}

#[test]
fn test_yaml_single_document_with_marker() {
    let source = "---\nname: only\n";
    let root = parse_yaml(source).unwrap().unwrap();
    // One document is unwrapped, not reported as a one-element stream.
    assert_type(&root, YamlType::Mapping);
    assert_eq!(find_by_path(&root, "$.name").value_preview, "only");
}

#[test]
fn test_yaml_document_end_marker() {
    let source = "name: first\n...\nname: second\n";
    let root = parse_yaml(source).unwrap().unwrap();
    // `...` ends a document, so this is a two-document stream.
    assert_eq!(root.children.len(), 2);
    assert_eq!(find_by_path(&root, "$[0].name").value_preview, "first");
    assert_eq!(find_by_path(&root, "$[1].name").value_preview, "second");
}

#[test]
fn test_yaml_comments_and_blank_lines_are_trivia() {
    let source = "# leading comment\n\nname: viewer  # trailing comment\n\n# tail\n";
    let root = parse_yaml(source).unwrap().unwrap();

    assert_eq!(root.children.len(), 1);
    let name = find_by_path(&root, "$.name");
    assert_eq!(name.value_preview, "viewer");
    // The trailing comment is not part of the value span.
    assert_span_text(source, name, "viewer");
}

#[test]
fn test_yaml_hash_inside_quoted_scalar_is_not_a_comment() {
    let source = "url: \"http://example.com/#anchor\"\ntag: plain#nothash\n";
    let root = parse_yaml(source).unwrap().unwrap();
    assert_eq!(
        find_by_path(&root, "$.url").value_preview,
        "http://example.com/#anchor"
    );
    // A `#` with no preceding space is part of a plain scalar.
    assert_eq!(find_by_path(&root, "$.tag").value_preview, "plain#nothash");
}

#[test]
fn test_yaml_empty_and_comment_only_documents() {
    assert!(parse_yaml("").unwrap().is_none());
    assert!(parse_yaml("   \n\n  ").unwrap().is_none());
    assert!(parse_yaml("# only a comment\n").unwrap().is_none());
}

#[test]
fn test_yaml_bom_is_stripped() {
    let source = "\u{FEFF}name: viewer\n";
    let root = parse_yaml(source).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.name").value_preview, "viewer");
    // Spans stay absolute to the original buffer, so they index the BOM.
    let name = find_by_path(&root, "$.name");
    assert_eq!(
        &source[name.span.start_byte..name.span.end_byte],
        "viewer"
    );
}

const BOM: &str = "\u{FEFF}";

/// Flattens the tree so two parses can be compared node for node.
fn flatten(node: &TreeNode, out: &mut Vec<TreeNode>) {
    out.push(node.clone());
    for child in &node.children {
        flatten(child, out);
    }
}

fn spans_of(source: &str) -> Vec<TreeNode> {
    let root = parse_yaml(source).unwrap().unwrap();
    let mut out = Vec::new();
    flatten(&root, &mut out);
    out
}

/// A BOM must not disturb anything but the byte offsets. Node order, paths,
/// previews, line numbers, and column numbers must all match the BOM-less
/// parse exactly, with every non-root offset shifted by the BOM width. This
/// catches the whole class of bugs where a shifted offset is fed back into the
/// BOM-stripped line table.
#[test]
fn test_yaml_bom_spans_match_the_bomless_parse() {
    let fixtures: &[&str] = &[
        "name: viewer\ncount: 3\nok: true\n",
        "base:\n  x: 1\n  deep:\n    y: [1, 2, 3]\nuse: 2\n",
        "items:\n  - one\n  - two\n  - key: v\n    other: w\ntail: end\n",
        "a: {x: 1, y: [2, 3]}\nb: [{k: 1}, {k: 2}]\nc: [q: 1]\n",
        "s: |\n  body\n  more\nafter: done\n",
        "base: &b\n  x: 1\nuse: *b\nafter: 2\n",
        "v: &v [1, 2]\nw: *v\n",
        "a: \"he said \\\"hi\\\"\"\nb: 'it''s'\n",
        "---\na: 1\n---\nb: 2\n...\n",
        "# leading\na: 1 # trailing\n# between\nb: 2\n",
        "a:\n- 1\n- 2\nb: 3\n",
        "a:\n  b:\n    c:\n      d: 1\n",
        "url: http://example.com:8080/x\n",
    ];

    for fixture in fixtures {
        let plain = spans_of(fixture);
        let bom_source = format!("{BOM}{fixture}");
        let shifted = spans_of(&bom_source);

        assert_eq!(
            plain.len(),
            shifted.len(),
            "node count changed for {fixture:?}"
        );

        for (a, b) in plain.iter().zip(shifted.iter()) {
            assert_eq!(a.path, b.path, "path changed for {fixture:?}");
            assert_eq!(
                a.value_preview, b.value_preview,
                "preview changed for {} in {fixture:?}",
                a.path
            );
            // The root covers the whole buffer, BOM included, so it starts at 0
            // and ends at the full length in both parses. Every other node is
            // shifted by the BOM width.
            let (want_start, want_end) = if a.path == "$" {
                (0, bom_source.len())
            } else {
                (a.span.start_byte + 3, a.span.end_byte + 3)
            };
            assert_eq!(
                (b.span.start_byte, b.span.end_byte),
                (want_start, want_end),
                "byte span shifted incorrectly for {} in {fixture:?}",
                a.path
            );
            assert_eq!(
                (
                    b.span.start_line,
                    b.span.start_col,
                    b.span.end_line,
                    b.span.end_col
                ),
                (
                    a.span.start_line,
                    a.span.start_col,
                    a.span.end_line,
                    a.span.end_col
                ),
                "line/col changed for {} in {fixture:?}",
                a.path
            );
        }
    }
}

#[test]
fn test_yaml_bom_spans_slice_the_original_buffer() {
    let source = "\u{FEFF}base: &b\n  x: 1\nuse: *b\nafter: 2\n";
    let root = parse_yaml(source).unwrap().unwrap();

    // Every span must slice the intended text straight out of the original
    // buffer, BOM included.
    assert_span_text(source, find_by_path(&root, "$.base.x"), "1");
    assert_span_text(source, find_by_path(&root, "$.use"), "*b");
    assert_span_text(source, find_by_path(&root, "$.after"), "2");
    assert!(
        find_by_path(&root, "$.base").span.start_byte
            < find_by_path(&root, "$.use").span.start_byte
    );
}

#[test]
fn test_yaml_bom_diagnostic_offsets_index_the_original_buffer() {
    let source = "\u{FEFF}a: 1\n  b: 2\n";
    let err = parse_yaml(source).unwrap_err();
    assert_eq!(err.line, 2);
    assert!(
        err.byte_offset <= source.len(),
        "byte offset {} past end of buffer {}",
        err.byte_offset,
        source.len()
    );
    assert!(
        source.is_char_boundary(err.byte_offset),
        "byte offset does not land on a character boundary"
    );
    // The snippet is rendered from the untouched buffer, so the BOM is present
    // in the text the user sees.
    assert!(
        source[err.byte_offset..].starts_with("b: 2"),
        "byte offset {} does not point at the offending text: {:?}",
        err.byte_offset,
        &source[err.byte_offset..]
    );
}

#[test]
fn test_yaml_unicode_spans_are_aligned() {
    let source = "naïve: café ☕\n";
    let root = parse_yaml(source).unwrap().unwrap();
    let value = find_by_path(&root, "$.naïve");
    assert_eq!(value.value_preview, "café ☕");
    // Byte offsets must land on character boundaries, not split a code point.
    assert_span_text(source, value, "café ☕");
    // Columns count characters, not bytes.
    // "naïve: " is 7 characters, so the value starts in column 8.
    assert_eq!(value.span.start_col, 8);
}

#[test]
fn test_yaml_plain_scalar_continuation_folds() {
    let source = "description: this is a long\n  value that wraps\nnext: 1\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let description = find_by_path(&root, "$.description");
    assert_eq!(description.value_preview, "this is a long value that wraps");
    // The span covers both the first line and the folded continuation.
    assert_eq!(
        &source[description.span.start_byte..description.span.end_byte],
        "this is a long\n  value that wraps"
    );
    assert_eq!(find_by_path(&root, "$.next").value_preview, "1");
}

#[test]
fn test_yaml_indentation_error_is_reported() {
    let source = "a: 1\n  b: 2\n";
    let err = parse_yaml(source).unwrap_err();
    assert_eq!(err.line, 2);
    assert!(
        err.message.contains("indentation"),
        "unexpected message: {}",
        err.message
    );
}

#[test]
fn test_yaml_complex_key_is_rejected() {
    let source = "? complex\n: value\n";
    let err = parse_yaml(source).unwrap_err();
    assert!(
        err.message.contains("complex keys"),
        "unexpected message: {}",
        err.message
    );
}

#[test]
fn test_yaml_deep_nesting_is_rejected() {
    // Build a document nested well past the depth limit.
    let mut source = String::new();
    for level in 0..200 {
        // Each level must indent further, or every `a:` is a sibling key
        // rather than a nested one.
        for _ in 0..level {
            source.push(' ');
        }
        source.push_str("a:\n");
    }
    source.push_str("    leaf: 1\n");

    let err = parse_yaml(&source).unwrap_err();
    assert!(
        err.message.contains("nesting"),
        "unexpected message: {}",
        err.message
    );
}

#[test]
fn test_yaml_deep_but_legal_nesting_succeeds() {
    let mut source = String::new();
    for i in 0..40 {
        for _ in 0..i {
            source.push_str("  ");
        }
        source.push_str("a:\n");
    }
    let root = parse_yaml(&source).unwrap().unwrap();
    assert!(root.total_node_count() >= 40);
}

#[test]
fn test_yaml_find_node_at_offset_selects_deepest_node() {
    let source = "root:\n  child: value\n";
    let root = parse_yaml(source).unwrap().unwrap();

    // The offset of `value` must resolve to the leaf, not its parent.
    let offset = source.find("value").expect("value in source");
    let found = root.find_node_at_offset(offset).expect("a node covers offset");
    assert_eq!(found.path, "$.root.child");
    assert_eq!(found.value_preview, "value");
}

#[test]
fn test_yaml_tree_state_integration() {
    let source = "name: viewer\nports:\n  - 80\n  - 443\n";
    let root = parse_yaml(source).unwrap().unwrap();

    let mut state = TreeState::default();
    state.set_root(Some(root));
    assert_eq!(state.total_node_count(), 5);

    let offset = source.find("443").expect("443 in source");
    let symbol = state
        .find_symbol_at_offset(offset)
        .expect("a symbol covers the offset");
    assert_eq!(symbol.path, "$.ports[1]");

    let root_node = state.root().expect("root is present");
    assert_eq!(find_by_path(root_node, "$.ports[1]").value_preview, "443");
}

#[test]
fn test_yaml_format_normalizes_spacing() {
    let source = "name:viewer\nitems:\n-   a\n-  b\n";
    let formatted = format_yaml(source, 2).unwrap();
    assert_eq!(formatted, "name: viewer\nitems:\n- a\n- b\n");
}

#[test]
fn test_yaml_format_preserves_comments_and_blank_lines() {
    let source = "# header\nname: viewer # note\n\n# footer\nother: 1\n";
    let formatted = format_yaml(source, 2).unwrap();
    assert!(formatted.contains("# header"));
    assert!(formatted.contains("# note"));
    assert!(formatted.contains("# footer"));
    // A blank line in the source stays a blank line.
    assert!(formatted.contains("viewer # note\n\n# footer"));
}

#[test]
fn test_yaml_format_preserves_block_scalar_body() {
    let source = "script: |\n    indented body\n    second line\nnext: 1\n";
    let formatted = format_yaml(source, 2).unwrap();
    // The body is value text: its leading whitespace must survive untouched.
    assert!(formatted.contains("    indented body\n"));
    assert!(formatted.contains("    second line\n"));
    assert!(formatted.contains("next: 1"));
}

#[test]
fn test_yaml_format_preserves_anchors_and_document_markers() {
    let source = "base: &b\n  x: 1\nuse: *b\n---\nsecond: doc\n";
    let formatted = format_yaml(source, 2).unwrap();
    assert!(formatted.contains("&b"));
    assert!(formatted.contains("*b"));
    assert!(formatted.contains("---"));
    assert!(formatted.contains("second: doc"));
}

#[test]
fn test_yaml_format_is_idempotent() {
    let source = "name:viewer\nlist:\n  -   a\n  -  b\nflow: {k:1,v:2}\nblock: |\n  body\n";
    let once = format_yaml(source, 2).unwrap();
    let twice = format_yaml(&once, 2).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn test_yaml_minify_strips_comments() {
    let source = "# header\nname: viewer # note\n\nother: 1\n";
    let minified = minify_yaml(source).unwrap();
    assert!(!minified.contains('#'));
    assert!(minified.contains("name: viewer"));
    assert!(minified.contains("other: 1"));
}

#[test]
fn test_yaml_minify_output_still_parses() {
    let source = "# c\nname: viewer\nitems:\n  - a\n  - b\n";
    let minified = minify_yaml(source).unwrap();
    let root = parse_yaml(&minified).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.name").value_preview, "viewer");
    assert_eq!(find_by_path(&root, "$.items[1]").value_preview, "b");
}

#[test]
fn test_yaml_minify_preserves_block_scalar_body() {
    let source = "script: |\n    keep   this\nnext: 1\n";
    let minified = minify_yaml(source).unwrap();
    assert!(
        minified.contains("keep   this"),
        "block scalar body was rewritten: {minified}"
    );
    assert!(minified.contains("next: 1"));
}

#[test]
fn test_yaml_format_rejects_invalid_document() {
    let source = "a: 1\n  b: 2\n";
    assert!(format_yaml(source, 2).is_err());
    assert!(minify_yaml(source).is_err());
}

#[test]
fn test_yaml_realistic_manifest() {
    let source = r#"# Kubernetes-style manifest
apiVersion: apps/v1
kind: Deployment
metadata:
  name: file-viewer
  labels: {app: viewer, tier: backend}
spec:
  replicas: 3
  template:
    spec:
      containers:
        - name: app
          image: "app:1.0"
          args: [--verbose, --port, "8080"]
          env:
            - name: MODE
              value: production
"#;
    let root = parse_yaml(source).unwrap().unwrap();
    assert_eq!(root.node_type, NodeType::Yaml(YamlType::Mapping));
    assert!(root.total_node_count() > 20);

    assert_eq!(find_by_path(&root, "$.kind").value_preview, "Deployment");
    assert_type(find_by_path(&root, "$.metadata.labels"), YamlType::Mapping);
    assert_eq!(find_by_path(&root, "$.spec.replicas").value_preview, "3");
    assert_eq!(
        find_by_path(&root, "$.spec.template.spec.containers[0].name").value_preview,
        "app"
    );
    assert_eq!(
        find_by_path(&root, "$.spec.template.spec.containers[0].env[0].value").value_preview,
        "production"
    );

    // Formatting must not corrupt the document.
    let formatted = format_yaml(source, 2).unwrap();
    let reparsed = parse_yaml(&formatted).unwrap().unwrap();
    assert_eq!(reparsed.total_node_count(), root.total_node_count());
}
