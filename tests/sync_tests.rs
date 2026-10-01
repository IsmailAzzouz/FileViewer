use file_viewer::editor::TextBuffer;
use file_viewer::formats::json::{format_json, parse_json, JsonType};
use file_viewer::formats::node::NodeType;
use file_viewer::tree::TreeState;

#[test]
fn test_tree_to_editor_sync_exact_span() {
    let source = r#"{
  "service": "authentication",
  "port": 8080,
  "endpoints": [
    "/login",
    "/logout"
  ]
}"#;

    let mut buffer = TextBuffer::new(source);
    let root = parse_json(source).unwrap().unwrap();

    // Select the second endpoint "/logout"
    let endpoints_node = &root.children[2];
    assert_eq!(endpoints_node.key.as_deref(), Some("endpoints"));
    let logout_node = &endpoints_node.children[1];
    assert_eq!(logout_node.value_preview, "\"/logout\"");

    // Tree-to-editor sync: set buffer selection from node span
    buffer.set_selection(logout_node.span.start_byte, logout_node.span.end_byte);
    let (sel_start, sel_end) = buffer.selection().unwrap();
    let selected_text = &buffer.text()[sel_start..sel_end];
    assert_eq!(selected_text, "\"/logout\"");

    // Verify line/col matches span
    let (start_line, start_col) = buffer.offset_to_line_col(sel_start);
    assert_eq!(start_line, logout_node.span.start_line);
    assert_eq!(start_col, logout_node.span.start_col);
}

#[test]
fn test_editor_to_tree_sync_cursor_navigation() {
    let source = r#"{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}"#;

    let mut buffer = TextBuffer::new(source);
    let root = parse_json(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root.clone()));
    tree.collapse_all();

    // Position cursor inside "localhost" in editor
    let host_val_offset = source.find("localhost").unwrap() + 2;
    buffer.set_cursor(host_val_offset);

    // Editor-to-tree sync: find node at cursor
    let found = root.find_node_at_offset(buffer.cursor()).unwrap();
    assert_eq!(found.key.as_deref(), Some("host"));
    assert_eq!(found.value_preview, "\"localhost\"");

    tree.select_node(Some(found.id));
    assert_eq!(tree.selected_id(), Some(found.id));
    // Verify that selecting the leaf node expanded its ancestors
    let visible = tree.visible_nodes();
    assert!(visible.iter().any(|(n, _)| n.id == found.id));
}

#[test]
fn test_unicode_and_multibyte_spans() {
    let source = r#"{"titre": "café crème", "emoji": "Rust 🦀"}"#;
    let mut buffer = TextBuffer::new(source);
    let root = parse_json(source).unwrap().unwrap();

    let cafe_node = &root.children[0];
    assert_eq!(cafe_node.value_preview, "\"café crème\"");
    let slice = &buffer.text()[cafe_node.span.start_byte..cafe_node.span.end_byte];
    assert_eq!(slice, "\"café crème\"");

    let rust_node = &root.children[1];
    let slice2 = &buffer.text()[rust_node.span.start_byte..rust_node.span.end_byte];
    assert_eq!(slice2, "\"Rust 🦀\"");

    // Test cursor inside multibyte emoji
    buffer.set_cursor(rust_node.span.start_byte + 3);
    let found = root.find_node_at_offset(buffer.cursor()).unwrap();
    assert_eq!(found.key.as_deref(), Some("emoji"));
}

#[test]
fn test_deeply_nested_json() {
    // Construct 10 levels of nesting
    let mut source = String::new();
    for i in 0..10 {
        source.push_str(&format!("{{\"level_{}\": ", i));
    }
    source.push_str("123");
    for _ in 0..10 {
        source.push('}');
    }

    let root = parse_json(&source).unwrap().unwrap();
    assert_eq!(root.node_type, NodeType::Json(JsonType::Object));

    // Count depth
    let mut current = &root;
    let mut depth = 0;
    while !current.children.is_empty() {
        depth += 1;
        current = &current.children[0];
    }
    assert_eq!(depth, 10);
    assert_eq!(current.node_type, NodeType::Json(JsonType::Number));
    assert_eq!(current.value_preview, "123");
}

#[test]
fn test_empty_containers() {
    let source = r#"{"empty_obj": {}, "empty_arr": []}"#;
    let root = parse_json(source).unwrap().unwrap();
    assert_eq!(root.children.len(), 2);

    let obj = &root.children[0];
    assert_eq!(obj.node_type, NodeType::Json(JsonType::Object));
    assert!(obj.children.is_empty());
    assert_eq!(obj.value_preview, "{ 0 items }");

    let arr = &root.children[1];
    assert_eq!(arr.node_type, NodeType::Json(JsonType::Array));
    assert!(arr.children.is_empty());
    assert_eq!(arr.value_preview, "[ 0 items ]");
}

#[test]
fn test_format_sync_roundtrip() {
    let unformatted = r#"{"b":2,"a":1}"#;
    let formatted = format_json(unformatted, 2).unwrap();

    let root = parse_json(&formatted).unwrap().unwrap();
    let buffer = TextBuffer::new(&formatted);

    // Node 'a' should have valid span in formatted text
    let a_node = root
        .children
        .iter()
        .find(|n| n.key.as_deref() == Some("a"))
        .unwrap();
    let text = &buffer.text()[a_node.span.start_byte..a_node.span.end_byte];
    assert_eq!(text, "1");
}

#[test]
fn test_editor_to_tree_sync_clicking_object_key() {
    let source = r#"{"name": "Alice", "age": 30}"#;
    let root = parse_json(source).unwrap().unwrap();
    let mut buffer = TextBuffer::new(source);
    let mut tree = TreeState::new();
    tree.set_root(Some(root.clone()));

    // Cursor directly on the key `"name"`
    let name_key_offset = source.find("name").unwrap() + 1;
    buffer.set_cursor(name_key_offset);

    // Finding node at this offset should resolve to the property node for "name"
    let found = root.find_node_at_offset(buffer.cursor()).unwrap();
    assert_eq!(found.key.as_deref(), Some("name"));
    assert_eq!(found.value_preview, "\"Alice\"");

    // Verify full_span covers both the key and the value
    let full = found.full_span();
    assert_eq!(
        &source[full.start_byte..full.end_byte],
        "\"name\": \"Alice\""
    );

    // Tree selection from clicking this key
    tree.select_node(Some(found.id));
    assert_eq!(tree.selected_id(), Some(found.id));
}
