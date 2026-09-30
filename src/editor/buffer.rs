//! Core text buffer supporting editing, navigation, undo/redo, and search.
//!
//! Maintains an efficient line-start index for O(log L) line/column translation
//! and O(1) single-line slicing without memory allocations.

use std::sync::Arc;

/// An immutable, cheaply-cloneable snapshot of text and line offsets for O(1) allocation-free rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSnapshot {
    text: Arc<str>,
    line_starts: Arc<[usize]>,
}

impl TextSnapshot {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    pub fn line_content(&self, line_index: usize) -> &str {
        if line_index >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line_index];
        let end = if line_index + 1 < self.line_starts.len() {
            let next_start = self.line_starts[line_index + 1];
            if next_start > 0 && self.text.as_bytes().get(next_start - 1) == Some(&b'\n') {
                if next_start > 1 && self.text.as_bytes().get(next_start - 2) == Some(&b'\r') {
                    next_start - 2
                } else {
                    next_start - 1
                }
            } else {
                next_start
            }
        } else {
            self.text.len()
        };
        &self.text[start..end]
    }

    pub fn line_start_offset(&self, line_index: usize) -> usize {
        self.line_starts.get(line_index).copied().unwrap_or(0)
    }

    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let mut clamped = offset.min(self.text.len());
        while clamped > 0 && !self.text.is_char_boundary(clamped) {
            clamped -= 1;
        }
        let line_index = match self.line_starts.binary_search(&clamped) {
            Ok(exact) => exact,
            Err(ins) => ins.saturating_sub(1),
        };

        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let col = self.text[line_start..clamped].chars().count() + 1;
        (line_index + 1, col)
    }

    pub fn line_col_to_offset(&self, line: usize, col: usize) -> usize {
        if line == 0 || self.line_starts.is_empty() {
            return 0;
        }
        let line_index = (line - 1).min(self.line_starts.len() - 1);
        let line_start = self.line_starts[line_index];
        let line_text = self.line_content(line_index);

        let mut byte_advance = 0;
        for (i, ch) in line_text.chars().enumerate() {
            if i + 1 >= col {
                break;
            }
            byte_advance += ch.len_utf8();
        }
        line_start + byte_advance
    }

    /// Returns the byte range `(start, end)` of the line content (0-indexed line).
    ///
    /// Complexity: O(1).
    pub fn line_range_at(&self, line_index: usize) -> (usize, usize) {
        if line_index >= self.line_starts.len() {
            return (0, 0);
        }
        let start = self.line_starts[line_index];
        let end = start + self.line_content(line_index).len();
        (start, end)
    }

    /// Computes the word boundary range `(start_byte, end_byte)` at the given byte offset.
    ///
    /// Complexity: O(W) where W is the word length.
    pub fn word_range_at(&self, offset: usize) -> (usize, usize) {
        word_range_in_text(&self.text, offset)
    }
}

/// Snapshot of the document for undo/redo history.
#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoryEntry {
    text: String,
    cursor: usize,
    selection: Option<(usize, usize)>,
}

/// Text editing buffer.
#[derive(Clone, Debug)]
pub struct TextBuffer {
    text: String,
    cursor: usize,
    selection: Option<(usize, usize)>,
    line_starts: Arc<[usize]>,
    cached_text: Arc<str>,
    history: Vec<HistoryEntry>,
    history_index: usize,
    last_action_was_char_insert: bool,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new("")
    }
}

impl TextBuffer {
    /// Creates a new buffer initialized with the given text.
    ///
    /// Complexity: O(N) where N is the length of the string.
    pub fn new(initial_text: &str) -> Self {
        let mut buffer = Self {
            text: initial_text.to_string(),
            cursor: 0,
            selection: None,
            line_starts: Arc::new([0]),
            cached_text: Arc::from(initial_text),
            history: Vec::new(),
            history_index: 0,
            last_action_was_char_insert: false,
        };
        buffer.rebuild_line_index();
        buffer.history.push(HistoryEntry {
            text: buffer.text.clone(),
            cursor: 0,
            selection: None,
        });
        buffer
    }

    /// Returns an immutable, O(1) cheaply-cloned snapshot for rendering.
    ///
    /// Complexity: O(1).
    pub fn snapshot(&self) -> TextSnapshot {
        TextSnapshot {
            text: self.cached_text.clone(),
            line_starts: self.line_starts.clone(),
        }
    }

