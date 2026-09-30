use file_viewer::editor::TextBuffer;

#[test]
fn test_buffer_creation_and_line_index() {
    let buf = TextBuffer::new("hello\nworld\njson");
    assert_eq!(buf.line_count(), 3);
    assert_eq!(buf.line_content(0), "hello");
    assert_eq!(buf.line_content(1), "world");
    assert_eq!(buf.line_content(2), "json");

    assert_eq!(buf.offset_to_line_col(0), (1, 1));
    assert_eq!(buf.offset_to_line_col(5), (1, 6));
    assert_eq!(buf.offset_to_line_col(6), (2, 1)); // 'w' in "world"
    assert_eq!(buf.line_col_to_offset(2, 1), 6);
}

#[test]
fn test_insert_and_delete() {
    let mut buf = TextBuffer::new("ac");
    buf.set_cursor(1);
    buf.insert_char('b');
    assert_eq!(buf.text(), "abc");
    assert_eq!(buf.cursor(), 2);

    buf.delete_backwards();
    assert_eq!(buf.text(), "ac");
    assert_eq!(buf.cursor(), 1);

    buf.delete_forwards();
    assert_eq!(buf.text(), "a");
    assert_eq!(buf.cursor(), 1);
}

#[test]
fn test_selection_replacement() {
    let mut buf = TextBuffer::new("hello world");
    buf.set_selection(6, 11);
    assert_eq!(buf.selection(), Some((6, 11)));

    buf.insert_str("Rust");
    assert_eq!(buf.text(), "hello Rust");
    assert_eq!(buf.selection(), None);
}

#[test]
fn test_undo_and_redo() {
    let mut buf = TextBuffer::new("initial");
    buf.set_cursor(7);
    buf.insert_str(" text");
    assert_eq!(buf.text(), "initial text");

    assert!(buf.undo());
    assert_eq!(buf.text(), "initial");

    assert!(buf.redo());
    assert_eq!(buf.text(), "initial text");
}

