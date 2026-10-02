//! YAML syntax highlighter for editor lines.
//!
//! Emits the shared [`StyledSegment`] shape so the editor can render YAML
//! through the same code path as JSON, JSONC, JSONL, and TOML.
//!
//! Tokenization is per line because the editor highlights a single line at a
//! time. One consequence is that the body of a block scalar (`|` / `>`) is
//! highlighted by content rather than as an opaque literal; the parser, not
//! the highlighter, is what resolves block scalar extent.

use gpui::Hsla;

use crate::formats::json::StyledSegment;
use crate::theme::{
    SYNTAX_BOOLEAN, SYNTAX_COMMENT, SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_PUNCTUATION,
    SYNTAX_STRING, TEXT_MUTED, TEXT_PRIMARY,
};

/// Tokenizes a single line of YAML into styled segments.
///
/// Complexity: O(M) where M is the character length of the line.
pub fn tokenize_yaml_line(line: &str) -> Vec<StyledSegment> {
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
                color: SYNTAX_COMMENT,
            });
            break;
        }

        // Document markers `---` and `...`
        if ch == '-' || ch == '.' {
            let mut run = i;
            while run < len && chars[run] == ch {
                run += 1;
            }
            let marker_len = run - i;
            if marker_len >= 3 && chars[i..run].iter().all(|c| *c == ch) {
                segments.push(StyledSegment {
                    text: chars[i..run].iter().collect(),
                    color: SYNTAX_NULL,
                });
                i = run;
                continue;
            }
        }

        // Block sequence entry marker
        if ch == '-' {
            let next = chars.get(i + 1);
            if next.is_none() || next == Some(&' ') || next == Some(&'\t') {
                segments.push(StyledSegment {
                    text: "-".to_string(),
                    color: SYNTAX_PUNCTUATION,
                });
                i += 1;
                continue;
            }
        }

        // Block scalar header: `|`, `>` plus chomping/indent indicators
        if ch == '|' || ch == '>' {
            let start = i;
            i += 1;
            while i < len && matches!(chars[i], '+' | '-' | '0'..='9') {
                i += 1;
            }
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: SYNTAX_NULL,
            });
            continue;
        }

        // Anchor `&name`, alias `*name`, and tag `!tag`
        if ch == '&' || ch == '*' || ch == '!' {
            let start = i;
            i += 1;
            while i < len
                && !chars[i].is_whitespace()
                && !matches!(chars[i], ',' | '{' | '}' | '[' | ']')
            {
                i += 1;
            }
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: SYNTAX_KEY,
            });
            continue;
        }

        // Flow punctuation
        if matches!(ch, '{' | '}' | '[' | ']' | ',' | ':') {
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: SYNTAX_PUNCTUATION,
            });
            i += 1;
            continue;
        }

        // Quoted scalars
        if ch == '"' || ch == '\'' {
            let start = i;
            let quote = ch;
            i += 1;
            while i < len {
                let c = chars[i];
                if quote == '"' && c == '\\' {
                    i += 2;
                    continue;
                }
                if c == quote {
                    if quote == '\'' && chars.get(i + 1) == Some(&'\'') {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            // A trailing escape (`"abc\"`) or doubled quote at end of line can
            // step past the last character; clamp so the slice below cannot
            // panic on an unterminated quote.
            let i = i.min(len);
            // A quoted token followed by `:` is a mapping key.
            let mut j = i;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            let is_key = chars.get(j) == Some(&':');
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: if is_key { SYNTAX_KEY } else { SYNTAX_STRING },
            });
            continue;
        }

        // Booleans and null
        if let Some(word) = match_keyword(&chars, i) {
            let (text, color) = word;
            let advance = text.chars().count();
            segments.push(StyledSegment { text, color });
            i += advance;
            continue;
        }

        // Numbers
        if ch.is_ascii_digit()
            || ((ch == '-' || ch == '+') && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let start = i;
            if ch == '-' || ch == '+' {
                i += 1;
            }
            while i < len && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '.' | '_')) {
                i += 1;
            }
            segments.push(StyledSegment {
                text: chars[start..i].iter().collect(),
                color: SYNTAX_NUMBER,
            });
            continue;
        }

        // Plain scalar or bare key: consume to a structural character, then
        // decide by lookahead whether it is a key.
        let start = i;
        while i < len
            && !chars[i].is_whitespace()
            && !matches!(
                chars[i],
                '#' | ':' | ',' | '{' | '}' | '[' | ']' | '|' | '>'
            )
        {
            i += 1;
        }
        if i == start {
            // A bare structural character the branches above did not claim.
            segments.push(StyledSegment {
                text: ch.to_string(),
                color: TEXT_MUTED,
            });
            i += 1;
            continue;
        }
        let mut j = i;
        while j < len && chars[j].is_whitespace() {
            j += 1;
        }
        let is_key = chars.get(j) == Some(&':');
        segments.push(StyledSegment {
            text: chars[start..i].iter().collect(),
            color: if is_key { SYNTAX_KEY } else { TEXT_PRIMARY },
        });
    }

    segments
}

/// Matches a boolean or null keyword at `i`, returning its text and color.
fn match_keyword(chars: &[char], i: usize) -> Option<(String, Hsla)> {
    const BOOLEANS: [&str; 10] = [
        "true", "True", "TRUE", "false", "False", "FALSE", "yes", "no", "on", "off",
    ];
    for word in BOOLEANS {
        if starts_with_at(chars, i, word) {
            return Some((word.to_string(), SYNTAX_BOOLEAN));
        }
    }
    for word in ["null", "Null", "NULL", "~"] {
        if starts_with_at(chars, i, word) {
            return Some((word.to_string(), SYNTAX_NULL));
        }
    }
    None
}

/// Returns true when `word` appears at `i` and is not followed by an
/// alphanumeric character.
fn starts_with_at(chars: &[char], i: usize, word: &str) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    if i + word_chars.len() > chars.len() {
        return false;
    }
    if chars[i..i + word_chars.len()] != word_chars[..] {
        return false;
    }
    match chars.get(i + word_chars.len()) {
        Some(c) => !c.is_alphanumeric() && *c != '_' && *c != '-',
        None => true,
    }
}