    /// Returns a reference to the full document text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Sets the entire buffer content, resetting undo history.
    ///
    /// Complexity: O(N).
    pub fn set_text(&mut self, new_text: &str) {
        self.text = new_text.to_string();
        self.cursor = 0;
        self.selection = None;
        self.last_action_was_char_insert = false;
        self.rebuild_line_index();
        self.history.clear();
        self.history.push(HistoryEntry {
            text: self.text.clone(),
            cursor: 0,
            selection: None,
        });
        self.history_index = 0;
    }

    /// Current cursor byte offset.
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Sets the cursor to a specific byte offset, clearing selection.
    ///
    /// Clamps to valid character boundary within document bounds.
    pub fn set_cursor(&mut self, offset: usize) {
        self.cursor = self.clamp_to_char_boundary(offset);
        self.selection = None;
        self.last_action_was_char_insert = false;
    }

    /// Returns the active selection range (start_byte, end_byte) ordered.
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection
            .map(|(a, b)| if a < b { (a, b) } else { (b, a) })
    }

    /// Sets the selection range.
    pub fn set_selection(&mut self, start: usize, end: usize) {
        let a = self.clamp_to_char_boundary(start);
        let b = self.clamp_to_char_boundary(end);
        if a == b {
            self.cursor = a;
            self.selection = None;
        } else {
            self.cursor = b;
            self.selection = Some((a, b));
        }
        self.last_action_was_char_insert = false;
    }

    /// Selects all text in the document.
    pub fn select_all(&mut self) {
        if !self.text.is_empty() {
            self.selection = Some((0, self.text.len()));
            self.cursor = self.text.len();
        }
    }

    /// Clears any active selection.
    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    /// Returns the total number of lines.
    ///
    /// Complexity: O(1).
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Returns the text slice of the specified line (0-indexed).
    ///
    /// Complexity: O(1).
    pub fn line_content(&self, line_index: usize) -> &str {
        if line_index >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line_index];
        let end = if line_index + 1 < self.line_starts.len() {
            let next_start = self.line_starts[line_index + 1];
            // Exclude trailing '\n' or '\r\n'
            if next_start > 0 && self.text.as_bytes().get(next_start - 1) == Some(&b'\n') {
                if next_start > 1 && self.text.as_bytes().get(next_start - 2) == Some(&b'\r') {
                    next_start - 2
                } else {
                    next_start - 1
                }
            } else {
                next_start
            }
        } else {
            self.text.len()
        };
        &self.text[start..end]
    }

    /// Returns the byte range `(start, end)` of the line content (0-indexed line).
    ///
    /// Complexity: O(1).
    pub fn line_range_at(&self, line_index: usize) -> (usize, usize) {
        if line_index >= self.line_starts.len() {
            return (0, 0);
        }
        let start = self.line_starts[line_index];
        let end = start + self.line_content(line_index).len();
        (start, end)
    }

    /// Computes the word boundary range `(start_byte, end_byte)` at the given byte offset.
    ///
    /// Complexity: O(W) where W is the word length.
    pub fn word_range_at(&self, offset: usize) -> (usize, usize) {
        word_range_in_text(&self.text, offset)
    }

    /// Extends selection from an anchor byte offset to a target byte offset.
    ///
    /// Sets cursor to target and selection to `(min(anchor, target), max(anchor, target))`.
    pub fn extend_selection_to(&mut self, anchor: usize, target: usize) {
        let a = self.clamp_to_char_boundary(anchor);
        let b = self.clamp_to_char_boundary(target);
        self.cursor = b;
        if a == b {
            self.selection = None;
        } else {
            self.selection = Some((a.min(b), a.max(b)));
        }
        self.last_action_was_char_insert = false;
    }

    /// Computes (line, column) for a given byte offset (1-indexed).
    ///
    /// Complexity: O(log L) where L is the number of lines.
    pub fn offset_to_line_col(&self, offset: usize) -> (usize, usize) {
        let clamped = self.clamp_to_char_boundary(offset);
        let line_index = match self.line_starts.binary_search(&clamped) {
            Ok(exact) => exact,
            Err(ins) => ins.saturating_sub(1),
        };

        let line_start = self.line_starts.get(line_index).copied().unwrap_or(0);
        let col = self.text[line_start..clamped].chars().count() + 1;
        (line_index + 1, col)
    }

    /// Computes byte offset from 1-indexed (line, column).
    ///
    /// Complexity: O(C) where C is the column count.
    pub fn line_col_to_offset(&self, line: usize, col: usize) -> usize {
        if line == 0 || self.line_starts.is_empty() {
            return 0;
        }
        let line_index = (line - 1).min(self.line_starts.len() - 1);
        let line_start = self.line_starts[line_index];
        let line_text = self.line_content(line_index);

        let mut byte_advance = 0;
        for (i, ch) in line_text.chars().enumerate() {
            if i + 1 >= col {
                break;
            }
            byte_advance += ch.len_utf8();
        }
        line_start + byte_advance
    }

    /// Inserts a string at current selection or cursor.
    ///
    /// Complexity: O(N) where N is the document length.
    pub fn insert_str(&mut self, text_to_insert: &str) {
        self.delete_selected_text_internal();

        let insert_point = self.cursor;
        self.text.insert_str(insert_point, text_to_insert);
        self.cursor += text_to_insert.len();
        self.selection = None;
        self.rebuild_line_index();
        self.push_snapshot();
        self.last_action_was_char_insert = false;
    }

    /// Inserts a single character, coalescing consecutive keystrokes into undo chunks.
    pub fn insert_char(&mut self, ch: char) {
        self.delete_selected_text_internal();

        let mut buf = [0u8; 4];
        let str_slice = ch.encode_utf8(&mut buf);
        self.text.insert_str(self.cursor, str_slice);
        self.cursor += str_slice.len();
        self.selection = None;
        self.rebuild_line_index();

        if self.last_action_was_char_insert && self.history_index > 0 {
            // Update top snapshot for coalescing
            if let Some(top) = self.history.last_mut() {
                top.text = self.text.clone();
                top.cursor = self.cursor;
                top.selection = self.selection;
            }
        } else {
            self.push_snapshot();
            self.last_action_was_char_insert = true;
        }
    }

    /// Deletes text backwards (Backspace key behavior).
    pub fn delete_backwards(&mut self) {
        if self.selection.is_some() {
            self.delete_selected_text_internal();
            self.rebuild_line_index();
            self.push_snapshot();
            self.last_action_was_char_insert = false;
            return;
        }

        if self.cursor == 0 {
            return;
        }

        let prev_char_boundary = self.prev_char_boundary(self.cursor);
        self.text.drain(prev_char_boundary..self.cursor);
        self.cursor = prev_char_boundary;
        self.rebuild_line_index();
        self.push_snapshot();
        self.last_action_was_char_insert = false;
    }

    /// Deletes text forwards (Delete key behavior).
    pub fn delete_forwards(&mut self) {
        if self.selection.is_some() {
            self.delete_selected_text_internal();
            self.rebuild_line_index();
            self.push_snapshot();
            self.last_action_was_char_insert = false;
            return;
        }

        if self.cursor >= self.text.len() {
            return;
        }

        let next_char_boundary = self.next_char_boundary(self.cursor);
        self.text.drain(self.cursor..next_char_boundary);
        self.rebuild_line_index();
        self.push_snapshot();
        self.last_action_was_char_insert = false;
    }

    /// Inserts a newline with automatic indentation matching the preceding line.
    pub fn insert_newline_auto_indent(&mut self) {
        self.delete_selected_text_internal();

        let (current_line, _) = self.offset_to_line_col(self.cursor);
        let line_text = self.line_content(current_line - 1);

        // Count leading spaces of current line
        let leading_spaces = line_text.chars().take_while(|c| *c == ' ').count();
        let trimmed_line = line_text.trim_end();
        let opens_block = trimmed_line.ends_with('{') || trimmed_line.ends_with('[');
        let extra_indent = if opens_block { 2 } else { 0 };

        let total_indent = leading_spaces + extra_indent;
        let indent_str = format!("\n{}", " ".repeat(total_indent));
        self.text.insert_str(self.cursor, &indent_str);
        self.cursor += indent_str.len();
        self.rebuild_line_index();
        self.push_snapshot();
        self.last_action_was_char_insert = false;
    }

    /// Navigates cursor one character to the left.
    pub fn move_left(&mut self, extend_selection: bool) {
        let prev = self.prev_char_boundary(self.cursor);
        self.update_cursor_motion(prev, extend_selection);
    }

    /// Navigates cursor one character to the right.
    pub fn move_right(&mut self, extend_selection: bool) {
        let next = self.next_char_boundary(self.cursor);
        self.update_cursor_motion(next, extend_selection);
    }

    /// Navigates cursor one line up, preserving visual column.
    pub fn move_up(&mut self, extend_selection: bool) {
        let (line, col) = self.offset_to_line_col(self.cursor);
        if line > 1 {
            let target = self.line_col_to_offset(line - 1, col);
            self.update_cursor_motion(target, extend_selection);
        } else {
            self.update_cursor_motion(0, extend_selection);
        }
    }

    /// Navigates cursor one line down, preserving visual column.
    pub fn move_down(&mut self, extend_selection: bool) {
        let (line, col) = self.offset_to_line_col(self.cursor);
        if line < self.line_count() {
            let target = self.line_col_to_offset(line + 1, col);
            self.update_cursor_motion(target, extend_selection);
        } else {
            self.update_cursor_motion(self.text.len(), extend_selection);
        }
    }

    /// Navigates to the beginning of the line (or document if already at line start).
    pub fn move_home(&mut self, extend_selection: bool) {
        let (line, _) = self.offset_to_line_col(self.cursor);
        let line_start = self.line_starts[line - 1];
        let line_text = self.line_content(line - 1);
        let first_non_space = line_text.find(|c: char| !c.is_whitespace()).unwrap_or(0);
        let smart_home = line_start + first_non_space;

        let target = if self.cursor == smart_home {
            line_start
        } else {
            smart_home
        };
        self.update_cursor_motion(target, extend_selection);
    }

    /// Navigates to the end of the line.
    pub fn move_end(&mut self, extend_selection: bool) {
        let (line, _) = self.offset_to_line_col(self.cursor);
        let line_content = self.line_content(line - 1);
        let line_start = self.line_starts[line - 1];
        let target = line_start + line_content.len();
        self.update_cursor_motion(target, extend_selection);
    }

    /// Moves cursor backwards by one word boundary, handling Unicode characters.
    pub fn move_word_left(&mut self, extend_selection: bool) {
        let mut idx = self.cursor;

        // Skip whitespace backwards
        while idx > 0 {
            let prev = self.prev_char_boundary(idx);
            let ch = self.text[prev..idx].chars().next().unwrap_or(' ');
            if ch.is_whitespace() {
                idx = prev;
            } else {
                break;
            }
        }

        // Skip contiguous word characters or punctuation backwards
        if idx > 0 {
            let prev = self.prev_char_boundary(idx);
            let ch = self.text[prev..idx].chars().next().unwrap_or(' ');
            let target_is_word = is_word_char(ch);
            while idx > 0 {
                let prev = self.prev_char_boundary(idx);
                let c = self.text[prev..idx].chars().next().unwrap_or(' ');
                if !c.is_whitespace() && (is_word_char(c) == target_is_word) {
                    idx = prev;
                } else {
                    break;
                }
            }
        }

        idx = self.clamp_to_char_boundary(idx);
        self.update_cursor_motion(idx, extend_selection);
    }

    /// Moves cursor forwards by one word boundary, handling Unicode characters.
    pub fn move_word_right(&mut self, extend_selection: bool) {
        let mut idx = self.cursor;
        let len = self.text.len();

        // Skip current word or punctuation block forwards
        if idx < len {
            let next = self.next_char_boundary(idx);
            let ch = self.text[idx..next].chars().next().unwrap_or(' ');
            if !ch.is_whitespace() {
                let target_is_word = is_word_char(ch);
                while idx < len {
                    let next = self.next_char_boundary(idx);
                    let c = self.text[idx..next].chars().next().unwrap_or(' ');
                    if !c.is_whitespace() && (is_word_char(c) == target_is_word) {
                        idx = next;
                    } else {
                        break;
                    }
                }
            }
        }

        // Skip trailing whitespace forwards
        while idx < len {
            let next = self.next_char_boundary(idx);
            let ch = self.text[idx..next].chars().next().unwrap_or(' ');
            if ch.is_whitespace() {
                idx = next;
            } else {
                break;
            }
        }

        idx = self.clamp_to_char_boundary(idx);
        self.update_cursor_motion(idx, extend_selection);
    }

    /// Restores previous document state from undo history.
    pub fn undo(&mut self) -> bool {
        if self.history_index > 0 {
            self.history_index -= 1;
            let snapshot = &self.history[self.history_index];
            self.text = snapshot.text.clone();
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
            self.last_action_was_char_insert = false;
            self.rebuild_line_index();
            true
        } else {
            false
        }
    }

    /// Re-applies undone state from redo history.
    pub fn redo(&mut self) -> bool {
        if self.history_index + 1 < self.history.len() {
            self.history_index += 1;
            let snapshot = &self.history[self.history_index];
            self.text = snapshot.text.clone();
            self.cursor = snapshot.cursor;
            self.selection = snapshot.selection;
            self.last_action_was_char_insert = false;
            self.rebuild_line_index();
            true
        } else {
            false
        }
    }

    /// Finds all matching ranges for a search query safely aligned to character boundaries.
    ///
    /// Complexity: O(N * M) where N is document char count and M is query length.
    pub fn find_matches(&self, query: &str) -> Vec<(usize, usize)> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_chars: Vec<char> = query.chars().collect();

        let mut matches = Vec::new();
        for (start_byte, _) in self.text.char_indices() {
            let remaining = &self.text[start_byte..];
            let mut text_chars_iter = remaining.chars();
            let mut matched = true;
            let mut match_byte_len = 0;

            for &qc in &query_chars {
                if let Some(tc) = text_chars_iter.next() {
                    if !tc.to_lowercase().eq(qc.to_lowercase()) {
                        matched = false;
                        break;
                    }
                    match_byte_len += tc.len_utf8();
                } else {
                    matched = false;
                    break;
                }
            }

            if matched {
                matches.push((start_byte, start_byte + match_byte_len));
            }
        }
        matches
    }

    // --- Private Helper Methods ---

    fn update_cursor_motion(&mut self, new_cursor: usize, extend_selection: bool) {
        if extend_selection {
            let anchor = match self.selection {
                Some((a, _)) => a,
                None => self.cursor,
            };
            self.cursor = new_cursor;
            if anchor != new_cursor {
                self.selection = Some((anchor, new_cursor));
            } else {
                self.selection = None;
            }
        } else {
            self.cursor = new_cursor;
            self.selection = None;
        }
        self.last_action_was_char_insert = false;
    }

    fn delete_selected_text_internal(&mut self) {
        if let Some((start, end)) = self.selection() {
            self.text.drain(start..end);
            self.cursor = start;
            self.selection = None;
        }
    }

    fn push_snapshot(&mut self) {
        if self.history_index + 1 < self.history.len() {
            self.history.truncate(self.history_index + 1);
        }

        // Bound history to at most 100 snapshots
        if self.history.len() >= 100 {
            self.history.remove(0);
            self.history_index = self.history_index.saturating_sub(1);
        }

        self.history.push(HistoryEntry {
            text: self.text.clone(),
            cursor: self.cursor,
            selection: self.selection,
        });
        self.history_index = self.history.len() - 1;
    }

    /// Rebuilds the line start byte index.
    ///
    /// Complexity: O(N) where N is document length.
    fn rebuild_line_index(&mut self) {
        let mut starts = Vec::new();
        starts.push(0);

        for (offset, ch) in self.text.char_indices() {
            if ch == '\n' {
                starts.push(offset + 1);
            }
        }
        self.line_starts = starts.into();
        self.cached_text = self.text.as_str().into();
    }

    fn clamp_to_char_boundary(&self, mut offset: usize) -> usize {
        offset = offset.min(self.text.len());
        while offset > 0 && !self.text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    fn prev_char_boundary(&self, offset: usize) -> usize {
        if offset == 0 {
            return 0;
        }
        let mut idx = offset - 1;
        while idx > 0 && !self.text.is_char_boundary(idx) {
            idx -= 1;
        }
        idx
    }

    fn next_char_boundary(&self, offset: usize) -> usize {
        if offset >= self.text.len() {
            return self.text.len();
        }
        let mut idx = offset + 1;
        while idx < self.text.len() && !self.text.is_char_boundary(idx) {
            idx += 1;
        }
        idx
    }
}

