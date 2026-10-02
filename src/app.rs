//! Main application view entity orchestrating the editor, tree view,
//! file operations, and user interactions.

use crate::editor::{
    char_index_at_x, char_index_to_byte_offset, compute_visual_rows, find_visual_row_by_offset,
    render_editor, EditorProps, TextBuffer, VisualRow,
};
use crate::formats::diagnostic::{Diagnostic, FileFormat};
use crate::formats::json::{
    format_json, format_jsonc, format_jsonl, minify_json, minify_jsonc, minify_jsonl, parse_json,
    parse_jsonc, parse_jsonl, JsonSpan,
};
use crate::formats::toml::{format_toml, minify_toml, parse_toml};
use crate::formats::yaml::{format_yaml, minify_yaml, parse_yaml};
use crate::theme::{
    BG_APP, CHAR_WIDTH, CODE_PADDING_LEFT, DEFAULT_WRAP_COLUMN, GUTTER_WIDTH, TEXT_MUTED,
    TREE_PANEL_WIDTH,
};
use crate::tree::{render_tree_view, TreeState};
use crate::ui::{
    render_diagnostics, render_empty_state, render_status_bar, render_toolbar, StatusBarProps,
    ToolbarProps,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::path::PathBuf;
use std::sync::Arc;

/// Default rich sample JSON demonstrating all JSON types and structures.
pub const SAMPLE_JSON: &str = r#"{
  "project": "FileViewer",
  "version": "1.0.0",
  "active": true,
  "stats": {
    "fps": 60,
    "memory_mb": 14.5,
    "threads": 4
  },
  "features": [
    "Native GPUI rendering",
    "Instant JSON formatting",
    "Precise syntax validation",
    "Two-way tree synchronization"
  ],
  "author": {
    "name": "Rust Engineer",
    "email": "engineer@example.com",
    "verified": true,
    "metadata": null
  },
  "configurations": [
    {
      "id": 1,
      "theme": "dark",
      "font_size": 13,
      "auto_save": false
    },
    {
      "id": 2,
      "theme": "system",
      "font_size": 14,
      "auto_save": true
    }
  ]
}"#;

/// Default rich sample JSONC demonstrating comments and trailing commas.
pub const SAMPLE_JSONC: &str = r#"{
  // Compiler options
  "compilerOptions": {
    /* Module resolution and language level */
    "target": "es2020",
    "module": "esnext",
    "strict": true,
  },
  "include": [
    "src/**/*", // only our sources
  ],
  "exclude": ["node_modules"],
  "version": 2 // trailing commas are allowed here
}"#;

/// Default rich sample JSON Lines demonstrating multi-document records.
pub const SAMPLE_JSONL: &str = r#"{"id": 1, "event": "open", "path": "README.md", "ok": true}
{"id": 2, "event": "open", "path": "src/main.rs", "ok": true}
{"id": 3, "event": "close", "path": "src/main.rs", "ok": false}

{"id": 4, "event": "close", "path": "README.md", "ok": true}
{"id": 5, "event": "error", "path": "Cargo.toml", "ok": false, "code": 404}
"#;
/// Default rich sample TOML demonstrating the common TOML constructs.
pub const SAMPLE_TOML: &str = r#"# FileViewer configuration
project = "FileViewer"
version = "1.0.0"
active = true

[stats]
fps = 60
memory_mb = 14.5
threads = 4

features = [
  "Native GPUI rendering",
  "Instant TOML formatting",
  "Precise syntax validation",
  "Two-way tree synchronization",
]

[author]
name = "Rust Engineer"
email = "engineer@example.com"
verified = true

[[configurations]]
id = 1
theme = "dark"
font_size = 13
auto_save = false

[[configurations]]
id = 2
theme = "system"
font_size = 14
auto_save = true
"#;

/// Default rich sample YAML demonstrating the common YAML constructs.
pub const SAMPLE_YAML: &str = r#"# FileViewer deployment manifest
apiVersion: apps/v1
kind: Deployment
metadata:
  name: file-viewer
  labels:
    app: file-viewer
    tier: backend
  annotations:
    description: >-
      Native GPUI viewer with precise
      syntax validation.

spec:
  replicas: 3
  paused: false
  strategy:
    type: RollingUpdate
  template:
    spec:
      containers:
        - name: file-viewer
          image: "file-viewer:1.0.0"
          ports:
            - containerPort: 8080
              protocol: TCP
          env:
            - name: RUST_LOG
              value: info
          resources:
            limits:
              cpu: "500m"
              memory: 512Mi

---
apiVersion: v1
kind: Service
metadata:
  name: file-viewer-svc
spec:
  type: ClusterIP
  ports:
    - port: 80
      targetPort: 8080
"#;