#[test]
fn test_search_matches() {
    let buf = TextBuffer::new(r#"{"key1": "value", "key2": "value"}"#);
    let matches = buf.find_matches("value");
    assert_eq!(matches.len(), 2);
    assert_eq!(&buf.text()[matches[0].0..matches[0].1], "value");
    assert_eq!(&buf.text()[matches[1].0..matches[1].1], "value");
}

#[test]
fn test_auto_indent_newline() {
    let mut buf = TextBuffer::new("  {\n  }");
    buf.set_cursor(3); // after '{'
    buf.insert_newline_auto_indent();
    // 2 leading spaces + 2 extra for block = 4 spaces indent
    assert!(buf.text().starts_with("  {\n    \n  }"));
}

#[test]
fn test_case_insensitive_search() {
    let buf = TextBuffer::new(r#"{"Name": "Alice", "name": "Bob", "NAME": "Charlie"}"#);
    let matches = buf.find_matches("name");
    assert_eq!(matches.len(), 3);
    assert_eq!(&buf.text()[matches[0].0..matches[0].1], "Name");
    assert_eq!(&buf.text()[matches[1].0..matches[1].1], "name");
    assert_eq!(&buf.text()[matches[2].0..matches[2].1], "NAME");

    let matches_upper = buf.find_matches("NAME");
    assert_eq!(matches_upper.len(), 3);
}

#[test]
fn test_text_snapshot() {
    let mut buf = TextBuffer::new("line one\nline two\nline three");
    let snap = buf.snapshot();

    assert_eq!(snap.line_count(), 3);
    assert_eq!(snap.line_content(1), "line two");
    assert_eq!(snap.offset_to_line_col(9), (2, 1)); // 'l' in "line two"
    assert_eq!(snap.text(), "line one\nline two\nline three");

    // Mutate buffer, snapshot should remain unchanged
    buf.insert_str("prepended\n");
    assert_eq!(snap.line_count(), 3);
    assert_eq!(snap.line_content(0), "line one");
    assert_eq!(buf.line_count(), 4);
}

#[test]
fn test_offset_to_line_col_utf8_boundary_safety() {
    let text = "🦀 café 🚀\nsecond line";
    let buf = TextBuffer::new(text);
    let snap = buf.snapshot();

    // '🦀' is 4 bytes (indices 0..4). Offsets 1, 2, 3 are invalid UTF-8 char boundaries.
    for mid_byte in 1..4 {
        // Both buffer and snapshot must not panic and return valid line/column
        let (buf_line, buf_col) = buf.offset_to_line_col(mid_byte);
        let (snap_line, snap_col) = snap.offset_to_line_col(mid_byte);
        assert_eq!(buf_line, 1);
        assert_eq!(snap_line, 1);
        assert!(buf_col >= 1);
        assert!(snap_col >= 1);
    }

    // 'é' in café is 2 bytes. Offset into 'é' must not panic.
    let cafe_offset = text.find("café").unwrap();
    let mid_e_offset = cafe_offset + "caf".len() + 1; // inside 'é'
    let (buf_line, _) = buf.offset_to_line_col(mid_e_offset);
    let (snap_line, _) = snap.offset_to_line_col(mid_e_offset);
    assert_eq!(buf_line, 1);
    assert_eq!(snap_line, 1);

    // Beyond length offset clamps safely
    let (buf_line, _) = buf.offset_to_line_col(1000);
    let (snap_line, _) = snap.offset_to_line_col(1000);
    assert_eq!(buf_line, 2);
    assert_eq!(snap_line, 2);
}

#[test]
fn test_unicode_word_navigation() {
    let mut buf = TextBuffer::new("café_au_lait 123 π_value");

    // Start at beginning
    buf.set_cursor(0);

    // Move word right across alphanumeric/underscore word with unicode to next word start
    buf.move_word_right(false);
    let first_word_next = "café_au_lait ".len(); // 13 + 1 = 14
    assert_eq!(buf.cursor(), first_word_next);

    // Move word right past "123" to start of "π_value"
    buf.move_word_right(false);
    let second_word_next = first_word_next + "123 ".len(); // 14 + 4 = 18
    assert_eq!(buf.cursor(), second_word_next);

    // Move word right past "π_value" to end of text
    buf.move_word_right(false);
    assert_eq!(buf.cursor(), buf.text().len());

    // Move word left back to start of "π_value"
    buf.move_word_left(false);
    let third_word_start = buf.text().find("π_value").unwrap();
    assert_eq!(buf.cursor(), third_word_start);

    // Move word left to start of "123"
    buf.move_word_left(false);
    let second_word_start = buf.text().find("123").unwrap();
    assert_eq!(buf.cursor(), second_word_start);

    // Move word left to start of "café_au_lait"
    buf.move_word_left(false);
    assert_eq!(buf.cursor(), 0);
}

#[test]
fn test_word_range_at_json_keys_and_values() {
    let buf = TextBuffer::new("  \"auto_save\": true,");

    // "auto_save" is at offsets 3..12
    let key_offset = buf.text().find("auto_save").unwrap();
    let (s, e) = buf.word_range_at(key_offset);
    assert_eq!(&buf.text()[s..e], "auto_save");

    // Double clicking middle of "auto_save" (e.g. at 's')
    let s_offset = buf.text().find("_save").unwrap();
    let (s, e) = buf.word_range_at(s_offset);
    assert_eq!(&buf.text()[s..e], "auto_save");

    // "true" is at offset 15..19
    let true_offset = buf.text().find("true").unwrap();
    let (s, e) = buf.word_range_at(true_offset);
    assert_eq!(&buf.text()[s..e], "true");

    // Colon ':'
    let colon_offset = buf.text().find(':').unwrap();
    let (s, e) = buf.word_range_at(colon_offset);
    assert_eq!(&buf.text()[s..e], ":");
}

#[test]
fn test_line_range_at() {
    let buf = TextBuffer::new("line 1\nline 2 with more text\nline 3");
    let (s0, e0) = buf.line_range_at(0);
    assert_eq!(&buf.text()[s0..e0], "line 1");

    let (s1, e1) = buf.line_range_at(1);
    assert_eq!(&buf.text()[s1..e1], "line 2 with more text");

    let (s2, e2) = buf.line_range_at(2);
    assert_eq!(&buf.text()[s2..e2], "line 3");
}

#[test]
fn test_extend_selection_to() {
    let mut buf = TextBuffer::new("hello world");
    buf.extend_selection_to(0, 5);
    assert_eq!(buf.selection(), Some((0, 5)));
    assert_eq!(buf.cursor(), 5);

    buf.extend_selection_to(5, 0);
    assert_eq!(buf.selection(), Some((0, 5)));
    assert_eq!(buf.cursor(), 0);

    buf.extend_selection_to(3, 3);
    assert_eq!(buf.selection(), None);
    assert_eq!(buf.cursor(), 3);
}

#[test]
fn test_char_index_and_byte_offset() {
    use file_viewer::editor::{char_index_at_x, char_index_to_byte_offset};

    let text = "  \"auto_save\": true,";
    // At char_width = 7.2
    // 0.0 -> char 0
    assert_eq!(char_index_at_x(text, 0.0, 7.2), 0);
    // 7.2 -> char 1
    assert_eq!(char_index_at_x(text, 7.2, 7.2), 1);
    // 14.4 -> char 2 ('"')
    assert_eq!(char_index_at_x(text, 14.4, 7.2), 2);

    let byte_off = char_index_to_byte_offset(text, 2);
    assert_eq!(byte_off, 2);
    assert_eq!(&text[byte_off..byte_off + 1], "\"");
}
