//! Relationships: the import-like constructs of a file, for JavaScript, JSX,
//! TypeScript, TSX, Python, Go and Rust.
//!
//! A relationship is one occurrence of an import construct, with its kind,
//! its target text as written (`specifier`), the value it is resolved by
//! (`value`: a string literal decoded by its language's escape rules) and
//! the range of the construct.
//! One statement can hold several relationships, which then share its range
//! (`import os, sys`; `use a::{b, c}`). A file with a parse error gives the
//! relationships of its recovered tree. Every other grammar gives none.
//!
//! `module_names` gives the names that a Python module binds, or that a Rust
//! module file declares, at its top level; resolution checks a
//! `from X import n` or a `use` path against them.

mod go;
mod js;
mod python;
mod rust;

use std::collections::BTreeSet;

use act_lite_core::CompactRange;
use tree_sitter::Node;

use crate::{Grammar, SourceFile};

/// The kind of a relationship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RelationshipKind {
    DynamicImport,
    ExternCrate,
    FromImport,
    Import,
    Include,
    Module,
    Reexport,
    Require,
    TypeImport,
    Use,
}

impl RelationshipKind {
    /// The kind's name in results.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DynamicImport => "dynamic_import",
            Self::ExternCrate => "extern_crate",
            Self::FromImport => "from_import",
            Self::Import => "import",
            Self::Include => "include",
            Self::Module => "module",
            Self::Reexport => "reexport",
            Self::Require => "require",
            Self::TypeImport => "type_import",
            Self::Use => "use",
        }
    }
}

/// A string literal that does not decode by its language's escape rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidLiteral;

/// The decoded value of a string literal.
pub type LiteralValue = Result<String, InvalidLiteral>;

/// One import-like construct of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub kind: RelationshipKind,
    /// The target text as written, without quotes.
    pub specifier: String,
    /// The value the target is resolved by: `specifier` decoded by the
    /// language's escape rules when it is read from a string literal, else
    /// `specifier` itself.
    pub value: LiteralValue,
    /// False when the target is computed (`require(name)`,
    /// `include!(concat!(…))`); `specifier` is then the argument's text.
    pub literal: bool,
    pub range: CompactRange,
    /// Python `from … import n`: `n` (`*` for a wildcard import).
    pub imported: Option<String>,
    /// Rust `mod x;`: the decoded value of a `#[path = "…"]` attribute on
    /// it.
    pub path_attribute: Option<LiteralValue>,
    /// Rust: read from the token tree of a macro invocation.
    pub in_macro: bool,
}

/// The names a module binds or declares at its top level.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleNames {
    pub names: BTreeSet<String>,
    /// The module has a wildcard import (`from … import *`, `use …::*`).
    pub glob: bool,
    /// Rust: the inline modules (`mod a { … }`), in location order.
    pub inline: Vec<InlineModule>,
    /// Rust: the blocks that declare an item, an inline module or a `use`,
    /// inside the module's function, `impl` and trait bodies and not inside
    /// another such block, in location order. They are not in the module.
    pub blocks: Vec<BlockModule>,
    /// Rust, for a block or an inline module inside a body: the leaves of
    /// its `use` declarations, in location order.
    pub uses: Vec<UseBinding>,
}

/// A Rust block (a function, closure or other block body) that declares an
/// item, an inline module or a `use`: its anonymous module. Its `names` are
/// the names of its items and inline modules, not of its `use` leaves,
/// which are its `uses`; its inline modules are not children of the
/// crate's module tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockModule {
    pub range: CompactRange,
    pub names: ModuleNames,
}

/// A leaf of a Rust `use` declaration inside a body: the name it binds in
/// its scope (`None` for a glob) and the path it binds, as written in that
/// scope (for a glob, the path before `::*`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseBinding {
    pub name: Option<String>,
    pub path: String,
    /// The `use` declaration.
    pub range: CompactRange,
}