/// The root application view entity.
pub struct AppView {
    /// Document text buffer.
    buffer: TextBuffer,
    /// Hierarchical tree view state.
    tree: TreeState,
    /// Active file path on disk (if any).
    file_path: Option<PathBuf>,
    /// Format of the active document.
    file_format: FileFormat,
    /// Whether the document has unsaved modifications.
    is_dirty: bool,
    /// Current validation diagnostic if syntax is invalid.
    diagnostic: Option<Diagnostic>,
    /// Whether the Tree View panel is visible.
    is_tree_visible: bool,
    /// Whether the Find/Search bar is visible.
    is_search_open: bool,
    /// Whether word wrapping is enabled.
    is_word_wrap: bool,
    /// Cached visual rows for editor virtual list rendering.
    visual_rows: Arc<[VisualRow]>,
    /// Drag selection start anchor byte offset.
    drag_anchor: Option<usize>,
    /// Generation counter for debounced background parsing.
    parse_generation: usize,
    /// Current search query string.
    search_query: String,
    /// Matches found for the current search query.
    search_matches: Vec<(usize, usize)>,
    /// Index into `search_matches` for the currently highlighted match.
    current_match_idx: usize,
    /// Editor list scroll handle.
    scroll_handle: UniformListScrollHandle,
    /// Keyboard focus handle for the editor.
    focus_handle: FocusHandle,
    /// Whether an asynchronous background operation is active.
    is_loading: bool,
    /// Transient status or notification message.
    status_message: Option<String>,
}