pub fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Computes the word boundary range `(start_byte, end_byte)` at the given byte offset in `text`.
///
/// If offset is on a word char (`is_word_char`), expands left/right across all adjacent word chars.
/// If offset is at a word end (e.g. right before delimiter or whitespace), selects the preceding word.
/// If on whitespace, expands across contiguous horizontal whitespace.
/// If on punctuation/delimiter, returns the delimiter or contiguous sequence of identical punctuation.
///
/// Complexity: O(W) where W is word length.
pub fn word_range_in_text(text: &str, mut offset: usize) -> (usize, usize) {
    if text.is_empty() {
        return (0, 0);
    }

    // Clamp offset to char boundary within document
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }

    // If offset is at the end or on a non-word char, check if preceding char is a word char
    if offset == text.len() && offset > 0 {
        let prev = prev_char_boundary_in_text(text, offset);
        let prev_ch = text[prev..offset].chars().next().unwrap_or(' ');
        if is_word_char(prev_ch) {
            offset = prev;
        }
    } else if offset < text.len() {
        let curr_ch = text[offset..].chars().next().unwrap_or(' ');
        if !is_word_char(curr_ch) && offset > 0 {
            let prev = prev_char_boundary_in_text(text, offset);
            let prev_ch = text[prev..offset].chars().next().unwrap_or(' ');
            if is_word_char(prev_ch) {
                offset = prev;
            }
        }
    }

    if offset >= text.len() {
        return (text.len(), text.len());
    }

    let ch = text[offset..].chars().next().unwrap_or(' ');
    if is_word_char(ch) {
        // Expand left
        let mut start = offset;
        while start > 0 {
            let prev = prev_char_boundary_in_text(text, start);
            let prev_ch = text[prev..start].chars().next().unwrap_or(' ');
            if is_word_char(prev_ch) {
                start = prev;
            } else {
                break;
            }
        }

        // Expand right
        let mut end = offset + ch.len_utf8();
        while end < text.len() {
            let next_ch = text[end..].chars().next().unwrap_or(' ');
            if is_word_char(next_ch) {
                end += next_ch.len_utf8();
            } else {
                break;
            }
        }
        (start, end)
    } else if ch.is_whitespace() {
        // Expand horizontal whitespace only (do not cross newlines)
        if ch == '\n' || ch == '\r' {
            return (offset, offset + ch.len_utf8());
        }
        let mut start = offset;
        while start > 0 {
            let prev = prev_char_boundary_in_text(text, start);
            let prev_ch = text[prev..start].chars().next().unwrap_or(' ');
            if prev_ch.is_whitespace() && prev_ch != '\n' && prev_ch != '\r' {
                start = prev;
            } else {
                break;
            }
        }
        let mut end = offset + ch.len_utf8();
        while end < text.len() {
            let next_ch = text[end..].chars().next().unwrap_or(' ');
            if next_ch.is_whitespace() && next_ch != '\n' && next_ch != '\r' {
                end += next_ch.len_utf8();
            } else {
                break;
            }
        }
        (start, end)
    } else {
        // Punctuation or delimiter: select identical contiguous characters (e.g. "==")
        let mut start = offset;
        while start > 0 {
            let prev = prev_char_boundary_in_text(text, start);
            let prev_ch = text[prev..start].chars().next().unwrap_or(' ');
            if prev_ch == ch {
                start = prev;
            } else {
                break;
            }
        }
        let mut end = offset + ch.len_utf8();
        while end < text.len() {
            let next_ch = text[end..].chars().next().unwrap_or(' ');
            if next_ch == ch {
                end += next_ch.len_utf8();
            } else {
                break;
            }
        }
        (start, end)
    }
}

fn prev_char_boundary_in_text(text: &str, offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut idx = offset - 1;
    while idx > 0 && !text.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// Computes the character column index (0-indexed) within a line from relative horizontal pixels.
///
/// Complexity: O(C) where C is the number of characters in the line.
pub fn char_index_at_x(line_text: &str, relative_x: f32, char_width: f32) -> usize {
    if relative_x <= 0.0 || char_width <= 0.0 {
        return 0;
    }
    let total_chars = line_text.chars().count();
    let approximate_idx = (relative_x / char_width).round() as usize;
    approximate_idx.min(total_chars)
}

/// Converts a 0-indexed character index within `line_text` to its byte advance.
///
/// Complexity: O(C) where C is the character index.
pub fn char_index_to_byte_offset(line_text: &str, char_index: usize) -> usize {
    let mut byte_offset = 0;
    for (i, ch) in line_text.chars().enumerate() {
        if i >= char_index {
            break;
        }
        byte_offset += ch.len_utf8();
    }
    byte_offset
}
