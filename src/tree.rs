pub mod view;

pub use view::render_tree_view;

use crate::formats::json::{JsonSpan, JsonTreeNode, JsonType};
use gpui::{SharedString, UniformListScrollHandle};
use std::collections::HashSet;
use std::sync::Arc;

/// Pre-computed flattened representation of a visible tree node for O(1) virtualized rendering.
#[derive(Clone, Debug)]
pub struct TreeRowData {
    pub id: usize,
    pub path: SharedString,
    pub span: JsonSpan,
    pub full_span: JsonSpan,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub json_type: JsonType,
    pub key: Option<SharedString>,
    pub value_preview: SharedString,
}

/// Metadata entry for a symbol in document order for O(log N) lookup and navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolEntry {
    pub id: usize,
    pub path: String,
    pub span: JsonSpan,
    pub full_span: JsonSpan,
    pub ancestor_paths: Arc<[String]>,
}

/// State for the hierarchical tree view with cached layout and symbol indexing.
#[derive(Clone, Debug, Default)]
pub struct TreeState {
    /// Root node of the parsed JSON tree.
    root: Option<JsonTreeNode>,
    /// Pre-computed total node count for O(1) status queries.
    total_node_count: usize,
    /// Set of expanded JSONPath strings.
    expanded_paths: HashSet<String>,
    /// Currently selected node ID.
    selected_id: Option<usize>,
    /// Optional filter query to filter nodes by key or path.
    filter_query: String,
    /// Pre-computed flattened visible rows for O(1) virtualized rendering.
    cached_rows: Arc<[TreeRowData]>,
    /// Pre-computed flat symbol index in document order for O(log N) offset lookup and navigation.
    symbols: Arc<[SymbolEntry]>,
    /// Virtual list scroll handle for uniform list rendering.
    scroll_handle: UniformListScrollHandle,
}

impl TreeState {
    /// Creates a new empty tree state.
    pub fn new() -> Self {
        Self {
            root: None,
            total_node_count: 0,
            expanded_paths: HashSet::new(),
            selected_id: None,
            filter_query: String::new(),
            cached_rows: Arc::new([]),
            symbols: Arc::new([]),
            scroll_handle: UniformListScrollHandle::new(),
        }
    }

    /// Returns the scroll handle for virtualized rendering.
    pub fn scroll_handle(&self) -> UniformListScrollHandle {
        self.scroll_handle.clone()
    }

    /// Sets the root JSON node, indexes all symbols, and preserves expanded paths.
    ///
    /// Auto-expands the first 2 levels on initial load.
    ///
    /// Complexity: O(N) where N is the total node count.
    pub fn set_root(&mut self, root: Option<JsonTreeNode>) {
        self.root = root;
        self.selected_id = None;

        // Auto-expand the first 2 levels by default only on initial load
        if self.expanded_paths.is_empty() {
            if let Some(ref r) = self.root {
                self.expanded_paths.insert(r.path.clone());
                for child in &r.children {
                    self.expanded_paths.insert(child.path.clone());
                }
            }
        }

        // Pre-compute symbol table and total node count in a single pass
        let mut symbols = Vec::new();
        if let Some(ref r) = self.root {
            let mut ancestors = Vec::new();
            Self::collect_symbols(r, &mut ancestors, &mut symbols);
        }
        self.total_node_count = symbols.len();
        self.symbols = symbols.into();

        self.rebuild_cached_rows();
    }

    /// Clears the tree state completely.
    pub fn clear(&mut self) {
        self.root = None;
        self.total_node_count = 0;
        self.expanded_paths.clear();
        self.selected_id = None;
        self.filter_query.clear();
        self.cached_rows = Arc::new([]);
        self.symbols = Arc::new([]);
    }

    /// Returns a reference to the root node.
    pub fn root(&self) -> Option<&JsonTreeNode> {
        self.root.as_ref()
    }

    /// Returns the pre-computed total node count in O(1).
    pub fn total_node_count(&self) -> usize {
        self.total_node_count
    }

    /// Returns the pre-computed cached visible rows in O(1).
    pub fn cached_rows(&self) -> Arc<[TreeRowData]> {
        self.cached_rows.clone()
    }

    /// Toggles the expanded state of a node by JSONPath.
    pub fn toggle_expand(&mut self, path: &str) {
        if self.expanded_paths.contains(path) {
            self.expanded_paths.remove(path);
        } else {
            self.expanded_paths.insert(path.to_string());
        }
        self.rebuild_cached_rows();
    }

    /// Toggles the expanded state of a node by its numeric ID.
    pub fn toggle_expand_id(&mut self, node_id: usize) {
        if let Some(sym) = self.find_symbol_by_id(node_id) {
            let path = sym.path.clone();
            self.toggle_expand(&path);
        }
    }

