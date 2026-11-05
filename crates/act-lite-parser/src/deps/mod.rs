//! Relationships: the import-like constructs of a file, for JavaScript, JSX,
//! TypeScript, TSX, Python, Go and Rust.
//!
//! A relationship is one occurrence of an import construct, with its kind,
//! its target text as written (`specifier`) and the range of the construct.
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

/// One import-like construct of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Relationship {
    pub kind: RelationshipKind,
    /// The target text as written, without quotes.
    pub specifier: String,
    /// False when the target is computed (`require(name)`,
    /// `include!(concat!(…))`); `specifier` is then the argument's text.
    pub literal: bool,
    pub range: CompactRange,
    /// Python `from … import n`: `n` (`*` for a wildcard import).
    pub imported: Option<String>,
    /// Rust `mod x;`: the value of a `#[path = "…"]` attribute on it.
    pub path_attribute: Option<String>,
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
}

/// A Rust inline module and the names it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineModule {
    pub name: String,
    pub range: CompactRange,
    pub path_attribute: Option<String>,
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
