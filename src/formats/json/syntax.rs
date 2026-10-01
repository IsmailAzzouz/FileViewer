//! Lightweight syntax highlighter for JSON editor lines.
//!
//! Tokenizes individual lines into styled text spans with syntax colors.

use crate::theme::{
    SYNTAX_BOOLEAN, SYNTAX_COMMENT, SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_PUNCTUATION,
    SYNTAX_STRING, TEXT_MUTED, TEXT_PRIMARY,
};
use gpui::Hsla;

/// A segment of text styled with a specific theme color.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledSegment {
    pub text: String,
    pub color: Hsla,
}

/// Tokenizes a single line of JSON into styled segments.
///
/// Complexity: O(M) where M is the character length of the line.
pub fn tokenize_json_line(line: &str) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Whitespace
        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                text,
                color: TEXT_PRIMARY,
            });
            continue;
        }

        // Punctuation: braces, brackets, commas, colons
        if matches!(ch, '{' | '}' | '[' | ']') {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: SYNTAX_PUNCTUATION,
            });
            i += 1;
            continue;
        }

        if matches!(ch, ':' | ',') {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: TEXT_MUTED,
            });
            i += 1;
            continue;
        }

        // Strings (keys or string values)
        if ch == '"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < len {
                let c = chars[i];
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();

            // Check if followed by colon (ignoring whitespace) -> it's an object key!
            let mut j = i;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = j < len && chars[j] == ':';

            let color = if is_key { SYNTAX_KEY } else { SYNTAX_STRING };
            segments.push(StyledSegment { text, color });
            continue;
        }

        // Numbers
        if ch == '-' || ch.is_ascii_digit() {
            let start = i;
            if ch == '-' {
                i += 1;
            }
            while i < len
                && (chars[i].is_ascii_digit() || matches!(chars[i], '.' | 'e' | 'E' | '+' | '-'))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                text,
                color: SYNTAX_NUMBER,
            });
            continue;
        }

        // Keywords: true, false, null
        if chars[i..].starts_with(&['t', 'r', 'u', 'e']) {
            segments.push(StyledSegment {
                text: "true".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 4;
            continue;
        } else if chars[i..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            segments.push(StyledSegment {
                text: "false".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 5;
            continue;
        } else if chars[i..].starts_with(&['n', 'u', 'l', 'l']) {
            segments.push(StyledSegment {
                text: "null".to_string(),
                color: SYNTAX_NULL,
            });
            i += 4;
            continue;
        }

        // Fallback for comments or malformed tokens
        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && !matches!(chars[i], '{' | '}' | '[' | ']' | ':' | ',' | '"')
        {
            i += 1;
        }
        let text: String = chars[start..i].iter().collect();
        segments.push(StyledSegment {
            text,
            color: TEXT_PRIMARY,
        });
    }

    segments
}

/// Tokenizes a single line of JSON with Comments into styled segments.
///
/// Identical to [`tokenize_json_line`] except that `//` and `/* */` comment
/// runs are recognized and colored with [`SYNTAX_COMMENT`] instead of being
/// absorbed by the malformed-token fallback. Because each line is tokenized in
/// isolation, an unterminated `/*` simply colors to end of line; the parser is
/// what reports a genuinely unterminated block comment.
///
/// A `//` or `/*` appearing inside a string literal is not treated as a
/// comment, because string scanning consumes those characters first.
///
/// Complexity: O(M) where M is the character length of the line.
pub fn tokenize_jsonc_line(line: &str) -> Vec<StyledSegment> {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut segments = Vec::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Comments run to the end of the line, so they terminate the scan.
        if ch == '/' && i + 1 < len && (chars[i + 1] == '/' || chars[i + 1] == '*') {
            let start = i;
            if chars[i + 1] == '/' {
                i = len;
            } else {
                i += 2;
                while i < len {
                    if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                text,
                color: SYNTAX_COMMENT,
            });
            continue;
        }

        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                text,
                color: TEXT_PRIMARY,
            });
            continue;
        }

        if matches!(ch, '{' | '}' | '[' | ']') {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: SYNTAX_PUNCTUATION,
            });
            i += 1;
            continue;
        }

        if matches!(ch, ':' | ',') {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: TEXT_MUTED,
            });
            i += 1;
            continue;
        }

        if ch == '"' {
            let start = i;
            i += 1;
            let mut escaped = false;
            while i < len {
                let c = chars[i];
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();

            // A string followed by a colon is an object key, not a value. The
            // lookahead skips comments as well as whitespace, so a key
            // separated from its colon by `/* ... */` is still recognized.
            let mut j = i;
            while j < len {
                if chars[j].is_whitespace() {
                    j += 1;
                } else if chars[j] == '/' && j + 1 < len && chars[j + 1] == '/' {
                    j = len;
                } else if chars[j] == '/' && j + 1 < len && chars[j + 1] == '*' {
                    let mut k = j + 2;
                    while k < len && !(chars[k] == '*' && k + 1 < len && chars[k + 1] == '/') {
                        k += 1;
                    }
                    j = if k < len { k + 2 } else { len };
                } else {
                    break;
                }
            }
            let is_key = j < len && chars[j] == ':';

            let color = if is_key { SYNTAX_KEY } else { SYNTAX_STRING };
            segments.push(StyledSegment { text, color });
            continue;
        }

        if ch == '-' || ch.is_ascii_digit() {
            let start = i;
            if ch == '-' {
                i += 1;
            }
            while i < len
                && (chars[i].is_ascii_digit() || matches!(chars[i], '.' | 'e' | 'E' | '+' | '-'))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            segments.push(StyledSegment {
                text,
                color: SYNTAX_NUMBER,
            });
            continue;
        }

        if chars[i..].starts_with(&['t', 'r', 'u', 'e']) {
            segments.push(StyledSegment {
                text: "true".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 4;
            continue;
        } else if chars[i..].starts_with(&['f', 'a', 'l', 's', 'e']) {
            segments.push(StyledSegment {
                text: "false".to_string(),
                color: SYNTAX_BOOLEAN,
            });
            i += 5;
            continue;
        } else if chars[i..].starts_with(&['n', 'u', 'l', 'l']) {
            segments.push(StyledSegment {
                text: "null".to_string(),
                color: SYNTAX_NULL,
            });
            i += 4;
            continue;
        }

        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && !matches!(chars[i], '{' | '}' | '[' | ']' | ':' | ',' | '"')
            && !(chars[i] == '/' && i + 1 < len && (chars[i + 1] == '/' || chars[i + 1] == '*'))
        {
            i += 1;
        }
        let text: String = chars[start..i].iter().collect();
        segments.push(StyledSegment {
            text,
            color: TEXT_PRIMARY,
        });
    }

    segments
}