    /// Checks if a node with the given JSONPath is currently expanded.
    pub fn is_expanded(&self, path: &str) -> bool {
        self.expanded_paths.contains(path)
    }

    /// Checks if a node with the given numeric ID is currently expanded.
    pub fn is_expanded_id(&self, node_id: usize) -> bool {
        if let Some(sym) = self.find_symbol_by_id(node_id) {
            self.is_expanded(&sym.path)
        } else {
            false
        }
    }

    /// Expands all nodes in the tree.
    ///
    /// Complexity: O(N) where N is the total node count.
    pub fn expand_all(&mut self) {
        if let Some(ref r) = self.root {
            Self::collect_all_paths(r, &mut self.expanded_paths);
            self.rebuild_cached_rows();
        }
    }

    /// Collapses all nodes in the tree.
    pub fn collapse_all(&mut self) {
        self.expanded_paths.clear();
        self.rebuild_cached_rows();
    }

    /// Returns the currently selected node ID.
    pub fn selected_id(&self) -> Option<usize> {
        self.selected_id
    }

    /// Sets the selected node ID and expands ancestor paths if not already expanded.
    ///
    /// Complexity: O(D) where D is ancestor depth, typically O(1).
    pub fn select_node(&mut self, node_id: Option<usize>) {
        if self.selected_id == node_id {
            return;
        }
        self.selected_id = node_id;

        // Ensure ancestor path is expanded so the selected node is visible
        if let Some(id) = node_id {
            let ancestors = self.find_symbol_by_id(id).map(|s| s.ancestor_paths.clone());
            if let Some(ancestor_list) = ancestors {
                let mut newly_expanded = false;
                for ancestor in ancestor_list.iter() {
                    if self.expanded_paths.insert(ancestor.clone()) {
                        newly_expanded = true;
                    }
                }
                if newly_expanded {
                    self.rebuild_cached_rows();
                }
            }
        }
    }

    /// Centers the virtual list on the currently selected item.
    pub fn scroll_to_selected(&self) {
        if let Some(sel_id) = self.selected_id {
            if let Some(idx) = self.cached_rows.iter().position(|r| r.id == sel_id) {
                self.scroll_handle
                    .scroll_to_item(idx, gpui::ScrollStrategy::Center);
            }
        }
    }

    /// Returns the current filter query.
    pub fn filter_query(&self) -> &str {
        &self.filter_query
    }

    /// Sets the filter query and updates cached rows.
    pub fn set_filter_query(&mut self, query: &str) {
        self.filter_query = query.to_string();
        self.rebuild_cached_rows();
    }

    /// Returns a symbol by its unique node ID.
    ///
    /// Complexity: O(1) fast path when ID matches 1-based index, else O(N).
    pub fn find_symbol_by_id(&self, id: usize) -> Option<&SymbolEntry> {
        if id > 0 && id <= self.symbols.len() && self.symbols[id - 1].id == id {
            return Some(&self.symbols[id - 1]);
        }
        self.symbols.iter().find(|s| s.id == id)
    }

    /// Returns the symbol whose span most tightly encloses the given byte offset.
    ///
    /// Complexity: O(log N) via binary search.
    pub fn find_symbol_at_offset(&self, offset: usize) -> Option<&SymbolEntry> {
        if self.symbols.is_empty() {
            return None;
        }

        // Binary search for candidate index
        let upper_bound = match self.symbols.binary_search_by(|s| {
            if s.full_span.start_byte <= offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) => idx,
            Err(idx) => idx,
        };

        // Scan backwards through preceding candidates to find the tightest enclosing span
        let mut best: Option<&SymbolEntry> = None;
        let mut min_len = usize::MAX;

        let check_start = upper_bound.saturating_sub(64);
        for i in (check_start..upper_bound).rev() {
            if let Some(sym) = self.symbols.get(i) {
                if sym.full_span.start_byte <= offset && offset <= sym.full_span.end_byte {
                    let span_len = sym.full_span.end_byte - sym.full_span.start_byte;
                    if span_len < min_len {
                        min_len = span_len;
                        best = Some(sym);
                    }
                }
            }
        }

        best
    }

    /// Returns the next symbol in document order strictly after `offset`.
    /// Wraps around to the first symbol if at the end of document.
    ///
    /// Complexity: O(log N).
    pub fn next_symbol(&self, offset: usize) -> Option<&SymbolEntry> {
        if self.symbols.is_empty() {
            return None;
        }

        match self.symbols.binary_search_by(|s| {
            if s.full_span.start_byte <= offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) | Err(idx) => {
                if idx < self.symbols.len() {
                    Some(&self.symbols[idx])
                } else {
                    Some(&self.symbols[0])
                }
            }
        }
    }