impl AppView {
    /// Creates a new application view.
    pub fn new(initial_file: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let mut app = Self {
            buffer: TextBuffer::new(""),
            tree: TreeState::new(),
            file_path: None,
            file_format: FileFormat::default(),
            is_dirty: false,
            diagnostic: None,
            is_tree_visible: true,
            is_search_open: false,
            is_word_wrap: true,
            visual_rows: Arc::new([]),
            drag_anchor: None,
            parse_generation: 0,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: 0,
            scroll_handle: UniformListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
            is_loading: false,
            status_message: None,
        };
        app.rebuild_visual_rows();

        if let Some(path) = initial_file {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    app.load_file(path, content, cx);
                }
                Err(err) => {
                    app.status_message =
                        Some(format!("Failed to open {}: {}", path.display(), err));
                }
            }
        }

        app
    }

    /// Returns a reference to the editor focus handle.
    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    /// Automatically scrolls the editor viewport so the current visual row of cursor remains visible.
    fn autoscroll_to_cursor(&mut self) {
        let visual_idx = find_visual_row_by_offset(&self.visual_rows, self.buffer.cursor());
        self.scroll_handle
            .scroll_to_item(visual_idx, ScrollStrategy::Center);
    }

    /// Recomputes visual rows for the document buffer.
    ///
    /// Complexity: O(N) where N is the document byte count.
    pub fn rebuild_visual_rows(&mut self) {
        self.visual_rows = compute_visual_rows(
            &self.buffer.snapshot(),
            self.is_word_wrap,
            DEFAULT_WRAP_COLUMN,
        );
    }

    /// Toggles word wrapping on or off.
    pub fn toggle_word_wrap(&mut self, cx: &mut Context<Self>) {
        self.is_word_wrap = !self.is_word_wrap;
        self.rebuild_visual_rows();
        self.autoscroll_to_cursor();
        self.status_message = Some(if self.is_word_wrap {
            "Word Wrap: ON".to_string()
        } else {
            "Word Wrap: OFF".to_string()
        });
        cx.notify();
    }

    /// Schedules an asynchronous debounced parse and sync operation.
    ///
    /// Ensures 144+ FPS editing responsiveness by debouncing full JSON tree
    /// parsing to a background thread pool with a 150ms trailing window.
    pub fn schedule_debounced_parse(&mut self, cx: &mut Context<Self>) {
        self.parse_generation += 1;
        let gen = self.parse_generation;
        let text_arc: Arc<str> = self.buffer.snapshot().text().into();
        let format = self.file_format;

        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(150))
                .await;

            let is_latest = this
                .update(cx, |view, _cx| view.parse_generation == gen)
                .unwrap_or(false);
            if !is_latest {
                return;
            }

            let parse_result = cx
                .background_executor()
                .spawn(async move {
                    match format {
                        FileFormat::Json => parse_json(&text_arc).map_err(Diagnostic::Json),
                        FileFormat::JsonC => parse_jsonc(&text_arc).map_err(Diagnostic::Json),
                        FileFormat::JsonL => parse_jsonl(&text_arc).map_err(Diagnostic::Json),
                        FileFormat::Toml => parse_toml(&text_arc).map_err(Diagnostic::Toml),
                        FileFormat::Yaml => parse_yaml(&text_arc).map_err(Diagnostic::Yaml),
                    }
                })
                .await;

            let _ = this.update(cx, |view, cx| {
                if view.parse_generation == gen {
                    match parse_result {
                        Ok(root_opt) => {
                            view.diagnostic = None;
                            view.tree.set_root(root_opt);
                            view.sync_tree_selection_from_cursor();
                        }
                        Err(diag) => {
                            view.diagnostic = Some(diag);
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Loads file content into the editor and synchronizes the tree.
    ///
    /// Complexity: O(N) where N is the length of `content`.
    pub fn load_file(&mut self, path: PathBuf, content: String, cx: &mut Context<Self>) {
        let clean = content.strip_prefix('\u{FEFF}').unwrap_or(&content);
        self.file_format = FileFormat::from_path(&path);
        self.tree.clear();
        self.buffer.set_text(clean);
        self.rebuild_visual_rows();
        self.file_path = Some(path);
        self.is_dirty = false;
        self.parse_and_sync(cx);
        self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Loads sample data for the active format into the editor.
    ///
    /// Complexity: O(N) where N is the length of the sample data.
    pub fn load_sample(&mut self, cx: &mut Context<Self>) {
        self.tree.clear();
        match self.file_format {
            FileFormat::Json => self.buffer.set_text(SAMPLE_JSON),
            FileFormat::JsonC => self.buffer.set_text(SAMPLE_JSONC),
            FileFormat::JsonL => self.buffer.set_text(SAMPLE_JSONL),
            FileFormat::Toml => self.buffer.set_text(SAMPLE_TOML),
            FileFormat::Yaml => self.buffer.set_text(SAMPLE_YAML),
        }
        self.rebuild_visual_rows();
        self.file_path = None;
        self.is_dirty = false;
        self.parse_and_sync(cx);
        self.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    /// Clears the entire document buffer.
    pub fn clear_document(&mut self, cx: &mut Context<Self>) {
        self.buffer.set_text("");
        self.rebuild_visual_rows();
        self.file_path = None;
        self.is_dirty = false;
        self.diagnostic = None;
        self.tree.clear();
        self.search_matches.clear();
        self.current_match_idx = 0;
        cx.notify();
    }

    /// Parses the JSON document from the buffer, updating tree and diagnostic state.
    ///
    /// Complexity: O(N) where N is the length of the document text.
    pub fn parse_and_sync(&mut self, _cx: &mut Context<Self>) {
        let text = self.buffer.text();
        if text.trim().is_empty() {
            self.diagnostic = None;
            self.tree.set_root(None);
            return;
        }
        let result = match self.file_format {
            FileFormat::Json => parse_json(text).map_err(Diagnostic::Json),
            FileFormat::JsonC => parse_jsonc(text).map_err(Diagnostic::Json),
            FileFormat::JsonL => parse_jsonl(text).map_err(Diagnostic::Json),
            FileFormat::Toml => parse_toml(text).map_err(Diagnostic::Toml),
            FileFormat::Yaml => parse_yaml(text).map_err(Diagnostic::Yaml),
        };

        match result {
            Ok(root_node) => {
                self.diagnostic = None;
                self.tree.set_root(root_node);
                self.sync_tree_selection_from_cursor();
            }
            Err(diag) => {
                self.diagnostic = Some(diag);
            }
        }
    }

    /// Pretty-formats the JSON document with 2-space indentation.
    ///
    /// Complexity: O(N) where N is the length of the document text.
    pub fn format_document(&mut self, cx: &mut Context<Self>) {
        let text = self.buffer.text();
        if text.trim().is_empty() {
            return;
        }
        let result = match self.file_format {
            FileFormat::Json => format_json(text, 2).map_err(Diagnostic::Json),
            FileFormat::JsonC => format_jsonc(text, 2).map_err(Diagnostic::Json),
            // JSON Lines is already one-record-per-line, so formatting only
            // normalizes trailing whitespace; the indent argument is unused.
            FileFormat::JsonL => format_jsonl(text).map_err(Diagnostic::Json),
            FileFormat::Toml => format_toml(text, 2).map_err(Diagnostic::Toml),
            FileFormat::Yaml => format_yaml(text, 2).map_err(Diagnostic::Yaml),
        };

        match result {
            Ok(formatted) => {
                if formatted != text {
                    self.buffer.set_text(&formatted);
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.parse_and_sync(cx);
                    self.status_message =
                        Some(format!("Formatted {}", self.file_format.name()));
                    cx.notify();
                }
            }
            Err(diag) => {
                self.diagnostic = Some(diag);
                cx.notify();
            }
        }
    }

    /// Minifies the JSON document by stripping whitespace outside strings.
    ///
    /// Complexity: O(N) where N is the length of the document text.
    pub fn minify_document(&mut self, cx: &mut Context<Self>) {
        let text = self.buffer.text();
        let result = match self.file_format {
            FileFormat::Json => minify_json(text).map_err(Diagnostic::Json),
            // Minifying JSONC strips comments, yielding strict JSON.
            FileFormat::JsonC => minify_jsonc(text).map_err(Diagnostic::Json),
            FileFormat::JsonL => minify_jsonl(text).map_err(Diagnostic::Json),
            FileFormat::Toml => minify_toml(text).map_err(Diagnostic::Toml),
            FileFormat::Yaml => minify_yaml(text).map_err(Diagnostic::Yaml),
        };

        match result {
            Ok(minified) => {
                if minified != text {
                    self.buffer.set_text(&minified);
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.parse_and_sync(cx);
                    self.status_message =
                        Some(format!("Minified {}", self.file_format.name()));
                    cx.notify();
                }
            }
            Err(diag) => {
                self.diagnostic = Some(diag);
                cx.notify();
            }
        }
    }

    /// Copies document content or selection to system clipboard.
    ///
    /// Complexity: O(N) where N is copied slice length.
    pub fn copy_all(&mut self, cx: &mut Context<Self>) {
        let text = match self.buffer.selection() {
            Some((start, end)) => self.buffer.text()[start..end].to_string(),
            None => self.buffer.text().to_string(),
        };

        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.status_message = Some("Copied to clipboard".to_string());
            cx.notify();
        }
    }

    /// Pastes text from system clipboard into buffer.
    ///
    /// Complexity: O(N) where N is document length.
    pub fn paste_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                if self.buffer.text().is_empty() {
                    self.buffer.set_text(&text);
                } else {
                    self.buffer.insert_str(&text);
                }
                self.rebuild_visual_rows();
                self.is_dirty = true;
                self.schedule_debounced_parse(cx);
                self.autoscroll_to_cursor();
                cx.notify();
            }
        }
    }

    /// Opens the native platform file picker to load a JSON file.
    pub fn open_file_dialog(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = prompt.await {
                if let Some(path) = paths.into_iter().next() {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => {
                            let _ = this.update(cx, |view, cx| {
                                view.load_file(path, content, cx);
                            });
                        }
                        Err(err) => {
                            let _ = this.update(cx, |view, cx| {
                                view.status_message = Some(format!("Open failed: {}", err));
                                cx.notify();
                            });
                        }
                    }
                }
            }
        })
        .detach();
    }

    /// Saves the current buffer to the existing file path or opens save dialog.
    ///
    /// Complexity: O(N) where N is the length of document.
    pub fn save_file(&mut self, cx: &mut Context<Self>) {
        if let Some(ref path) = self.file_path {
            match std::fs::write(path, self.buffer.text()) {
                Ok(_) => {
                    self.is_dirty = false;
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                    self.status_message = Some(format!("Saved to {}", filename));
                    cx.notify();
                }
                Err(err) => {
                    self.status_message = Some(format!("Save failed: {}", err));
                    cx.notify();
                }
            }
        } else {
            self.save_file_as_dialog(cx);
        }
    }

    /// Opens native save file dialog to specify destination path.
    pub fn save_file_as_dialog(&mut self, cx: &mut Context<Self>) {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let default_name = format!("document.{}", self.file_format.default_extension());
        let prompt = cx.prompt_for_new_path(&cwd, Some(&default_name));

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(path))) = prompt.await {
                let _ = this.update(cx, |view, cx| {
                    match std::fs::write(&path, view.buffer.text()) {
                        Ok(_) => {
                            view.file_path = Some(path.clone());
                            view.file_format = FileFormat::from_path(&path);
                            view.is_dirty = false;
                            let filename =
                                path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
                            view.status_message = Some(format!("Saved to {}", filename));
                            cx.notify();
                        }
                        Err(err) => {
                            view.status_message = Some(format!("Save failed: {}", err));
                            cx.notify();
                        }
                    }
                });
            }
        })
        .detach();
    }

    /// Jumps the editor cursor to the current parse error line.
    pub fn jump_to_error(&mut self, cx: &mut Context<Self>) {
        if let Some(ref diag) = self.diagnostic.clone() {
            let offset = self.buffer.line_col_to_offset(diag.line(), diag.column());
            self.buffer.set_cursor(offset);
            self.scroll_handle
                .scroll_to_item(diag.line().saturating_sub(1), ScrollStrategy::Center);
            cx.notify();
        }
    }

    /// Handles tree node selection, highlighting span in editor and scrolling.
    pub fn handle_tree_node_selected(
        &mut self,
        node_id: usize,
        span: JsonSpan,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        self.tree.select_node(Some(node_id));
        self.buffer.set_selection(span.start_byte, span.end_byte);
        let visual_idx = find_visual_row_by_offset(&self.visual_rows, span.start_byte);
        self.scroll_handle
            .scroll_to_item(visual_idx, ScrollStrategy::Center);
        cx.notify();
    }

    /// Jumps to the next JSON symbol in document order.
    ///
    /// Complexity: O(log N) via pre-computed symbol index.
    pub fn jump_next_symbol(&mut self, cx: &mut Context<Self>) {
        let anchor = self
            .buffer
            .selection()
            .map(|(s, _)| s)
            .unwrap_or_else(|| self.buffer.cursor());
        if let Some(sym) = self.tree.next_symbol(anchor).cloned() {
            self.tree.select_node(Some(sym.id));
            self.tree.scroll_to_selected();
            self.buffer
                .set_selection(sym.full_span.start_byte, sym.full_span.end_byte);
            let visual_idx = find_visual_row_by_offset(&self.visual_rows, sym.full_span.start_byte);
            self.scroll_handle
                .scroll_to_item(visual_idx, ScrollStrategy::Center);
            cx.notify();
        }
    }

    /// Jumps to the previous JSON symbol in document order.
    ///
    /// Complexity: O(log N) via pre-computed symbol index.
    pub fn jump_prev_symbol(&mut self, cx: &mut Context<Self>) {
        let anchor = self
            .buffer
            .selection()
            .map(|(s, _)| s)
            .unwrap_or_else(|| self.buffer.cursor());
        if let Some(sym) = self.tree.prev_symbol(anchor).cloned() {
            self.tree.select_node(Some(sym.id));
            self.tree.scroll_to_selected();
            self.buffer
                .set_selection(sym.full_span.start_byte, sym.full_span.end_byte);
            let visual_idx = find_visual_row_by_offset(&self.visual_rows, sym.full_span.start_byte);
            self.scroll_handle
                .scroll_to_item(visual_idx, ScrollStrategy::Center);
            cx.notify();
        }
    }

    /// Computes the document byte offset corresponding to a mouse X position on a visual row.
    ///
    /// Complexity: O(C) where C is the character count of the visual row.
    pub fn calculate_offset_at_mouse(&self, row_idx: usize, mouse_x: f32) -> usize {
        let row = match self.visual_rows.get(row_idx) {
            Some(r) => r,
            None => return self.buffer.text().len(),
        };

        let x_tree = if self.is_tree_visible {
            f32::from(TREE_PANEL_WIDTH)
        } else {
            0.0
        };
        let x_gutter = f32::from(GUTTER_WIDTH);
        let x_pad = f32::from(CODE_PADDING_LEFT);
        let content_start_x = x_tree + x_gutter + x_pad;
        let rel_x = (mouse_x - content_start_x).max(0.0);

        let full_text = self.buffer.text();
        let row_text = if row.start_byte < full_text.len() && row.end_byte <= full_text.len() {
            &full_text[row.start_byte..row.end_byte]
        } else {
            ""
        };

        let char_idx = char_index_at_x(row_text, rel_x, CHAR_WIDTH);
        let byte_advance = char_index_to_byte_offset(row_text, char_idx);
        (row.start_byte + byte_advance).min(full_text.len())
    }

    /// Handles mouse down event in editor, starting drag selection or placing cursor.
    pub fn handle_editor_mouse_down(
        &mut self,
        row_idx: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let mouse_x = f32::from(event.position.x);
        let offset = self.calculate_offset_at_mouse(row_idx, mouse_x);

        if event.modifiers.shift {
            let anchor = match self.buffer.selection() {
                Some((s, e)) => {
                    if self.buffer.cursor() == s {
                        e
                    } else {
                        s
                    }
                }
                None => self.buffer.cursor(),
            };
            self.buffer.extend_selection_to(anchor, offset);
            self.drag_anchor = Some(anchor);
        } else if event.click_count == 1 {
            self.buffer.set_cursor(offset);
            self.drag_anchor = Some(offset);
        }
        self.sync_tree_selection_from_cursor();
        cx.notify();
    }

    /// Handles mouse move during dragging to extend active selection.
    pub fn handle_editor_mouse_move(
        &mut self,
        row_idx: usize,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) {
        if let Some(anchor) = self.drag_anchor {
            let mouse_x = f32::from(event.position.x);
            let offset = self.calculate_offset_at_mouse(row_idx, mouse_x);
            if offset != self.buffer.cursor() {
                self.buffer.extend_selection_to(anchor, offset);
                self.sync_tree_selection_from_cursor();
                cx.notify();
            }
        }
    }

    /// Handles mouse release to finalize drag selection.
    pub fn handle_editor_mouse_up(&mut self, cx: &mut Context<Self>) {
        if self.drag_anchor.is_some() {
            self.drag_anchor = None;
            cx.notify();
        }
    }

    /// Handles single, double, and triple clicks for cursor and word/line selections.
    pub fn handle_editor_click(
        &mut self,
        row_idx: usize,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        let ClickEvent::Mouse(m) = event else {
            return;
        };

        let mouse_x = f32::from(m.up.position.x);
        let offset = self.calculate_offset_at_mouse(row_idx, mouse_x);

        match m.down.click_count {
            1 if !m.up.modifiers.shift && self.buffer.selection().is_none() => {
                self.buffer.set_cursor(offset);
            }
            2 => {
                // Double click: word selection
                let (start, end) = self.buffer.word_range_at(offset);
                self.buffer.set_selection(start, end);
                self.drag_anchor = None;
            }
            3.. => {
                // Triple click: entire logical line selection
                if let Some(row) = self.visual_rows.get(row_idx) {
                    let (start, end) = self.buffer.line_range_at(row.logical_line);
                    self.buffer.set_selection(start, end);
                    self.drag_anchor = None;
                }
            }
            _ => {}
        }
        self.sync_tree_selection_from_cursor();
        cx.notify();
    }

    /// Synchronizes tree selection based on current editor cursor position.
    ///
    /// Complexity: O(log N) via pre-computed symbol binary search.
    pub fn sync_tree_selection_from_cursor(&mut self) {
        let cursor = self.buffer.cursor();
        if let Some(sym) = self.tree.find_symbol_at_offset(cursor) {
            let id = sym.id;
            if self.tree.selected_id() != Some(id) {
                self.tree.select_node(Some(id));
                self.tree.scroll_to_selected();
            }
        }
    }

    /// Navigates to the next search match.
    pub fn search_next(&mut self, cx: &mut Context<Self>) {
        if self.search_matches.is_empty() {
            return;
        }
        self.current_match_idx = (self.current_match_idx + 1) % self.search_matches.len();
        self.highlight_current_search_match(cx);
    }

    /// Navigates to the previous search match.
    pub fn search_prev(&mut self, cx: &mut Context<Self>) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.current_match_idx == 0 {
            self.current_match_idx = self.search_matches.len() - 1;
        } else {
            self.current_match_idx -= 1;
        }
        self.highlight_current_search_match(cx);
    }

    /// Updates search matches after query or buffer change.
    ///
    /// Complexity: O(N) where N is document length.
    pub fn update_search(&mut self) {
        self.search_matches = self.buffer.find_matches(&self.search_query);
        self.current_match_idx = 0;
    }

    fn highlight_current_search_match(&mut self, cx: &mut Context<Self>) {
        if let Some(&(start, end)) = self.search_matches.get(self.current_match_idx) {
            self.buffer.set_selection(start, end);
            let (line, _) = self.buffer.offset_to_line_col(start);
            self.scroll_handle
                .scroll_to_item(line.saturating_sub(1), ScrollStrategy::Center);
            self.sync_tree_selection_from_cursor();
            cx.notify();
        }
    }

    /// Central key handler for editor input and global application shortcuts.
    pub fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = event.keystroke.key.as_str();
        let mods = event.keystroke.modifiers;
        let is_ctrl = mods.control || mods.platform;

        // --- Search bar input & navigation isolation when open ---
        if self.is_search_open {
            if is_ctrl {
                match key {
                    "v" => {
                        if let Some(item) = cx.read_from_clipboard() {
                            if let Some(text) = item.text() {
                                let first_line = text.lines().next().unwrap_or("");
                                self.search_query.push_str(first_line);
                                self.update_search();
                                if !self.search_matches.is_empty() {
                                    self.highlight_current_search_match(cx);
                                }
                                cx.notify();
                            }
                        }
                        return;
                    }
                    "c" => {
                        if !self.search_query.is_empty() {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                self.search_query.clone(),
                            ));
                        }
                        return;
                    }
                    "a" => {
                        self.search_query.clear();
                        self.update_search();
                        cx.notify();
                        return;
                    }
                    "f" => {
                        self.is_search_open = false;
                        cx.notify();
                        return;
                    }
                    "s" => {
                        self.save_file(cx);
                        return;
                    }
                    "o" => {
                        self.open_file_dialog(cx);
                        return;
                    }
                    _ => {
                        // Prevent leaking any other Ctrl shortcut into background document
                        return;
                    }
                }
            }

            match key {
                "escape" => {
                    self.is_search_open = false;
                    cx.notify();
                    return;
                }
                "enter" => {
                    if mods.shift {
                        self.search_prev(cx);
                    } else {
                        self.search_next(cx);
                    }
                    return;
                }
                "up" => {
                    self.search_prev(cx);
                    return;
                }
                "down" => {
                    self.search_next(cx);
                    return;
                }
                "left" | "right" | "home" | "end" => {
                    // Prevent arrow / navigation keys from leaking to background document
                    return;
                }
                "backspace" => {
                    self.search_query.pop();
                    self.update_search();
                    if !self.search_matches.is_empty() {
                        self.highlight_current_search_match(cx);
                    }
                    cx.notify();
                    return;
                }
                _ => {
                    // Printable character input into search bar
                    let char_to_insert = if let Some(ref ch_str) = event.keystroke.key_char {
                        if ch_str.chars().count() == 1 {
                            ch_str.chars().next()
                        } else {
                            None
                        }
                    } else if event.keystroke.key.chars().count() == 1 {
                        event.keystroke.key.chars().next()
                    } else {
                        None
                    };

                    if let Some(ch) = char_to_insert {
                        if !ch.is_control() {
                            self.search_query.push(ch);
                            self.update_search();
                            if !self.search_matches.is_empty() {
                                self.highlight_current_search_match(cx);
                            }
                            cx.notify();
                            return;
                        }
                    }
                }
            }
            // Do not fall through to document editing while search bar is active!
            return;
        }

        // --- Global Shortcuts (Ctrl / Cmd + Key) ---
        if is_ctrl {
            match key {
                "s" => {
                    self.save_file(cx);
                    return;
                }
                "o" => {
                    self.open_file_dialog(cx);
                    return;
                }
                "f" if mods.shift => {
                    self.format_document(cx);
                    return;
                }
                "f" => {
                    self.is_search_open = !self.is_search_open;
                    if self.is_search_open {
                        if let Some((s, e)) = self.buffer.selection() {
                            let sel = &self.buffer.text()[s..e];
                            if !sel.contains('\n') && !sel.is_empty() {
                                self.search_query = sel.to_string();
                                self.update_search();
                            }
                        }
                    }
                    cx.notify();
                    return;
                }
                "z" if !mods.shift => {
                    if self.buffer.undo() {
                        self.rebuild_visual_rows();
                        self.is_dirty = true;
                        self.parse_and_sync(cx);
                        self.autoscroll_to_cursor();
                        cx.notify();
                    }
                    return;
                }
                "y" => {
                    if self.buffer.redo() {
                        self.rebuild_visual_rows();
                        self.is_dirty = true;
                        self.parse_and_sync(cx);
                        self.autoscroll_to_cursor();
                        cx.notify();
                    }
                    return;
                }
                "z" if mods.shift => {
                    if self.buffer.redo() {
                        self.rebuild_visual_rows();
                        self.is_dirty = true;
                        self.parse_and_sync(cx);
                        self.autoscroll_to_cursor();
                        cx.notify();
                    }
                    return;
                }
                "a" => {
                    self.buffer.select_all();
                    cx.notify();
                    return;
                }
                "c" if mods.shift => {
                    self.tree.collapse_all();
                    cx.notify();
                    return;
                }
                "c" => {
                    self.copy_all(cx);
                    return;
                }
                "v" => {
                    self.paste_clipboard(cx);
                    return;
                }
                "left" => {
                    self.buffer.move_word_left(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "right" => {
                    self.buffer.move_word_right(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "t" => {
                    self.is_tree_visible = !self.is_tree_visible;
                    cx.notify();
                    return;
                }
                "e" if mods.shift => {
                    self.tree.expand_all();
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }

        // --- Alt Shortcuts (Word Wrap & Symbol Navigation) ---
        if mods.alt {
            match key {
                "z" => {
                    self.toggle_word_wrap(cx);
                    return;
                }
                "down" => {
                    self.jump_next_symbol(cx);
                    return;
                }
                "up" => {
                    self.jump_prev_symbol(cx);
                    return;
                }
                _ => {}
            }
        }

        // --- Escape dismisses diagnostic banner ---
        if key == "escape" && self.diagnostic.is_some() {
            self.diagnostic = None;
            cx.notify();
            return;
        }

        // --- Editor Navigation & Editing Keys ---
        if !is_ctrl && !mods.alt {
            match key {
                "backspace" => {
                    self.buffer.delete_backwards();
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.schedule_debounced_parse(cx);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "delete" => {
                    self.buffer.delete_forwards();
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.schedule_debounced_parse(cx);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "enter" => {
                    self.buffer.insert_newline_auto_indent();
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.schedule_debounced_parse(cx);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "tab" => {
                    self.buffer.insert_str("  ");
                    self.rebuild_visual_rows();
                    self.is_dirty = true;
                    self.schedule_debounced_parse(cx);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "left" => {
                    self.buffer.move_left(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "right" => {
                    self.buffer.move_right(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "up" => {
                    self.buffer.move_up(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "down" => {
                    self.buffer.move_down(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "home" => {
                    self.buffer.move_home(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                "end" => {
                    self.buffer.move_end(mods.shift);
                    self.sync_tree_selection_from_cursor();
                    self.autoscroll_to_cursor();
                    cx.notify();
                    return;
                }
                _ => {}
            }

            // Printable character input
            let char_to_insert = if let Some(ref ch_str) = event.keystroke.key_char {
                if ch_str.chars().count() == 1 {
                    ch_str.chars().next()
                } else {
                    None
                }
            } else if event.keystroke.key.chars().count() == 1 {
                event.keystroke.key.chars().next()
            } else {
                None
            };

            if let Some(ch) = char_to_insert {
                self.buffer.insert_char(ch);
                self.rebuild_visual_rows();
                self.is_dirty = true;
                self.schedule_debounced_parse(cx);
                self.sync_tree_selection_from_cursor();
                self.autoscroll_to_cursor();
                cx.notify();
            }
        }
    }
}

impl Render for AppView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (cursor_line, cursor_col) = self.buffer.offset_to_line_col(self.buffer.cursor());
        let selection_len = self.buffer.selection().map(|(s, e)| e - s);
        let has_content = !self.buffer.text().is_empty();
        let node_count = if has_content {
            Some(self.tree.total_node_count())
        } else {
            None
        };
        let error_line = self.diagnostic.as_ref().map(|d| d.line());
        let file_path_str = self.file_path.as_ref().and_then(|p| p.to_str());

        // Prepare toolbar properties
        let toolbar_props = ToolbarProps {
            is_dirty: self.is_dirty,
            is_tree_visible: self.is_tree_visible,
            is_search_open: self.is_search_open,
            is_word_wrap: self.is_word_wrap,
            has_content,
            is_busy: self.is_loading,
        };

        // Prepare status bar properties
        let status_props = StatusBarProps {
            file_path: file_path_str,
            is_dirty: self.is_dirty,
            status_message: self.status_message.as_deref(),
            line: cursor_line,
            col: cursor_col,
            line_count: self.buffer.line_count(),
            char_count: self.buffer.text().len(),
            selection_len,
            diagnostic: self.diagnostic.as_ref(),
            node_count,
            format_name: self.file_format.name(),
        };

        // Prepare editor properties with allocation-free snapshot
        let editor_props = EditorProps {
            snapshot: self.buffer.snapshot(),
            visual_rows: self.visual_rows.clone(),
            scroll_handle: self.scroll_handle.clone(),
            focus_handle: self.focus_handle.clone(),
            cursor_offset: self.buffer.cursor(),
            cursor_line,
            cursor_col,
            selection: self.buffer.selection(),
            error_line,
            is_search_open: self.is_search_open,
            search_query: &self.search_query,
            match_count: self.search_matches.len(),
            current_match_idx: self.current_match_idx,
            format: self.file_format,
        };

        let view_entity = cx.entity().clone();

        div()
            .id("app_root")
            .size_full()
            .bg(BG_APP)
            .flex()
            .flex_col()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            // Top Toolbar
            .child(render_toolbar(
                toolbar_props,
                cx,
                |this, _event, _window, cx| this.open_file_dialog(cx),
                |this, _event, _window, cx| this.save_file(cx),
                |this, _event, _window, cx| this.format_document(cx),
                |this, _event, _window, cx| this.minify_document(cx),
                |this, _event, _window, cx| this.clear_document(cx),
                |this, _event, _window, cx| this.copy_all(cx),
                |this, _event, _window, cx| this.load_sample(cx),
                |this, _event, _window, cx| {
                    this.is_search_open = !this.is_search_open;
                    cx.notify();
                },
                |this, _event, _window, cx| this.toggle_word_wrap(cx),
                |this, _event, _window, cx| {
                    this.is_tree_visible = !this.is_tree_visible;
                    cx.notify();
                },
            ))
            // Parse Error Diagnostic Banner
            .when_some(self.diagnostic.as_ref(), |this, diag| {
                this.child(render_diagnostics(
                    diag,
                    cx,
                    |this, _event, _window, cx| this.jump_to_error(cx),
                    |this, _event, _window, cx| {
                        this.diagnostic = None;
                        cx.notify();
                    },
                ))
            })
            // Central Content Area (4 UI States: Loading, Empty, Valid/Success, Error)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.0))
                    .w_full()
                    .flex()
                    .flex_row()
                    .overflow_hidden()
                    .when(self.is_loading, |this| {
                        this.child(
                            div()
                                .size_full()
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .text_color(TEXT_MUTED)
                                .child(format!("Loading {} document...", self.file_format.name())),
                        )
                    })
                    .when(!self.is_loading && !has_content, |this| {
                        this.child(render_empty_state(
                            cx,
                            |this, _event, _window, cx| this.open_file_dialog(cx),
                            |this, _event, _window, cx| this.load_sample(cx),
                            |this, _event, _window, cx| this.paste_clipboard(cx),
                        ))
                    })
                    .when(!self.is_loading && has_content, |this| {
                        let view_toggle = view_entity.clone();
                        let view_select = view_entity.clone();
                        let view_mouse_down = view_entity.clone();
                        let view_mouse_move = view_entity.clone();
                        let view_mouse_up = view_entity.clone();
                        let view_click = view_entity.clone();
                        this
                            // Left: Hierarchical Tree View (if enabled)
                            .when(self.is_tree_visible, |content| {
                                content.child(render_tree_view(
                                    &self.tree,
                                    cx,
                                    move |path, _event, _window, cx| {
                                        view_toggle.update(cx, |this, cx| {
                                            this.tree.toggle_expand(&path);
                                            cx.notify();
                                        });
                                    },
                                    move |node_id, span, _event, window, cx| {
                                        view_select.update(cx, |this, cx| {
                                            this.handle_tree_node_selected(
                                                node_id, span, window, cx,
                                            );
                                        });
                                    },
                                    |this, _event, _window, cx| {
                                        this.tree.expand_all();
                                        cx.notify();
                                    },
                                    |this, _event, _window, cx| {
                                        this.tree.collapse_all();
                                        cx.notify();
                                    },
                                    |this, _event, _window, cx| {
                                        this.jump_prev_symbol(cx);
                                    },
                                    |this, _event, _window, cx| {
                                        this.jump_next_symbol(cx);
                                    },
                                ))
                            })
                            // Right: Main Text Editor View
                            .child(div().flex_1().min_w(px(0.0)).h_full().child(render_editor(
                                editor_props,
                                cx,
                                move |row_idx, event, window, cx| {
                                    view_mouse_down.update(cx, |this, cx| {
                                        this.handle_editor_mouse_down(row_idx, event, window, cx);
                                    });
                                },
                                move |row_idx, event, _window, cx| {
                                    view_mouse_move.update(cx, |this, cx| {
                                        this.handle_editor_mouse_move(row_idx, event, cx);
                                    });
                                },
                                move |_row_idx, _event, _window, cx| {
                                    view_mouse_up.update(cx, |this, cx| {
                                        this.handle_editor_mouse_up(cx);
                                    });
                                },
                                move |row_idx, event, window, cx| {
                                    view_click.update(cx, |this, cx| {
                                        this.handle_editor_click(row_idx, event, window, cx);
                                    });
                                },
                                |this, query, _window, cx| {
                                    this.search_query = query;
                                    this.update_search();
                                    cx.notify();
                                },
                                |this, _event, _window, cx| this.search_next(cx),
                                |this, _event, _window, cx| this.search_prev(cx),
                                |this, _event, _window, cx| {
                                    this.is_search_open = false;
                                    cx.notify();
                                },
                            )))
                    }),
            )
            // Bottom Status Bar
            .child(render_status_bar(
                status_props,
                cx,
                |this, _event, _window, cx| this.jump_to_error(cx),
            ))
    }
}
