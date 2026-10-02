//! Syntax tokenizer tests for YAML.

use file_viewer::formats::json::StyledSegment;
use file_viewer::formats::yaml::tokenize_yaml_line;

/// Concatenated text of a segment list, to assert nothing was lost.
fn text_of(segments: &[StyledSegment]) -> String {
    segments.iter().map(|s| s.text.as_str()).collect()
}

/// An unterminated quote must not panic. A trailing escape or doubled quote
/// steps the cursor past the last character, and the editor tokenizes every
/// visible line, so this runs on each keystroke while the user types.
#[test]
fn tokenize_yaml_line_handles_unterminated_quotes() {
    let cases = [
        "wrapped: \"escaped\\", // cursor sits after the backslash",
        "single: 'it''",        // doubled quote then end of line
        "just: \"",
        "just: '",
        "trailing: \"abc\\",
        "trailing: 'abc''",
        "\"key",
        "'key",
    ];

    for line in cases {
        let segments = tokenize_yaml_line(line);
        assert_eq!(
            text_of(&segments),
            line,
            "tokenizer lost or added text for {line:?}"
        );
    }
}

#[test]
fn tokenize_yaml_line_is_lossless_for_punctuation() {
    let line = "key: {a: 1, b: [2, 3]} # trailing";
    assert_eq!(text_of(&tokenize_yaml_line(line)), line);
}

#[test]
fn tokenize_yaml_line_handles_empty_and_blank() {
    for line in ["", " ", "\t", "   # only a comment"] {
        assert_eq!(text_of(&tokenize_yaml_line(line)), line);
    }
}
