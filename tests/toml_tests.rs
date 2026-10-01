use file_viewer::formats::node::{NodeType, TreeNode};
use file_viewer::formats::toml::{format_toml, minify_toml, parse_toml, TomlType};
use file_viewer::tree::TreeState;

/// Finds a node by its dotted path, asserting the whole path exists.
/// Finds a node by its dotted path, panicking with context if it is missing.
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

#[test]
fn test_toml_primitives() {
    let source = r#"
name = "FileViewer"
count = 42
ratio = 0.5
enabled = true
"#;

    let root = parse_toml(source).unwrap().unwrap();

    let name = find_by_path(&root, "$.name");
    assert_eq!(name.node_type, NodeType::Toml(TomlType::String));
    assert_eq!(name.value_preview, "\"FileViewer\"");

    let count = find_by_path(&root, "$.count");
    assert_eq!(count.node_type, NodeType::Toml(TomlType::Integer));
    assert_eq!(count.value_preview, "42");

    let ratio = find_by_path(&root, "$.ratio");
    assert_eq!(ratio.node_type, NodeType::Toml(TomlType::Float));
    assert_eq!(ratio.value_preview, "0.5");

    let enabled = find_by_path(&root, "$.enabled");
    assert_eq!(enabled.node_type, NodeType::Toml(TomlType::Boolean));
    assert_eq!(enabled.value_preview, "true");
}

#[test]
fn test_toml_spans_are_exact() {
    let source = "title = \"Hello\"\ncount = 7\n";
    let root = parse_toml(source).unwrap().unwrap();

    let title = find_by_path(&root, "$.title");
    assert_eq!(&source[title.span.start_byte..title.span.end_byte], "\"Hello\"");
    assert_eq!(title.span.start_line, 1);
    assert_eq!(title.span.end_line, 1);

    let count = find_by_path(&root, "$.count");
    assert_eq!(&source[count.span.start_byte..count.span.end_byte], "7");
    assert_eq!(count.span.start_line, 2);

    // full_span covers key through value.
    let full = title.full_span();
    assert_eq!(&source[full.start_byte..full.end_byte], "title = \"Hello\"");
}

#[test]
fn test_toml_tables_and_nesting() {
    let source = r#"
[server]
host = "localhost"
port = 8080

[server.tls]
enabled = true
"#;

    let root = parse_toml(source).unwrap().unwrap();
    let server = find_by_path(&root, "$.server");
    assert_eq!(server.node_type, NodeType::Toml(TomlType::Table));

    let host = find_by_path(&root, "$.server.host");
    assert_eq!(host.value_preview, "\"localhost\"");

    let tls = find_by_path(&root, "$.server.tls");
    assert_eq!(tls.node_type, NodeType::Toml(TomlType::Table));
    assert_eq!(find_by_path(&root, "$.server.tls.enabled").value_preview, "true");
}

#[test]
fn test_toml_array_of_tables() {
    let source = r#"
[[products]]
id = 1
name = "Hammer"

[[products]]
id = 2
name = "Nail"
"#;

    let root = parse_toml(source).unwrap().unwrap();
    let products = find_by_path(&root, "$.products");
    assert_eq!(products.node_type, NodeType::Toml(TomlType::Array));
    assert_eq!(products.children.len(), 2);

    assert_eq!(find_by_path(&root, "$.products[0].name").value_preview, "\"Hammer\"");
    assert_eq!(find_by_path(&root, "$.products[1].name").value_preview, "\"Nail\"");
    assert_eq!(
        find_by_path(&root, "$.products[1].id").value_preview,
        "2"
    );
}

