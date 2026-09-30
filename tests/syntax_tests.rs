use file_viewer::formats::json::tokenize_json_line;
use file_viewer::theme::{
    SYNTAX_BOOLEAN, SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_PUNCTUATION, SYNTAX_STRING,
};

#[test]
fn test_tokenize_json_line() {
    let line = r#"  "name": "FileViewer", "count": 42, "flag": true, "empty": null"#;
    let tokens = tokenize_json_line(line);

    // Check that keys have SYNTAX_KEY color
    let key_tokens: Vec<_> = tokens.iter().filter(|t| t.color == SYNTAX_KEY).collect();
    assert_eq!(key_tokens.len(), 4);
    assert_eq!(key_tokens[0].text, "\"name\"");
    assert_eq!(key_tokens[1].text, "\"count\"");
    assert_eq!(key_tokens[2].text, "\"flag\"");
    assert_eq!(key_tokens[3].text, "\"empty\"");

    // Check string value
    let string_tokens: Vec<_> = tokens.iter().filter(|t| t.color == SYNTAX_STRING).collect();
    assert_eq!(string_tokens.len(), 1);
    assert_eq!(string_tokens[0].text, "\"FileViewer\"");

    // Check number value
    let number_tokens: Vec<_> = tokens.iter().filter(|t| t.color == SYNTAX_NUMBER).collect();
    assert_eq!(number_tokens.len(), 1);
    assert_eq!(number_tokens[0].text, "42");

    // Check bool value
    let bool_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| t.color == SYNTAX_BOOLEAN)
        .collect();
    assert_eq!(bool_tokens.len(), 1);
    assert_eq!(bool_tokens[0].text, "true");

    // Check null value
    let null_tokens: Vec<_> = tokens.iter().filter(|t| t.color == SYNTAX_NULL).collect();
    assert_eq!(null_tokens.len(), 1);
    assert_eq!(null_tokens[0].text, "null");
}

#[test]
fn test_tokenize_brackets_and_braces() {
    let line = "  [ { } ] ";
    let tokens = tokenize_json_line(line);
    let punctuation: Vec<_> = tokens
        .iter()
        .filter(|t| t.color == SYNTAX_PUNCTUATION)
        .collect();
    assert_eq!(punctuation.len(), 4);
    assert_eq!(punctuation[0].text, "[");
    assert_eq!(punctuation[1].text, "{");
    assert_eq!(punctuation[2].text, "}");
    assert_eq!(punctuation[3].text, "]");
}