    /// Returns the previous symbol in document order strictly before `offset`.
    /// Wraps around to the last symbol if at the start of document.
    ///
    /// Complexity: O(log N).
    pub fn prev_symbol(&self, offset: usize) -> Option<&SymbolEntry> {
        if self.symbols.is_empty() {
            return None;
        }

        match self.symbols.binary_search_by(|s| {
            if s.full_span.start_byte < offset {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        }) {
            Ok(idx) | Err(idx) => {
                if idx > 0 {
                    Some(&self.symbols[idx - 1])
                } else {
                    Some(&self.symbols[self.symbols.len() - 1])
                }
            }
        }
    }

    /// Flattens visible nodes based on expansion state and filter.
    ///
    /// Complexity: O(V) where V is the number of visible nodes.
    pub fn visible_nodes(&self) -> Vec<(&JsonTreeNode, usize)> {
        let mut list = Vec::new();
        if let Some(ref root) = self.root {
            self.collect_visible_nodes(root, 0, &mut list);
        }
        list
    }

    /// Rebuilds the flattened visible rows cache.
    ///
    /// Complexity: O(V) where V is the number of visible nodes.
    fn rebuild_cached_rows(&mut self) {
        let mut rows = Vec::new();
        if let Some(ref root) = self.root {
            self.collect_visible_row_data(root, 0, &mut rows);
        }
        self.cached_rows = rows.into();
    }

    fn collect_visible_row_data(
        &self,
        node: &JsonTreeNode,
        depth: usize,
        out: &mut Vec<TreeRowData>,
    ) {
        let is_expanded = self.is_expanded(&node.path);
        let has_children = !node.children.is_empty();

        if self.filter_query.is_empty() {
            out.push(TreeRowData {
                id: node.id,
                path: SharedString::from(node.path.clone()),
                span: node.span,
                full_span: node.full_span(),
                depth,
                has_children,
                is_expanded,
                json_type: node.json_type,
                key: node
                    .key
                    .as_ref()
                    .map(|k| SharedString::from(format!("\"{}\":", k))),
                value_preview: SharedString::from(node.value_preview.clone()),
            });
            if is_expanded {
                for child in &node.children {
                    self.collect_visible_row_data(child, depth + 1, out);
                }
            }
        } else {
            let query_lower = self.filter_query.to_lowercase();
            let matches = node.matches_filter(&query_lower);
            let descendant_matches = node.has_matching_descendant(&query_lower);
            if matches || descendant_matches {
                out.push(TreeRowData {
                    id: node.id,
                    path: SharedString::from(node.path.clone()),
                    span: node.span,
                    full_span: node.full_span(),
                    depth,
                    has_children,
                    is_expanded,
                    json_type: node.json_type,
                    key: node
                        .key
                        .as_ref()
                        .map(|k| SharedString::from(format!("\"{}\":", k))),
                    value_preview: SharedString::from(node.value_preview.clone()),
                });
                for child in &node.children {
                    self.collect_visible_row_data(child, depth + 1, out);
                }
            }
        }
    }

    fn collect_visible_nodes<'a>(
        &'a self,
        node: &'a JsonTreeNode,
        depth: usize,
        out: &mut Vec<(&'a JsonTreeNode, usize)>,
    ) {
        if self.filter_query.is_empty() {
            out.push((node, depth));
            if self.is_expanded(&node.path) {
                for child in &node.children {
                    self.collect_visible_nodes(child, depth + 1, out);
                }
            }
        } else {
            let query_lower = self.filter_query.to_lowercase();
            let matches = node.matches_filter(&query_lower);
            let descendant_matches = node.has_matching_descendant(&query_lower);
            if matches || descendant_matches {
                out.push((node, depth));
                for child in &node.children {
                    self.collect_visible_nodes(child, depth + 1, out);
                }
            }
        }
    }

    fn collect_all_paths(node: &JsonTreeNode, out: &mut HashSet<String>) {
        out.insert(node.path.clone());
        for child in &node.children {
            Self::collect_all_paths(child, out);
        }
    }

    fn collect_symbols(
        node: &JsonTreeNode,
        ancestors: &mut Vec<String>,
        symbols: &mut Vec<SymbolEntry>,
    ) {
        let ancestor_paths: Arc<[String]> = ancestors.as_slice().into();
        symbols.push(SymbolEntry {
            id: node.id,
            path: node.path.clone(),
            span: node.span,
            full_span: node.full_span(),
            ancestor_paths,
        });

        ancestors.push(node.path.clone());
        for child in &node.children {
            Self::collect_symbols(child, ancestors, symbols);
        }
        ancestors.pop();
    }
}
