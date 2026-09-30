use file_viewer::editor::{compute_visual_rows, find_visual_row_by_offset, TextBuffer};

#[test]
fn test_unwrapped_visual_rows() {
    let buf = TextBuffer::new("line 1\nline 2\nline 3");
    let snap = buf.snapshot();
    let rows = compute_visual_rows(&snap, false, 80);

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].logical_line, 0);
    assert_eq!(rows[0].sub_line, 0);
    assert_eq!(rows[1].logical_line, 1);
    assert_eq!(rows[2].logical_line, 2);
}

#[test]
fn test_wrapped_visual_rows_at_delimiter() {
    // 100 character long line with space and comma delimiters
    let text = "{\"id\": 12345, \"title\": \"Very long title that exceeds the wrap limit and must wrap cleanly at spaces\", \"active\": true}";
    let buf = TextBuffer::new(text);
    let snap = buf.snapshot();

    // Wrap at 40 columns
    let rows = compute_visual_rows(&snap, true, 40);

    // Should break into multiple sub-lines
    assert!(rows.len() > 1);

    // All rows belong to logical line 0
    for (i, row) in rows.iter().enumerate() {
        assert_eq!(row.logical_line, 0);
        assert_eq!(row.sub_line, i);
        assert!(row.start_byte < row.end_byte);
    }

    // Verify row spans reconstruct the full text
    assert_eq!(rows[0].start_byte, 0);
    assert_eq!(rows.last().unwrap().end_byte, text.len());

    let mut reconstructed = String::new();
    for row in rows.iter() {
        reconstructed.push_str(&text[row.start_byte..row.end_byte]);
    }
    assert_eq!(reconstructed, text);
}

#[test]
fn test_find_visual_row_by_offset() {
    let text = "first line\nsecond line is quite long and wraps nicely\nthird line";
    let buf = TextBuffer::new(text);
    let snap = buf.snapshot();
    let rows = compute_visual_rows(&snap, true, 20);

    let offset_at_third = text.find("third line").unwrap();
    let row_idx = find_visual_row_by_offset(&rows, offset_at_third);

    assert_eq!(rows[row_idx].logical_line, 2);
}
