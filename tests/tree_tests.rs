use file_viewer::formats::json::parse_json;
use file_viewer::tree::TreeState;

#[test]
fn test_tree_expansion_and_visibility() {
    let source = r#"{
      "users": [
        {"id": 1, "name": "Alice"},
        {"id": 2, "name": "Bob"}
      ]
    }"#;

    let root = parse_json(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));

    // Initially root and depth 1 are auto-expanded
    let visible_init = tree.visible_nodes();
    assert!(!visible_init.is_empty());

    // Collapse all
    tree.collapse_all();
    let visible_collapsed = tree.visible_nodes();
    assert_eq!(visible_collapsed.len(), 1); // Only root visible

    // Expand all
    tree.expand_all();
    let visible_expanded = tree.visible_nodes();
    // root (1) + users array (1) + 2 user objects (2) + 4 properties (id, name, id, name) = 8
    assert_eq!(visible_expanded.len(), 8);
}

#[test]
fn test_tree_select_expands_ancestors() {
    let source = r#"{"a": {"b": {"c": 42}}}"#;
    let root = parse_json(source).unwrap().unwrap();
    let leaf_id = root.children[0].children[0].children[0].id;

    let mut tree = TreeState::new();
    tree.set_root(Some(root));
    tree.collapse_all();

    // Leaf should not be visible when collapsed
    assert_eq!(tree.visible_nodes().len(), 1);

    // Selecting leaf expands ancestors
    tree.select_node(Some(leaf_id));
    assert_eq!(tree.selected_id(), Some(leaf_id));

    let visible = tree.visible_nodes();
    assert!(visible.iter().any(|(n, _)| n.id == leaf_id));
}

#[test]
fn test_tree_filtering() {
    let source = r#"{"alpha": 1, "beta": 2, "gamma": 3}"#;
    let root = parse_json(source).unwrap().unwrap();

    let mut tree = TreeState::new();
    tree.set_root(Some(root));
    tree.set_filter_query("beta");

    let visible = tree.visible_nodes();
    assert_eq!(visible.len(), 2); // Root object + matching "beta" property
    assert!(visible
        .iter()
        .any(|(n, _)| n.key.as_deref() == Some("beta")));
    assert!(!visible
        .iter()
        .any(|(n, _)| n.key.as_deref() == Some("alpha")));
    assert!(!visible
        .iter()
        .any(|(n, _)| n.key.as_deref() == Some("gamma")));
}

#[test]
fn test_tree_expansion_persistence_across_edits() {
    let source1 = r#"{"a": {"b": {"c": 42}}, "d": [1, 2]}"#;
    let root1 = parse_json(source1).unwrap().unwrap();

    let mut tree = TreeState::new();
    tree.set_root(Some(root1));

    // Collapse everything first
    tree.collapse_all();
    assert_eq!(tree.visible_nodes().len(), 1);

    // Expand specifically "$.a" and "$.a.b"
    tree.toggle_expand("$.a");
    tree.toggle_expand("$.a.b");
    assert!(tree.is_expanded("$.a"));
    assert!(tree.is_expanded("$.a.b"));
    assert!(!tree.is_expanded("$.d"));

    // Now simulate an edit in editor (e.g. changing 42 to 43 or adding a property) and re-parsing:
    let source2 = r#"{"a": {"b": {"c": 43, "extra": true}}, "d": [1, 2]}"#;
    let root2 = parse_json(source2).unwrap().unwrap();

    // Call set_root with the updated AST
    tree.set_root(Some(root2));

    // Verify expansions were preserved!
    assert!(tree.is_expanded("$.a"), "$.a should remain expanded");
    assert!(tree.is_expanded("$.a.b"), "$.a.b should remain expanded");
    assert!(!tree.is_expanded("$.d"), "$.d should remain collapsed");

    // Clear explicitly (e.g. on new file) resets expansions
    tree.clear();
    assert!(!tree.is_expanded("$.a"));
}

#[test]
fn test_tree_cached_rows_and_total_count() {
    let source = r#"{
      "name": "FileViewer",
      "version": 1,
      "nested": {
        "active": true
      }
    }"#;
    let root = parse_json(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));

    // Total node count is pre-computed in O(1)
    // root (1) + name (1) + version (1) + nested (1) + active (1) = 5
    assert_eq!(tree.total_node_count(), 5);

    // Cached rows match visible nodes length
    assert_eq!(tree.cached_rows().len(), tree.visible_nodes().len());

    // Collapsing updates cached_rows in O(V)
    tree.collapse_all();
    assert_eq!(tree.cached_rows().len(), 1);
    assert_eq!(tree.total_node_count(), 5); // Total count remains constant

    // Expanding all updates cached_rows
    tree.expand_all();
    assert_eq!(tree.cached_rows().len(), 5);
}

#[test]
fn test_symbol_navigation_next_prev_cycle() {
    let source = r#"{"a": 1, "b": 2, "c": 3}"#;
    let root = parse_json(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));

    // Offset at 0 (root object starts at 0): next symbol after 0 is first child "a": 1 (id 2)
    let next_from_start = tree.next_symbol(0).unwrap();
    assert_eq!(next_from_start.id, 2);

    // Offset at start of "a": 1 -> next symbol is "b": 2 (id 3)
    let next_sym = tree
        .next_symbol(next_from_start.full_span.start_byte + 2)
        .unwrap();
    assert_eq!(next_sym.id, 3);
    assert!(next_sym.full_span.start_byte > next_from_start.full_span.start_byte);

    // Cycling at document end wraps back to first symbol (root object, id 1)
    let wrapped = tree.next_symbol(source.len() + 10).unwrap();
    assert_eq!(wrapped.id, 1);

    // Prev symbol cycling from beginning wraps back to last symbol ("c": 3, id 4)
    let prev_wrapped = tree.prev_symbol(0).unwrap();
    assert_eq!(prev_wrapped.id, 4);
    assert!(prev_wrapped.full_span.start_byte > 0);
}

#[test]
fn test_symbol_lookup_at_offset() {
    let source = r#"{"user": {"name": "Alice", "score": 99}}"#;
    let root = parse_json(source).unwrap().unwrap();
    let mut tree = TreeState::new();
    tree.set_root(Some(root));

    // Find offset inside "Alice"
    let alice_offset = source.find("Alice").unwrap() + 1;
    let sym = tree.find_symbol_at_offset(alice_offset).unwrap();
    assert_eq!(sym.path, "$.user.name");

    // Find offset inside "score"
    let score_offset = source.find("score").unwrap() + 1;
    let sym_score = tree.find_symbol_at_offset(score_offset).unwrap();
    assert_eq!(sym_score.path, "$.user.score");
}
