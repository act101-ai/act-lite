//! Line-preserving `.env` buffer editor for `act-lite env mv` and `act-lite env import`.
//!
//! Every function here operates on a string buffer — no I/O. `set` replaces
//! only the value token of each matching assignment line and preserves every
//! other byte: indentation, `export`, inline comment, and the line terminator.
use std::path::Path;

use crate::env_line::{EnvLine, Parsed, parse_line};
use crate::error::SecretsError;
use crate::silhouette::SEQUESTERED_VALUE;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineState {
    Absent,
    Sequestered,
    /// The raw token (quotes included) of the assignment.
    Value(String),
}

/// Yield `(line_number, byte_range_of_line_content)` for every line. The
/// range excludes `\n` but includes a trailing `\r`, so callers slice
/// `parse_line` results against `&buffer[range]`.
fn lines(buffer: &str) -> impl Iterator<Item = (usize, std::ops::Range<usize>)> + '_ {
    let mut pos = 0usize;
    let mut no = 0usize;
    std::iter::from_fn(move || {
        if pos >= buffer.len() {
            return None;
        }
        no += 1;
        let start = pos;
        let end = buffer[pos..].find('\n').map_or(buffer.len(), |i| pos + i);
        pos = if end < buffer.len() { end + 1 } else { end };
        Some((no, start..end))
    })
}

fn assignment<'a>(buffer: &'a str, range: &std::ops::Range<usize>) -> Option<EnvLine<'a>> {
    match parse_line(&buffer[range.clone()]) {
        Ok(Parsed::Assignment(a)) => Some(a),
        _ => None,
    }
}

/// State of `key` in the buffer. Lines that do not parse are skipped (the
/// strict whole-file parser reports them; this editor only needs the key).
pub fn find(buffer: &str, key: &str, path: &Path) -> Result<LineState, SecretsError> {
    let mut seen: Option<(usize, String)> = None;
    for (no, range) in lines(buffer) {
        let Some(a) = assignment(buffer, &range) else {
            continue;
        };
        if a.key != key {
            continue;
        }
        match &seen {
            Some((first, tok)) if tok != a.token => {
                return Err(SecretsError::DuplicateKey {
                    path: path.to_path_buf(),
                    key: key.to_string(),
                    first: *first,
                    second: no,
                });
            }
            Some(_) => {}
            None => seen = Some((no, a.token.to_string())),
        }
    }
    Ok(match seen {
        None => LineState::Absent,
        Some((_, tok)) if tok == SEQUESTERED_VALUE => LineState::Sequestered,
        Some((_, tok)) => LineState::Value(tok),
    })
}