/// A Rust inline module and the names it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineModule {
    pub name: String,
    pub range: CompactRange,
    pub path_attribute: Option<LiteralValue>,
    pub names: ModuleNames,
}

/// Every relationship of the file, in location order, then specifier.
pub fn extract(file: &SourceFile) -> Vec<Relationship> {
    let root = file.tree().root_node();
    let source = file.content();
    let mut found = match file.grammar() {
        Grammar::JavaScript | Grammar::Jsx | Grammar::TypeScript | Grammar::Tsx => {
            js::extract(root, source)
        }
        Grammar::Python => python::extract(root, source),
        Grammar::Go => go::extract(root, source),
        Grammar::Rust => rust::extract(root, source),
        _ => Vec::new(),
    };
    found.sort_by(|a, b| {
        position(&a.range)
            .cmp(&position(&b.range))
            .then_with(|| a.specifier.cmp(&b.specifier))
    });
    found
}

/// The names bound (Python) or declared (Rust) at the file's top level;
/// empty for every other grammar.
pub fn module_names(file: &SourceFile) -> ModuleNames {
    let root = file.tree().root_node();
    match file.grammar() {
        Grammar::Python => python::module_names(root, file.content()),
        Grammar::Rust => rust::module_names(root, file.content()),
        _ => ModuleNames::default(),
    }
}

/// The names of the `mod x;` declarations at the top level of a piece of
/// Rust text, such as a test region that analysis removes.
pub fn mods_in_text(text: &str) -> Vec<String> {
    rust::mods_in_text(text)
}

impl Relationship {
    fn new(kind: RelationshipKind, specifier: &str, node: Node) -> Self {
        Self {
            kind,
            specifier: specifier.to_string(),
            value: Ok(specifier.to_string()),
            literal: true,
            range: range_of(node),
            imported: None,
            path_attribute: None,
            in_macro: false,
        }
    }
}

fn position(range: &CompactRange) -> (u32, u32, u32, u32) {
    (
        range.start_line(),
        range.start_col(),
        range.end_line(),
        range.end_col(),
    )
}

fn range_of(node: Node) -> CompactRange {
    let (start, end) = (node.start_position(), node.end_position());
    CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    )
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// A string literal's text without its delimiters.
fn unquote(text: &str) -> &str {
    text.get(1..text.len().saturating_sub(1)).unwrap_or("")
}

/// The value of a JavaScript or TypeScript string, or of a template string
/// without a substitution, whose text without delimiters is `inner`: `\xHH`,
/// `\uHHHH` (a surrogate pair is one character), `\u{…}`, `\0`, the
/// single-character escapes and line continuations.
fn js_value(inner: &str) -> LiteralValue {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or(InvalidLiteral)? {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'v' => out.push('\u{b}'),
            '0' if !chars.as_str().starts_with(|d: char| d.is_ascii_digit()) => out.push('\0'),
            '0'..='9' => return Err(InvalidLiteral),
            'x' => out.push(char_of(hex(&mut chars, 2)?)?),
            'u' => {
                let mut code = js_code(&mut chars)?;
                if (0xD800..0xDC00).contains(&code) {
                    let mut ahead = chars.clone();
                    if ahead.next() == Some('\\')
                        && ahead.next() == Some('u')
                        && let Ok(low) = js_code(&mut ahead)
                        && (0xDC00..0xE000).contains(&low)
                    {
                        chars = ahead;
                        code = 0x10000 + ((code - 0xD800) << 10) + (low - 0xDC00);
                    }
                }
                out.push(char_of(code)?);
            }
            '\r' => {
                if chars.as_str().starts_with('\n') {
                    chars.next();
                }
            }
            '\n' | '\u{2028}' | '\u{2029}' => {}
            other => out.push(other),
        }
    }
    Ok(out)
}

/// The code of a JavaScript `\u` escape after the `u`: four hex digits or
/// `{…}`.
fn js_code(chars: &mut std::str::Chars) -> Result<u32, InvalidLiteral> {
    if !chars.as_str().starts_with('{') {
        return hex(chars, 4);
    }
    chars.next();
    braced(chars, false)
}

