//! The TOML reading the Python and Rust resolvers share: keys, strings and
//! the first syntax error of a parsed tree.

use tree_sitter::Node;

/// Each pair below `prefix`, also those of inline tables, with its full key.
pub(super) fn toml_pair<'t>(
    pair: Node<'t>,
    prefix: &[String],
    text: &str,
    emit: &mut impl FnMut(Vec<String>, Node<'t>),
) {
    let mut cursor = pair.walk();
    let children: Vec<Node> = pair.named_children(&mut cursor).collect();
    let (Some(key), Some(value)) = (children.first(), children.last()) else {
        return;
    };
    let mut path = prefix.to_vec();
    path.extend(key_path(*key, text));
    if value.kind() == "inline_table" {
        let mut cursor = value.walk();
        for inner in value.named_children(&mut cursor) {
            if inner.kind() == "pair" {
                toml_pair(inner, &path, text, emit);
            }
        }
    }
    emit(path, *value);
}

pub(super) fn is_key(node: Node) -> bool {
    matches!(node.kind(), "bare_key" | "quoted_key" | "dotted_key")
}

/// The segments of a key.
pub(super) fn key_path(key: Node, text: &str) -> Vec<String> {
    if key.kind() == "dotted_key" {
        let mut cursor = key.walk();
        return key
            .named_children(&mut cursor)
            .filter(|k| is_key(*k))
            .flat_map(|k| key_path(k, text))
            .collect();
    }
    vec![unquote(&text[key.byte_range()])]
}

/// The value of a TOML string or quoted key: quotes removed, the newline
/// after an opening `"""` or `'''` trimmed, basic-string escapes applied.
pub(super) fn unquote(literal: &str) -> String {
    let strip = |quote: &str| literal.strip_prefix(quote)?.strip_suffix(quote);
    let first_line = |s: &'_ str| -> String {
        s.strip_prefix("\r\n")
            .or_else(|| s.strip_prefix('\n'))
            .unwrap_or(s)
            .to_string()
    };
    if let Some(inner) = strip("'''") {
        return first_line(inner);
    }
    if let Some(inner) = strip("\"\"\"") {
        return unescape(&first_line(inner), true);
    }
    if let Some(inner) = strip("'") {
        return inner.to_string();
    }
    if let Some(inner) = strip("\"") {
        return unescape(inner, false);
    }
    literal.to_string()
}

/// TOML basic-string escapes; in a multi-line string, a `\` before
/// whitespace removes that whitespace and the newlines after it.
fn unescape(text: &str, multiline: bool) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push('\\');
            break;
        };
        let simple = match next {
            'b' => Some('\u{8}'),
            't' => Some('\t'),
            'n' => Some('\n'),
            'f' => Some('\u{c}'),
            'r' => Some('\r'),
            'e' => Some('\u{1b}'),
            '"' => Some('"'),
            '\\' => Some('\\'),
            _ => None,
        };
        if let Some(simple) = simple {
            out.push(simple);
        } else if next == 'u' || next == 'U' {
            let digits: String = chars
                .by_ref()
                .take(if next == 'u' { 4 } else { 8 })
                .collect();
            match u32::from_str_radix(&digits, 16)
                .ok()
                .and_then(char::from_u32)
            {
                Some(decoded) => out.push(decoded),
                None => {
                    out.push('\\');
                    out.push(next);
                    out.push_str(&digits);
                }
            }
        } else if multiline && next.is_whitespace() {
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
        } else {
            out.push('\\');
            out.push(next);
        }
    }
    out
}

/// The first `ERROR` or `MISSING` node of the tree.
pub(super) fn first_error(node: Node) -> Option<Node> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }
    if !node.has_error() {
        return None;
    }
    let mut cursor = node.walk();
    let children: Vec<Node> = node.children(&mut cursor).collect();
    children.into_iter().find_map(first_error)
}
