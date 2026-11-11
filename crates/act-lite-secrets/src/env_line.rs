//! The one `.env` line grammar every reader in this crate uses.
//!
//! Grammar (a dotenv-compatible subset):
//!   [ws] [`export` ws] KEY [ws] `=` [ws] TOKEN [ws `#` comment]
//! TOKEN is `"…"` (ends at the next `"`), `'…'` (ends at the next `'`), or
//! bare (ends at end of line, or at the first whitespace followed by `#`).
//! No escape processing inside quotes. A trailing `\r` is not part of the line.
//!
//! An empty value followed by a comment, `KEY= # note`, is NOT an empty value
//! plus a comment: the comment rule needs whitespace *before* the `#` within
//! the token, and the token starts after the whitespace that follows `=`, so
//! the token is `# note`. python-dotenv reads that line the same way. Write
//! `KEY="" # note` for an empty value with a comment.
use std::ops::Range;

/// One assignment line, borrowed from its source. `token` is the raw value
/// text (quotes included); `token_span` is its byte range in the input line so
/// an editor can replace exactly that slice.
#[derive(Debug, PartialEq, Eq)]
pub struct EnvLine<'a> {
    pub export: bool,
    pub key: &'a str,
    pub token: &'a str,
    pub token_span: Range<usize>,
    pub comment: Option<&'a str>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Parsed<'a> {
    Blank,
    Comment,
    Assignment(EnvLine<'a>),
}

/// Parse one line. `Err(reason)` names the defect in plain words; the caller
/// adds the path and line number.
pub fn parse_line(line: &str) -> Result<Parsed<'_>, String> {
    let line = line.strip_suffix('\r').unwrap_or(line);
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return Ok(Parsed::Blank);
    }
    if trimmed.starts_with('#') {
        return Ok(Parsed::Comment);
    }

    let Some(eq) = line.find('=') else {
        return Err(
            "no `=` found; continuation lines of multi-line values are not supported".into(),
        );
    };
    let lhs = line[..eq].trim();
    let (export, key) = match lhs.strip_prefix("export") {
        Some(rest) if rest.starts_with(char::is_whitespace) => (true, rest.trim()),
        _ => (false, lhs),
    };
    if key.is_empty() {
        return Err("empty key before `=`".into());
    }
    if key.contains(char::is_whitespace) || key.contains('=') || key.starts_with('#') {
        return Err(
            "invalid key: names must have no whitespace or `=`, and must not start with `#`".into(),
        );
    }

    // Token starts after `=` and any whitespace.
    let after_eq = eq + 1;
    let ws = line[after_eq..].len() - line[after_eq..].trim_start().len();
    let start = after_eq + ws;
    let rest = &line[start..];

    let (token_end, comment) = match rest.as_bytes().first() {
        Some(&q @ (b'"' | b'\'')) => {
            let q = q as char;
            let Some(close_rel) = rest[1..].find(q) else {
                return Err("unterminated quote; multi-line values are not supported".into());
            };
            let end = start + 1 + close_rel + 1;
            let tail = &line[end..];
            let tail_trim = tail.trim_start();
            if tail_trim.is_empty() {
                (end, None)
            } else if tail_trim.starts_with('#') {
                (end, Some(tail_trim))
            } else {
                return Err("unexpected text after closing quote".into());
            }
        }
        _ => match find_inline_comment(rest) {
            Some(rel) => {
                let token = rest[..rel].trim_end();
                (start + token.len(), Some(rest[rel..].trim_start()))
            }
            None => (start + rest.trim_end().len(), None),
        },
    };

    Ok(Parsed::Assignment(EnvLine {
        export,
        key,
        token: &line[start..token_end],
        token_span: start..token_end,
        comment,
    }))
}

/// Byte offset in `rest` of the whitespace that precedes a `#` comment on a
/// bare token, or None. `abc#def` is a value; `abc #def` is a value + comment.
fn find_inline_comment(rest: &str) -> Option<usize> {
    let bytes = rest.as_bytes();
    (1..bytes.len()).find(|&i| bytes[i] == b'#' && bytes[i - 1].is_ascii_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assign(line: &str) -> EnvLine<'_> {
        match parse_line(line).unwrap() {
            Parsed::Assignment(a) => a,
            other => panic!("expected assignment, got {other:?}"),
        }
    }

    #[test]
    fn bare_token() {
        let a = assign("KEY=abc");
        assert_eq!(
            (a.export, a.key, a.token, a.comment),
            (false, "KEY", "abc", None)
        );
        assert_eq!(&"KEY=abc"[a.token_span.clone()], "abc");
    }

    #[test]
    fn export_prefix_and_spaces_around_equals() {
        let a = assign("  export KEY = abc");
        assert_eq!((a.export, a.key, a.token), (true, "KEY", "abc"));
    }

    #[test]
    fn inline_comment_after_bare_token() {
        let a = assign("KEY=abc # note");
        assert_eq!(a.token, "abc");
        assert_eq!(a.comment, Some("# note"));
    }

    #[test]
    fn hash_inside_bare_token_without_space_is_value() {
        let a = assign("KEY=abc#def");
        assert_eq!(a.token, "abc#def");
    }

    #[test]
    fn double_quoted_keeps_quotes_and_inner_hash() {
        let a = assign(r#"KEY="a b # c" # trailing"#);
        assert_eq!(a.token, r#""a b # c""#);
        assert_eq!(a.comment, Some("# trailing"));
    }

    #[test]
    fn single_quoted() {
        let a = assign("KEY='x=y'");
        assert_eq!(a.token, "'x=y'");
    }

    #[test]
    fn marker_is_a_bare_token() {
        let a = assign("KEY=#act:sequestered");
        assert_eq!(a.token, "#act:sequestered");
        assert_eq!(a.comment, None);
    }

    #[test]
    fn empty_token() {
        let a = assign("KEY=");
        assert_eq!(a.token, "");
    }

    #[test]
    fn a_comment_after_an_empty_value_is_the_token() {
        // Documented in the module grammar; python-dotenv reads it the same way.
        let a = assign("KEY= # note");
        assert_eq!((a.token, a.comment), ("# note", None));
        let a = assign("KEY=\"\" # note");
        assert_eq!((a.token, a.comment), ("\"\"", Some("# note")));
    }

    #[test]
    fn crlf_is_not_part_of_the_token() {
        let a = assign("KEY=abc\r");
        assert_eq!(a.token, "abc");
    }

    #[test]
    fn blank_and_comment_lines() {
        assert!(matches!(parse_line("   ").unwrap(), Parsed::Blank));
        assert!(matches!(parse_line("# KEY=abc").unwrap(), Parsed::Comment));
    }

    #[test]
    fn malformed_lines_report_a_reason() {
        assert!(parse_line("junk").unwrap_err().contains("no `=`"));
        assert!(parse_line("=abc").unwrap_err().contains("empty key"));
        assert!(
            parse_line("KEY=\"open")
                .unwrap_err()
                .contains("unterminated")
        );
        assert!(
            parse_line("KEY=\"a\" b")
                .unwrap_err()
                .contains("after closing quote")
        );
        assert!(
            parse_line("BAD KEY=abc")
                .unwrap_err()
                .contains("invalid key")
        );
    }
}
