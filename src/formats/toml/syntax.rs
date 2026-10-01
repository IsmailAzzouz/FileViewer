//! TOML syntax highlighter for editor lines.
//!
//! Emits the shared [`StyledSegment`] shape so the editor can render JSON and
//! TOML through the same code path.

use crate::formats::json::StyledSegment;
use crate::theme::{
    SYNTAX_BOOLEAN, SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_PUNCTUATION, SYNTAX_STRING,
    TEXT_MUTED, TEXT_PRIMARY,
};

/// Tokenizes a single line of TOML into styled segments.
///
/// Complexity: O(M) where M is the character length of the line.
pub fn tokenize_toml_line(line: &str) -> Vec<StyledSegment> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut segments: Vec<StyledSegment> = Vec::new();
    let mut i = 0usize;

    while i < len {
        let ch = chars[i];

        // Whitespace
        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: TEXT_PRIMARY,
            });
            continue;
        }

        // Comment: rest of the line
        if ch == '#' {
            segments.push(StyledSegment {
                text: chars[i..].iter().collect(),
                color: TEXT_MUTED,
            });
            break;
        }

        // Table header: [table] or [[array]]
        if ch == '[' {
            let start = i;
            i += 1;
            if i < len && chars[i] == '[' {
                i += 1;
            }
            while i < len && chars[i] != ']' {
                i += 1;
            }
            while i < len && chars[i] == ']' {
                i += 1;
            }
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: SYNTAX_NULL,
            });
            continue;
        }

        // Inline table brace
        if ch == '{' || ch == '}' {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: SYNTAX_PUNCTUATION,
            });
            i += 1;
            continue;
        }

        // Comma
        if ch == ',' {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: TEXT_MUTED,
            });
            i += 1;
            continue;
        }

        // Strings: basic ("), literal ('), and their multi-line forms
        if ch == '"' || ch == '\'' {
            let start = i;
            let quote = ch;
            let triple = i + 2 < len && chars[i + 1] == quote && chars[i + 2] == quote;
            if triple {
                i += 3;
                while i < len {
                    if chars[i] == quote
                        && i + 2 < len
                        && chars[i + 1] == quote
                        && chars[i + 2] == quote
                    {
                        i += 3;
                        break;
                    }
                    if quote == '"' && chars[i] == '\\' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                }
            } else {
                i += 1;
                while i < len {
                    let c = chars[i];
                    if c == '\\' && quote == '"' {
                        i += 2;
                        continue;
                    }
                    if c == quote {
                        i += 1;
                        break;
                    }
                    if c == '\n' {
                        break;
                    }
                    i += 1;
                }
            }
            // A string followed by `=` is a key.
            let mut j = i;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = j < len && chars[j] == '=';
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: if is_key { SYNTAX_KEY } else { SYNTAX_STRING },
            });
            continue;
        }

        // Assignment operator
        if ch == '=' {
            segments.push(StyledSegment {
                text: "=".to_string(),
                color: SYNTAX_PUNCTUATION,
            });
            i += 1;
            continue;
        }

        // Booleans
        if chars[i..].starts_with(&['t', 'r', 'u', 'e']) {
            segments.push(StyledSegment {
                text: "true".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 4;
            continue;
        }
        if chars[i..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            segments.push(StyledSegment {
                text: "false".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 5;
            continue;
        }

        // Numbers, datetimes, and bare keys
        if ch.is_ascii_digit() || ch == '-' || ch == '+' {
            let start = i;
            if ch == '-' || ch == '+' {
                i += 1;
            }
            while i < len && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '.' | ':' | '-' | '+' | '_')) {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                color: SYNTAX_NUMBER,
                text,
            });
            continue;
        }

        // Bare key: run of key characters, then lookahead for `=`.
        if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
                i += 1;
            }
            let mut j = i;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = j < len && chars[j] == '=';
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: if is_key { SYNTAX_KEY } else { TEXT_PRIMARY },
            });
            continue;
        }

        // Dotted key separator and anything else
        segments.push(StyledSegment {
            text: ch.to_string(),
            color: TEXT_MUTED,
        });
        i += 1;
    }

    segments
}