/// Return a buffer in which every assignment line for `key` has its value
/// token replaced by `token`; all other bytes are unchanged. If no such line
/// exists, `KEY=token\n` is appended (preceded by `\n` iff the buffer is
/// non-empty and does not already end with one).
pub fn set(buffer: &str, key: &str, token: &str) -> String {
    let mut out = String::with_capacity(buffer.len() + token.len());
    let mut cursor = 0usize;
    let mut replaced = false;
    for (_, range) in lines(buffer) {
        if let Some(a) = assignment(buffer, &range)
            && a.key == key
        {
            let span_start = range.start + a.token_span.start;
            let span_end = range.start + a.token_span.end;
            out.push_str(&buffer[cursor..span_start]);
            out.push_str(token);
            cursor = span_end;
            replaced = true;
        }
    }
    out.push_str(&buffer[cursor..]);
    if replaced {
        return out;
    }
    if !buffer.is_empty() && !buffer.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(key);
    out.push('=');
    out.push_str(token);
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn p() -> &'static Path {
        Path::new("t.env")
    }

    // ── find tests ────────────────────────────────────────────────────────────

    #[test]
    fn find_returns_value_for_plain_assignment() {
        let buf = "FOO=hello\n";
        assert_eq!(
            find(buf, "FOO", p()).unwrap(),
            LineState::Value("hello".to_string())
        );
    }

    #[test]
    fn find_keeps_double_quotes_in_raw_token() {
        let buf = r#"KEY="abc""#;
        assert_eq!(
            find(buf, "KEY", p()).unwrap(),
            LineState::Value("\"abc\"".to_string())
        );
    }

    #[test]
    fn find_keeps_single_quotes_in_raw_token() {
        let buf = "KEY='abc'\n";
        assert_eq!(
            find(buf, "KEY", p()).unwrap(),
            LineState::Value("'abc'".to_string())
        );
    }

    #[test]
    fn find_trims_whitespace_around_equals() {
        let buf = "KEY = abc\n";
        assert_eq!(
            find(buf, "KEY", p()).unwrap(),
            LineState::Value("abc".to_string())
        );
    }

    #[test]
    fn find_returns_sequestered_for_marker() {
        let buf = format!("KEY={SEQUESTERED_VALUE}\n");
        assert_eq!(find(&buf, "KEY", p()).unwrap(), LineState::Sequestered);
    }

    #[test]
    fn find_returns_absent_when_key_not_present() {
        let buf = "OTHER=val\n# KEY=comment\nANOTHER=x\n";
        assert_eq!(find(buf, "KEY", p()).unwrap(), LineState::Absent);
    }

    #[test]
    fn find_not_fooled_by_key_in_comment() {
        let buf = "# FOO=bar\nBAR=baz\n";
        assert_eq!(find(buf, "FOO", p()).unwrap(), LineState::Absent);
    }

    #[test]
    fn find_not_fooled_by_key_as_substring_of_another_key() {
        // FOOBAR should not match FOO
        let buf = "FOOBAR=val\n";
        assert_eq!(find(buf, "FOO", p()).unwrap(), LineState::Absent);
    }

    #[test]
    fn find_sees_through_export_prefix() {
        assert_eq!(
            find("export FOO=bar\n", "FOO", p()).unwrap(),
            LineState::Value("bar".into())
        );
    }

    #[test]
    fn find_returns_raw_token_with_quotes() {
        assert_eq!(
            find("K=\"a b\"\n", "K", p()).unwrap(),
            LineState::Value("\"a b\"".into())
        );
    }

    #[test]
    fn find_errors_on_differing_duplicates_and_accepts_identical_ones() {
        let err = find("K=a\nK=b\n", "K", p()).unwrap_err();
        assert!(matches!(
            err,
            SecretsError::DuplicateKey {
                first: 1,
                second: 2,
                ..
            }
        ));
        assert_eq!(
            find("K=a\nK=a\n", "K", p()).unwrap(),
            LineState::Value("a".into())
        );
    }

    // ── set tests ─────────────────────────────────────────────────────────────

    #[test]
    fn set_replaces_existing_line_leaves_others_byte_identical() {
        // Buffer: comment, two app lines, blank, target line.
        let buf = "# config\nAPP_ID=123\nAPP_SECRET=xyz\n\nTARGET=old\n";
        let result = set(buf, "TARGET", "new");
        assert_eq!(
            result,
            "# config\nAPP_ID=123\nAPP_SECRET=xyz\n\nTARGET=new\n"
        );
    }

    #[test]
    fn set_replaces_only_the_target_line_all_others_identical() {
        let buf = "# config\nAPP_ID=123\nAPP_SECRET=xyz\n\nTARGET=old\n";
        let result = set(buf, "TARGET", "new");
        // Every other byte must be identical — check by replacing only the
        // known-changed portion and asserting total equality.
        let expected = buf.replace("TARGET=old", "TARGET=new");
        assert_eq!(result, expected);
    }

    #[test]
    fn set_appends_when_absent_buffer_ends_with_newline() {
        let buf = "FOO=bar\n";
        let result = set(buf, "NEW_KEY", "value");
        assert_eq!(result, "FOO=bar\nNEW_KEY=value\n");
    }

    #[test]
    fn set_appends_with_leading_newline_when_buffer_does_not_end_with_newline() {
        let buf = "FOO=bar";
        let result = set(buf, "NEW_KEY", "value");
        assert_eq!(result, "FOO=bar\nNEW_KEY=value\n");
    }

    #[test]
    fn set_on_empty_buffer_yields_key_equals_rhs_newline() {
        let result = set("", "KEY", "rhs");
        assert_eq!(result, "KEY=rhs\n");
    }

    #[test]
    fn set_preserves_crlf_terminator_on_other_lines() {
        // Mix: first line CRLF, second LF, target LF.
        let buf = "FIRST=a\r\nSECOND=b\nTARGET=old\n";
        let result = set(buf, "TARGET", "new");
        assert_eq!(result, "FIRST=a\r\nSECOND=b\nTARGET=new\n");
    }

    #[test]
    fn set_preserves_crlf_terminator_on_edited_line() {
        let buf = "KEY=old\r\n";
        let result = set(buf, "KEY", "new");
        assert_eq!(result, "KEY=new\r\n");
    }

    #[test]
    fn round_trip_sequester_then_restore_is_byte_identical() {
        let buf = "APP=origval\n";
        let sequestered = set(buf, "APP", SEQUESTERED_VALUE);
        let restored = set(&sequestered, "APP", "origval");
        assert_eq!(restored, buf);
    }

    #[test]
    fn set_keeps_export_prefix_indent_and_inline_comment() {
        let buf = "  export K=old # keep me\r\nOTHER=x\n";
        assert_eq!(
            set(buf, "K", "new"),
            "  export K=new # keep me\r\nOTHER=x\n"
        );
    }

    #[test]
    fn set_rewrites_every_matching_line() {
        assert_eq!(set("K=first\nK=first\n", "K", "r"), "K=r\nK=r\n");
    }

    #[test]
    fn set_replaces_a_quoted_token_wholesale() {
        assert_eq!(
            set("K=\"a b\" # c\n", "K", "#act:sequestered"),
            "K=#act:sequestered # c\n"
        );
    }
}