/// The value of a Go interpreted string whose text without quotes is
/// `inner`: `\xHH` and octal `\OOO` (bytes), `\uHHHH`, `\UHHHHHHHH` and the
/// single-character escapes; the bytes must be UTF-8.
pub fn go_value(inner: &str) -> LiteralValue {
    let mut out = Vec::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        let c = if c != '\\' {
            c
        } else {
            match chars.next().ok_or(InvalidLiteral)? {
                'a' => '\u{7}',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '\\' => '\\',
                '"' => '"',
                'x' => {
                    out.push(hex(&mut chars, 2)? as u8);
                    continue;
                }
                d @ '0'..='7' => {
                    let mut code = d.to_digit(8).unwrap_or(0);
                    for _ in 0..2 {
                        let digit = chars.next().and_then(|d| d.to_digit(8));
                        code = code * 8 + digit.ok_or(InvalidLiteral)?;
                    }
                    out.push(u8::try_from(code).map_err(|_| InvalidLiteral)?);
                    continue;
                }
                'u' => char_of(hex(&mut chars, 4)?)?,
                'U' => char_of(hex(&mut chars, 8)?)?,
                _ => return Err(InvalidLiteral),
            }
        };
        out.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes());
    }
    String::from_utf8(out).map_err(|_| InvalidLiteral)
}

/// The value of a Rust string whose text without quotes is `inner`: `\xHH`
/// up to `\x7F`, `\u{…}`, the single-character escapes and line
/// continuations (a `\` at the end of a line skips the line break and the
/// whitespace after it).
fn rust_value(inner: &str) -> LiteralValue {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next().ok_or(InvalidLiteral)? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '\\' => out.push('\\'),
            '0' => out.push('\0'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            'x' => {
                let code = hex(&mut chars, 2)?;
                if code > 0x7F {
                    return Err(InvalidLiteral);
                }
                out.push(char_of(code)?);
            }
            'u' => {
                if chars.next() != Some('{') {
                    return Err(InvalidLiteral);
                }
                out.push(char_of(braced(&mut chars, true)?)?);
            }
            '\n' => {
                let rest = chars.as_str().trim_start_matches([' ', '\t', '\n', '\r']);
                chars = rest.chars();
            }
            '\r' if chars.as_str().starts_with('\n') => {
                let rest = chars.as_str()[1..].trim_start_matches([' ', '\t', '\n', '\r']);
                chars = rest.chars();
            }
            _ => return Err(InvalidLiteral),
        }
    }
    Ok(out)
}

/// `n` hex digits as a number.
fn hex(chars: &mut std::str::Chars, n: usize) -> Result<u32, InvalidLiteral> {
    let mut code = 0;
    for _ in 0..n {
        let digit = chars.next().and_then(|d| d.to_digit(16));
        code = code * 16 + digit.ok_or(InvalidLiteral)?;
    }
    Ok(code)
}

/// The hex digits up to `}` of a `\u{…}` escape, at least one and at most a
/// code point's worth; Rust allows at most six digits.
fn braced(chars: &mut std::str::Chars, rust: bool) -> Result<u32, InvalidLiteral> {
    let mut code: u32 = 0;
    let mut digits = 0;
    loop {
        match chars.next().ok_or(InvalidLiteral)? {
            '}' if digits > 0 => return Ok(code),
            d => {
                code = code * 16 + d.to_digit(16).ok_or(InvalidLiteral)?;
                digits += 1;
                if code > 0x10FFFF || (rust && digits > 6) {
                    return Err(InvalidLiteral);
                }
            }
        }
    }
}

fn char_of(code: u32) -> Result<char, InvalidLiteral> {
    char::from_u32(code).ok_or(InvalidLiteral)
}

/// Every node of the tree, in preorder.
fn nodes(root: Node) -> Vec<Node> {
    let mut all = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        all.push(node);
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    all
}
