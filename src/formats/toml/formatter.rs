//! TOML formatter for pretty-printing and minification.
//!
//! Formatting is driven by a lightweight line-oriented pass rather than a
//! parse-and-reserialize round trip. Reserializing through a value tree would
//! drop comments and table ordering, which are semantically meaningful in
//! TOML, so this module normalizes whitespace in place instead.

use super::parser::TomlDiagnostic;

/// Formats a TOML string, normalizing indentation and spacing.
///
/// Returns the formatted document or a diagnostic if the input is invalid.
///
/// Complexity: O(N) where N is the length of the TOML string.
pub fn format_toml(source: &str, indent: usize) -> Result<String, TomlDiagnostic> {
    // Validate first so invalid documents report a diagnostic instead of being
    // rewritten in a broken shape.
    super::parse_toml(source)?;

    let unit = " ".repeat(indent);
    let mut out = String::with_capacity(source.len() + source.len() / 8);
    let mut array_depth = 0usize;
    let mut pending_blank = false;

    for (idx, raw_line) in source.lines().enumerate() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            pending_blank = true;
            continue;
        }

        if pending_blank && idx > 0 && !out.is_empty() {
            out.push('\n');
        }
        pending_blank = false;

        let is_comment = trimmed.starts_with('#');
        let is_table_header = trimmed.starts_with('[');

        if is_comment {
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }

        if is_table_header {
            array_depth = 0;
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }

        // Track multi-line array depth so continuation lines stay indented.
        let delta = count_array_delta(trimmed);
        array_depth = (array_depth as isize + delta).max(0) as usize;

        let line = if array_depth > 0 {
            // Continuation line inside a multi-line array: trim and indent.
            format!("{unit}{}", trimmed)
        } else if let Some((key, value)) = split_assignment(trimmed) {
            if value.is_empty() {
                format!("{} =", key)
            } else {
                format!("{} = {}", key, value)
            }
        } else {
            trimmed.to_string()
        };

        out.push_str(&line);
        out.push('\n');
    }

    Ok(out)
}

/// Minifies a TOML string by stripping comments and collapsing whitespace.
///
/// Comments are removed because they cannot be preserved in a single-line
/// layout. Structure and values are left untouched.
///
/// Complexity: O(N) where N is the length of the TOML string.
pub fn minify_toml(source: &str) -> Result<String, TomlDiagnostic> {
    super::parse_toml(source)?;

    let mut out = String::with_capacity(source.len());
    for raw_line in source.lines() {
        let stripped = strip_comment(raw_line);
        let line = stripped.trim();
        if line.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(line);
    }
    Ok(out)
}

/// Splits `key = value` on the first top-level `=`.
fn split_assignment(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut in_string: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    in_string = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_string = Some(b);
                } else if b == b'=' {
                    // Reject `==` which is never valid TOML.
                    if bytes.get(i + 1) == Some(&b'=') {
                        return None;
                    }
                    return Some((line[..i].trim_end(), line[i + 1..].trim_start()));
                }
            }
        }
        i += 1;
    }
    None
}

/// Returns the net change in array nesting depth contributed by a line.
fn count_array_delta(line: &str) -> isize {
    let bytes = line.as_bytes();
    let mut in_string: Option<u8> = None;
    let mut delta = 0isize;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    in_string = None;
                }
            }
            None => {
                match b {
                    b'"' | b'\'' => in_string = Some(b),
                    b'#' => break,
                    b'[' => delta += 1,
                    b']' => delta -= 1,
                    _ => {}
                }
            }
        }
        i += 1;
    }
    delta
}

/// Removes a trailing comment, ignoring `#` inside strings.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match in_string {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    in_string = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_string = Some(b);
                } else if b == b'#' {
                    return &line[..i];
                }
            }
        }
        i += 1;
    }
    line
}
