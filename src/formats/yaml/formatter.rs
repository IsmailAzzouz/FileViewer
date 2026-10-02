//! YAML formatter for pretty-printing and minification.
//!
//! Formatting is driven by a lightweight line-oriented pass rather than a
//! parse-and-reserialize round trip. Reserializing through a value tree would
//! drop comments, anchors, block scalar styles, and `---` separators, all of
//! which are meaningful in YAML, so this module normalizes spacing in place.
//!
//! Two rules keep the pass safe:
//!
//! * The body of a block scalar (`|` / `>`) is emitted verbatim, because its
//!   leading whitespace is part of the value.
//! * Indentation is never rewritten, because column changes can silently
//!   change which block a line belongs to.

use super::parser::YamlDiagnostic;

/// Formats a YAML string, normalizing spacing after `-` and `:`.
///
/// Returns the formatted document or a diagnostic if the input is invalid.
///
/// Complexity: O(N) where N is the length of the YAML string.
pub fn format_yaml(source: &str, indent: usize) -> Result<String, YamlDiagnostic> {
    // Validate first so invalid documents report a diagnostic instead of being
    // rewritten in a broken shape.
    super::parse_yaml(source)?;

    let unit = " ".repeat(indent);
    let mut out = String::with_capacity(source.len() + source.len() / 8);
    let mut pending_blank = false;
    let mut flow_depth: isize = 0;
    // Indent of the line that opened a block scalar, whose body is verbatim.
    let mut in_block_scalar = false;
    let mut block_scalar_indent = 0usize;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();

        // Block scalar bodies are value text, not structure.
        if in_block_scalar {
            let is_blank = trimmed.is_empty();
            if is_blank || raw_line.len() - raw_line.trim_start().len() > block_scalar_indent {
                if !is_blank {
                    out.push_str(raw_line.trim_end());
                } else {
                    out.push_str("");
                }
                out.push('\n');
                if is_blank {
                    // A blank line may end the scalar; the next content line
                    // decides.
                    continue;
                }
                continue;
            }
            in_block_scalar = false;
        }

        if trimmed.is_empty() {
            pending_blank = true;
            continue;
        }

        if pending_blank && !out.is_empty() {
            out.push('\n');
        }
        pending_blank = false;

        // Comments and document markers pass through untouched.
        if trimmed.starts_with('#') || is_document_marker(trimmed) {
            out.push_str(trimmed);
            out.push('\n');
            continue;
        }

        let leading = raw_line.len() - raw_line.trim_start().len();
        let line = if flow_depth > 0 {
            format!("{unit}{}", trimmed)
        } else {
            normalize_line(trimmed)
        };
        let _ = leading;

        if opens_block_scalar(trimmed) {
            in_block_scalar = true;
            block_scalar_indent = leading;
        }

        flow_depth = (flow_depth + flow_delta(trimmed)).max(0);

        out.push_str(&line);
        out.push('\n');
    }

    Ok(out)
}

/// Minifies a YAML string by stripping comments and collapsing whitespace.
///
/// Comments are removed because they cannot be preserved in a single-line
/// layout. Block scalar bodies are kept verbatim: folding their whitespace
/// would change the value.
///
/// Complexity: O(N) where N is the length of the YAML string.
pub fn minify_yaml(source: &str) -> Result<String, YamlDiagnostic> {
    super::parse_yaml(source)?;

    let mut out = String::with_capacity(source.len());
    let mut in_block_scalar = false;
    let mut block_scalar_indent = 0usize;

    for raw_line in source.lines() {
        let trimmed = raw_line.trim();
        let leading = raw_line.len() - raw_line.trim_start().len();

        if in_block_scalar {
            let is_blank = trimmed.is_empty();
            if is_blank || leading > block_scalar_indent {
                if !is_blank {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    // Verbatim, including indentation: this text *is* the value.
                    out.push_str(raw_line.trim_end());
                }
                continue;
            }
            in_block_scalar = false;
        }

        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }

        // Indentation is structural in YAML, unlike JSON, so the original
        // leading whitespace is preserved. Only the spacing *within* the line
        // is normalized.
        let line = normalize_line(strip_comment(trimmed)).trim_end().to_string();
        if line.is_empty() {
            continue;
        }
        if opens_block_scalar(&line) {
            in_block_scalar = true;
            block_scalar_indent = leading;
        }
        let pad = &raw_line[..leading];
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(pad);
        out.push_str(&line);
    }

    Ok(out)
}