#[test]
fn test_toml_arrays_and_inline_tables() {
    let source = r#"
tags = ["a", "b", "c"]
point = { x = 1, y = 2 }
"#;

    let root = parse_toml(source).unwrap().unwrap();

    let tags = find_by_path(&root, "$.tags");
    assert_eq!(tags.node_type, NodeType::Toml(TomlType::Array));
    assert_eq!(tags.children.len(), 3);
    assert_eq!(find_by_path(&root, "$.tags[1]").value_preview, "\"b\"");

    let point = find_by_path(&root, "$.point");
    assert_eq!(point.node_type, NodeType::Toml(TomlType::InlineTable));
    assert_eq!(find_by_path(&root, "$.point.x").value_preview, "1");
    assert_eq!(find_by_path(&root, "$.point.y").value_preview, "2");
}

#[test]
fn test_toml_multiline_array_spans() {
    let source = "values = [\n  1,\n  2,\n]\n";
    let root = parse_toml(source).unwrap().unwrap();
    let values = find_by_path(&root, "$.values");

    assert_eq!(
        &source[values.span.start_byte..values.span.end_byte],
        "[\n  1,\n  2,\n]"
    );
    assert_eq!(find_by_path(&root, "$.values[0]").value_preview, "1");
    assert_eq!(find_by_path(&root, "$.values[1]").value_preview, "2");
}

#[test]
fn test_toml_dotted_keys_and_quote_styles() {
    let source = r#"
physical.color = "orange"
"quoted key" = 1
'literal key' = 2
"#;

    let root = parse_toml(source).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.physical.color").value_preview, "\"orange\"");
    assert_eq!(find_by_path(&root, "$.quoted key").value_preview, "1");
    assert_eq!(find_by_path(&root, "$.literal key").value_preview, "2");
}

#[test]
fn test_toml_comments_are_ignored() {
    let source = r#"
# leading comment
key = "value" # trailing comment
# another
other = 1
"#;
    let root = parse_toml(source).unwrap().unwrap();
    assert_eq!(root.children.len(), 2);
    assert_eq!(find_by_path(&root, "$.key").value_preview, "\"value\"");
    assert_eq!(find_by_path(&root, "$.other").value_preview, "1");
}

#[test]
fn test_toml_number_forms() {
    let source = r#"
hex = 0xFF
oct = 0o755
bin = 0b1010
under = 1_000
neg = -17
exp = 1.5e3
inf1 = inf
"#;
    let root = parse_toml(source).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.hex").node_type, NodeType::Toml(TomlType::Integer));
    assert_eq!(find_by_path(&root, "$.oct").node_type, NodeType::Toml(TomlType::Integer));
    assert_eq!(find_by_path(&root, "$.bin").node_type, NodeType::Toml(TomlType::Integer));
    assert_eq!(find_by_path(&root, "$.under").value_preview, "1_000");
    assert_eq!(find_by_path(&root, "$.neg").value_preview, "-17");
    assert_eq!(find_by_path(&root, "$.exp").node_type, NodeType::Toml(TomlType::Float));
    assert_eq!(find_by_path(&root, "$.inf1").node_type, NodeType::Toml(TomlType::Float));
}

#[test]
fn test_toml_datetime() {
    let source = r#"
when = 1979-05-27T07:32:00Z
day = 1979-05-27
"#;
    let root = parse_toml(source).unwrap().unwrap();
    assert_eq!(find_by_path(&root, "$.when").node_type, NodeType::Toml(TomlType::Datetime));
    assert_eq!(find_by_path(&root, "$.day").node_type, NodeType::Toml(TomlType::Datetime));
}

#[test]
fn test_toml_empty_and_comment_only() {
    assert!(parse_toml("").unwrap().is_none());
    assert!(parse_toml("   \n\t  ").unwrap().is_none());
    assert!(parse_toml("# just a comment\n").unwrap().is_none());
}

#[test]
fn test_toml_diagnostic_on_invalid_input() {
    let err = parse_toml("key = \n").unwrap_err();
    assert!(err.line >= 1);
    assert!(!err.message.is_empty());
    assert!(!err.context_snippet.is_empty());
    assert!(err.pointer.contains('^'));

    // Missing `=`.
    let err2 = parse_toml("[table]\njust_a_key\n").unwrap_err();
    assert!(err2.message.contains('='));

    // Unterminated array.
    let err3 = parse_toml("a = [1, 2\n").unwrap_err();
    assert!(!err3.message.is_empty());

    // Duplicate key.
    let err4 = parse_toml("a = 1\na = 2\n").unwrap_err();
    assert!(err4.message.contains("duplicate"));
}

