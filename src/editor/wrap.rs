//! Visual word wrap and row layout computation for editor virtualization.
//!
//! Maps logical lines into one or more `VisualRow` elements of uniform height,
//! enabling smooth, high-FPS virtualized rendering without horizontal overflow.

use super::buffer::TextSnapshot;
use std::sync::Arc;

/// A discrete, uniform-height visual row rendered by the virtual list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VisualRow {
    /// 0-indexed logical line in the document buffer.
    pub logical_line: usize,
    /// 0-indexed sub-line index within the logical line (0 for first row, 1+ for wrapped continuation).
    pub sub_line: usize,
    /// Absolute start byte offset within the full document (inclusive).
    pub start_byte: usize,
    /// Absolute end byte offset within the full document (exclusive).
    pub end_byte: usize,
}

/// Computes visual rows from a text snapshot with optional word wrapping.
///
/// Long lines are cleanly wrapped at whitespace or delimiter boundaries when available,
/// or hard-wrapped at `wrap_column` for unbroken tokens.
///
/// Complexity: O(N) where N is document length in bytes.
pub fn compute_visual_rows(
    snapshot: &TextSnapshot,
    wrap_enabled: bool,
    wrap_column: usize,
) -> Arc<[VisualRow]> {
    let line_count = snapshot.line_count();
    if line_count == 0 {
        return Arc::new([]);
    }

    let wrap_col = if wrap_column < 20 { 20 } else { wrap_column };
    let mut rows = Vec::with_capacity(line_count);

    for line_idx in 0..line_count {
        let content = snapshot.line_content(line_idx);
        let line_start = snapshot.line_start_offset(line_idx);

        // Fast O(1) check: UTF-8 char count is always <= byte length
        if !wrap_enabled || content.len() <= wrap_col || content.chars().count() <= wrap_col {
            rows.push(VisualRow {
                logical_line: line_idx,
                sub_line: 0,
                start_byte: line_start,
                end_byte: line_start + content.len(),
            });
        } else {
            // Line requires wrapping into multiple visual rows
            wrap_single_line(line_idx, line_start, content, wrap_col, &mut rows);
        }
    }

    rows.into()
}

/// Wraps a single logical line into multiple `VisualRow` chunks.
///
/// Complexity: O(L) where L is the character count of `content`.
fn wrap_single_line(
    line_idx: usize,
    line_start_byte: usize,
    content: &str,
    wrap_col: usize,
    out: &mut Vec<VisualRow>,
) {
    if content.is_ascii() {
        wrap_single_ascii_line(line_idx, line_start_byte, content.as_bytes(), wrap_col, out);
    } else {
        wrap_single_unicode_line(line_idx, line_start_byte, content, wrap_col, out);
    }
}

/// Allocation-free fast path for wrapping ASCII lines (e.g. minified 50k+ char JSON).
///
/// Complexity: O(L) where L is byte length.
fn wrap_single_ascii_line(
    line_idx: usize,
    line_start_byte: usize,
    bytes: &[u8],
    wrap_col: usize,
    out: &mut Vec<VisualRow>,
) {
    let total_len = bytes.len();
    let mut cursor = 0;
    let mut sub_line = 0;
    let min_break = (wrap_col * 65 / 100).max(1);

    while cursor < total_len {
        let remaining = total_len - cursor;
        if remaining <= wrap_col {
            out.push(VisualRow {
                logical_line: line_idx,
                sub_line,
                start_byte: line_start_byte + cursor,
                end_byte: line_start_byte + total_len,
            });
            break;
        }

        let max_chunk = wrap_col.min(remaining);
        let mut break_offset = None;

        for offset in (min_break..max_chunk).rev() {
            let b = bytes[cursor + offset];
            if b.is_ascii_whitespace() || is_ascii_wrap_delimiter(b) {
                break_offset = Some(offset + 1);
                break;
            }
        }

        let chunk_len = break_offset.unwrap_or(max_chunk);
        let next_cursor = cursor + chunk_len;

        out.push(VisualRow {
            logical_line: line_idx,
            sub_line,
            start_byte: line_start_byte + cursor,
            end_byte: line_start_byte + next_cursor,
        });

        cursor = next_cursor;
        sub_line += 1;
    }
}

/// Unicode fallback path for lines containing multi-byte UTF-8 characters.
///
/// Complexity: O(L) where L is character count.
fn wrap_single_unicode_line(
    line_idx: usize,
    line_start_byte: usize,
    content: &str,
    wrap_col: usize,
    out: &mut Vec<VisualRow>,
) {
    let mut current_byte_offset = 0;
    let mut sub_line = 0;

    let chars: Vec<(usize, char)> = content.char_indices().collect();
    let total_chars = chars.len();
    let mut char_cursor = 0;
    let min_break = (wrap_col * 65 / 100).max(1);

    while char_cursor < total_chars {
        let remaining_chars = total_chars - char_cursor;

        if remaining_chars <= wrap_col {
            let row_start_byte = line_start_byte + current_byte_offset;
            let row_end_byte = line_start_byte + content.len();
            out.push(VisualRow {
                logical_line: line_idx,
                sub_line,
                start_byte: row_start_byte,
                end_byte: row_end_byte,
            });
            break;
        }

        let max_chunk = wrap_col.min(remaining_chars);
        let mut break_char_offset = None;

        for offset in (min_break..max_chunk).rev() {
            let (_, ch) = chars[char_cursor + offset];
            if ch.is_whitespace() || is_wrap_delimiter(ch) {
                break_char_offset = Some(offset + 1);
                break;
            }
        }

        let chunk_char_len = break_char_offset.unwrap_or(max_chunk);
        let chunk_end_char = char_cursor + chunk_char_len;

        let chunk_end_byte = if chunk_end_char < total_chars {
            chars[chunk_end_char].0
        } else {
            content.len()
        };

        let row_start_byte = line_start_byte + current_byte_offset;
        let row_end_byte = line_start_byte + chunk_end_byte;

        out.push(VisualRow {
            logical_line: line_idx,
            sub_line,
            start_byte: row_start_byte,
            end_byte: row_end_byte,
        });

        current_byte_offset = chunk_end_byte;
        char_cursor = chunk_end_char;
        sub_line += 1;
    }
}

/// Finds the index of the `VisualRow` that contains the given byte offset.
///
/// Complexity: O(log R) where R is the number of visual rows.
pub fn find_visual_row_by_offset(rows: &[VisualRow], offset: usize) -> usize {
    if rows.is_empty() {
        return 0;
    }
    match rows.binary_search_by(|row| {
        if offset < row.start_byte {
            std::cmp::Ordering::Greater
        } else if offset >= row.end_byte && row.end_byte > row.start_byte {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Equal
        }
    }) {
        Ok(idx) => idx,
        Err(idx) => idx.min(rows.len() - 1),
    }
}

fn is_ascii_wrap_delimiter(b: u8) -> bool {
    matches!(
        b,
        b',' | b':' | b';' | b'{' | b'}' | b'[' | b']' | b')' | b'(' | b'>'
    )
}

fn is_wrap_delimiter(ch: char) -> bool {
    matches!(
        ch,
        ',' | ':' | ';' | '{' | '}' | '[' | ']' | ')' | '(' | '>'
    )
}