/// Normalizes the spacing within one structural line.
///
/// Sequence entries get a single space after the `-`, and mapping entries get
/// a single space after the `:`. Any other spacing the author chose is left
/// alone so that flow collections and block scalar headers survive intact.
fn normalize_line(trimmed: &str) -> String {
    let bytes = trimmed.as_bytes();

    // Block sequence entry.
    if bytes[0] == b'-' && (bytes.len() == 1 || bytes[1] == b' ' || bytes[1] == b'\t') {
        let rest = trimmed[1..].trim();
        if rest.is_empty() {
            return "-".to_string();
        }
        if rest.starts_with('#') {
            return format!("- {}", rest);
        }
        return format!("- {}", normalize_entry(rest));
    }

    normalize_entry(trimmed)
}

/// Normalizes a mapping entry or bare scalar.
fn normalize_entry(text: &str) -> String {
    match split_mapping(text) {
        Some((key, rest)) => {
            if rest.is_empty() {
                // Keep the colon: it is the key/value separator, and dropping it
                // turns `items:` into the bare scalar `items`, which reparses as
                // a completely different document.
                format!("{key}:")
            } else {
                format!("{key}: {rest}")
            }
        }
        None => text.to_string(),
    }
}

/// Splits `key: value` at the first structural `:`, ignoring colons inside
/// quotes or flow collections. Returns `None` when there is no such colon.
fn split_mapping(line: &str) -> Option<(&str, &str)> {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut flow_depth = 0usize;
    let mut i = 0usize;

    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    if q == b'\'' && bytes.get(i + 1) == Some(&b'\'') {
                        i += 2;
                        continue;
                    }
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'[' | b'{' => flow_depth += 1,
                b']' | b'}' => flow_depth = flow_depth.saturating_sub(1),
                b'#' if i > 0 && matches!(bytes[i - 1], b' ' | b'\t') => return None,
                b':' if flow_depth == 0 => {
                    let next = bytes.get(i + 1);
                    // A separator colon is followed by whitespace, by nothing,
                    // or directly by the value (`name:viewer`). The `//` case is
                    // excluded so a URL like `http://x` is not split.
                    let is_separator = next.is_none()
                        || matches!(next, Some(b' ') | Some(b'\t'))
                        || !matches!(next, Some(b'/'));
                    if is_separator {
                        return Some((line[..i].trim_end(), line[i + 1..].trim_start()));
                    }
                }
                _ => {}
            },
        }
        i += 1;
    }
    None
}

/// Returns true when the line ends with a block scalar header, meaning the
/// following more-indented lines are literal value text.
fn opens_block_scalar(trimmed: &str) -> bool {
    let text = strip_comment(trimmed).trim_end();
    // Drop a sequence marker and a key prefix so `key: |` and `- |` both match.
    let candidate = match text.find(": ") {
        Some(idx) => text[idx + 2..].trim_start(),
        None => text.strip_prefix("- ").unwrap_or(text).trim_start(),
    };
    if candidate.is_empty() {
        return false;
    }
    let mut chars = candidate.chars();
    match chars.next() {
        Some('|') | Some('>') => {}
        _ => return false,
    }
    chars.all(|c| matches!(c, '+' | '-' | '0'..='9'))
}

/// Returns true when the line is a `---` or `...` document marker.
fn is_document_marker(trimmed: &str) -> bool {
    for marker in ["---", "..."] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            if rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t') {
                return true;
            }
        }
    }
    false
}

/// Returns the net change in flow-collection depth contributed by a line.
fn flow_delta(line: &str) -> isize {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut delta = 0isize;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    quote = None;
                }
            }
            None => match b {
                b'"' | b'\'' => quote = Some(b),
                b'#' => break,
                b'[' | b'{' => delta += 1,
                b']' | b'}' => delta -= 1,
                _ => {}
            },
        }
        i += 1;
    }
    delta
}

/// Removes a trailing comment, ignoring `#` inside quotes and a `#` that is
/// not preceded by whitespace (which is part of a plain scalar).
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) => {
                if b == b'\\' && q == b'"' {
                    i += 2;
                    continue;
                }
                if b == q {
                    quote = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    quote = Some(b);
                } else if b == b'#' && (i == 0 || matches!(bytes[i - 1], b' ' | b'\t')) {
                    return &line[..i];
                }
            }
        }
        i += 1;
    }
    line
}