#[test]
fn test_toml_bom_and_multiline_strings() {
    let source = "\u{FEFF}text = \"\"\"\nhello\nworld\"\"\"\n";
    let root = parse_toml(source).unwrap().unwrap();
    let text = find_by_path(&root, "$.text");
    assert_eq!(text.node_type, NodeType::Toml(TomlType::String));
    assert!(text.value_preview.contains("hello"));
}

#[test]
fn test_toml_tree_integration() {
    let source = r#"
[server]
host = "localhost"
port = 8080

[[hosts]]
name = "a"
"#;

    let root = parse_toml(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));

    // root + server + host + port + hosts(array) + 1 element + name = 7
    assert_eq!(tree.total_node_count(), 7);

    tree.collapse_all();
    assert_eq!(tree.visible_nodes().len(), 1);

    tree.expand_all();
    assert_eq!(tree.visible_nodes().len(), 7);

    // Symbol lookup drives editor -> tree sync.
    let offset = source.find("localhost").unwrap() + 2;
    let sym = tree.find_symbol_at_offset(offset).unwrap();
    assert_eq!(sym.path, "$.server.host");
}

#[test]
fn test_toml_format_normalizes_spacing() {
    let source = "a=1\nb   =   2\n[table]\nc = \"x\"\n";
    let formatted = format_toml(source, 2).unwrap();
    assert_eq!(formatted, "a = 1\nb = 2\n[table]\nc = \"x\"\n");

    // A blank line present in the source is preserved.
    let spaced = "a=1\n\n[table]\nc = \"x\"\n";
    assert_eq!(
        format_toml(spaced, 2).unwrap(),
        "a = 1\n\n[table]\nc = \"x\"\n"
    );

    // Formatting must not change the parsed meaning.
    let before = parse_toml(source).unwrap().unwrap();
    let after = parse_toml(&formatted).unwrap().unwrap();
    let before_pairs: Vec<(String, String)> = before
        .children
        .iter()
        .map(|n| (n.path.clone(), n.value_preview.clone()))
        .collect();
    let after_pairs: Vec<(String, String)> = after
        .children
        .iter()
        .map(|n| (n.path.clone(), n.value_preview.clone()))
        .collect();
    assert_eq!(before_pairs, after_pairs);
}

#[test]
fn test_toml_format_preserves_comments() {
    let source = "# keep me\na = 1 # trailing\n";
    let formatted = format_toml(source, 2).unwrap();
    assert!(formatted.contains("# keep me"));
    assert!(formatted.contains("# trailing"));
}

#[test]
fn test_toml_format_rejects_invalid() {
    assert!(format_toml("a = \n", 2).is_err());
    assert!(minify_toml("[[[bad\n").is_err());
}

#[test]
fn test_toml_minify_strips_blank_lines_and_comments() {
    let source = "# comment\na = 1\n\n\nb = 2\n";
    let minified = minify_toml(source).unwrap();
    assert_eq!(minified, "a = 1\nb = 2");
}

#[test]
fn test_toml_format_is_idempotent() {
    let source = "a = 1\n[t]\nb = 2\narr = [\n  1,\n  2,\n]\n";
    let once = format_toml(source, 2).unwrap();
    let twice = format_toml(&once, 2).unwrap();
    assert_eq!(once, twice);
}

#[test]
fn test_toml_recursion_depth_limit() {
    // Nest arrays far beyond the parser limit.
    let deep = "[".repeat(200) + &"]".repeat(200);
    let source = format!("a = {deep}\n");
    let err = parse_toml(&source).unwrap_err();
    assert!(err.message.contains("nesting depth"), "got: {}", err.message);
}
