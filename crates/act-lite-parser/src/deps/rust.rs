//! Rust: `mod x;`, the leaves of each `use` tree, `extern crate`, the
//! `include!` family, the `mod` and `use` forms inside a module-level macro
//! invocation, and the names a module file declares.
//!
//! tree-sitter leaves a macro body as a flat `token_tree`. Wrapper macros
//! (`cfg_rt! { pub mod a; pub use a::B; }`) pass their items through, so at
//! module level three token forms are read, each with an optional `pub` or
//! `pub(…)`: `mod <name> ;`, `use <tree> ;` (the text is parsed again as a
//! `use` declaration), and an item keyword followed by a name. A
//! `# [ path = "…" ]` among the attributes directly before a `mod` form is
//! its `#[path]` value. A nested
//! token tree is read the same way when it is the body of a nested macro
//! invocation (`name! { … }`).

use act_lite_core::CompactRange;
use tree_sitter::Node;

use super::{
    InlineModule, ModuleNames, Relationship, RelationshipKind, nodes, range_of, text, unquote,
};
use crate::{Grammar, Parser};

const INCLUDE_MACROS: [&str; 3] = ["include", "include_str", "include_bytes"];

/// The keywords that declare a name inside a macro body.
const ITEM_KEYWORDS: [&str; 9] = [
    "const", "enum", "fn", "mod", "static", "struct", "trait", "type", "union",
];

/// The item kinds whose `name` a module file declares.
const NAMED_ITEMS: [&str; 9] = [
    "const_item",
    "enum_item",
    "function_item",
    "macro_definition",
    "static_item",
    "struct_item",
    "trait_item",
    "type_item",
    "union_item",
];

/// One leaf of a `use` tree.
struct Leaf {
    path: String,
    alias: Option<String>,
    glob: bool,
}

/// A form read from a macro body.
enum MacroItem {
    /// The name, the range and the `#[path]` value.
    Mod(String, CompactRange, Option<String>),
    Use(Vec<Leaf>, CompactRange),
    Name(String),
}

pub(super) fn extract(root: Node, source: &str) -> Vec<Relationship> {
    let mut found = Vec::new();
    for node in nodes(root) {
        match node.kind() {
            "mod_item" if node.child_by_field_name("body").is_none() => {
                if let Some(name) = node.child_by_field_name("name") {
                    found.push(Relationship {
                        path_attribute: path_attribute(node, source),
                        ..Relationship::new(RelationshipKind::Module, text(name, source), node)
                    });
                }
            }
            "use_declaration" => {
                if let Some(argument) = node.child_by_field_name("argument") {
                    for leaf in use_leaves(argument, source) {
                        found.push(Relationship::new(RelationshipKind::Use, &leaf.path, node));
                    }
                }
            }
            "extern_crate_declaration" => {
                if let Some(name) = node.child_by_field_name("name") {
                    found.push(Relationship::new(
                        RelationshipKind::ExternCrate,
                        text(name, source),
                        node,
                    ));
                }
            }
            "macro_invocation" => {
                found.extend(include(node, source));
                if at_module_level(node)
                    && let Some(tree) = token_tree(node)
                {
                    let mut items = Vec::new();
                    macro_items(tree, source, &mut items);
                    for item in items {
                        found.extend(macro_relationships(item));
                    }
                }
            }
            _ => {}
        }
    }
    found
}

fn macro_relationships(item: MacroItem) -> Vec<Relationship> {
    let relationship = |kind, specifier: String, range: &CompactRange| Relationship {
        kind,
        specifier,
        literal: true,
        range: range.clone(),
        imported: None,
        path_attribute: None,
        in_macro: true,
    };
    match item {
        MacroItem::Mod(name, range, path_attribute) => vec![Relationship {
            path_attribute,
            ..relationship(RelationshipKind::Module, name, &range)
        }],
        MacroItem::Use(leaves, range) => leaves
            .into_iter()
            .map(|leaf| relationship(RelationshipKind::Use, leaf.path, &range))
            .collect(),
        MacroItem::Name(_) => Vec::new(),
    }
}

/// The names declared at the module level of the file, and its inline
/// modules with their own names.
pub(super) fn module_names(root: Node, source: &str) -> ModuleNames {
    declared(root, source)
}

fn declared(container: Node, source: &str) -> ModuleNames {
    let mut names = ModuleNames::default();
    let mut cursor = container.walk();
    for item in container.named_children(&mut cursor) {
        match item.kind() {
            kind if NAMED_ITEMS.contains(&kind) => {
                if let Some(name) = item.child_by_field_name("name")
                    && text(name, source) != "_"
                {
                    names.names.insert(text(name, source).to_string());
                }
            }
            "mod_item" => {
                if let Some(body) = item.child_by_field_name("body")
                    && let Some(name) = item.child_by_field_name("name")
                {
                    let name = text(name, source).to_string();
                    names.names.insert(name.clone());
                    names.inline.push(InlineModule {
                        name,
                        range: range_of(item),
                        path_attribute: path_attribute(item, source),
                        names: declared(body, source),
                    });
                }
            }
            "extern_crate_declaration" => {
                if let Some(alias) = item.child_by_field_name("alias") {
                    names.names.insert(text(alias, source).to_string());
                }
            }
            "use_declaration" => {
                if let Some(argument) = item.child_by_field_name("argument") {
                    for leaf in use_leaves(argument, source) {
                        declare_leaf(&mut names, leaf);
                    }
                }
            }
            "macro_invocation" | "expression_statement" => {
                let invocation = if item.kind() == "macro_invocation" {
                    Some(item)
                } else {
                    item.named_child(0)
                        .filter(|c| c.kind() == "macro_invocation")
                };
                let mut items = Vec::new();
                if let Some(tree) = invocation.and_then(token_tree) {
                    macro_items(tree, source, &mut items);
                }
                for item in items {
                    match item {
                        MacroItem::Mod(name, ..) | MacroItem::Name(name) => {
                            names.names.insert(name);
                        }
                        MacroItem::Use(leaves, _) => {
                            for leaf in leaves {
                                declare_leaf(&mut names, leaf);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    names
}

fn declare_leaf(names: &mut ModuleNames, leaf: Leaf) {
    if leaf.glob {
        names.glob = true;
        return;
    }
    let name = leaf.alias.unwrap_or_else(|| {
        let last = leaf.path.rsplit("::").next().unwrap_or(&leaf.path);
        last.to_string()
    });
    names.names.insert(name);
}

/// The names of the `mod x;` declarations at the top level of `text`.
pub(super) fn mods_in_text(text_: &str) -> Vec<String> {
    let Ok(tree) = Parser::new().parse(text_, Grammar::Rust) else {
        return Vec::new();
    };
    let root = tree.root_node();
    let mut cursor = root.walk();
    root.named_children(&mut cursor)
        .filter(|item| item.kind() == "mod_item" && item.child_by_field_name("body").is_none())
        .filter_map(|item| item.child_by_field_name("name"))
        .map(|name| text(name, text_).to_string())
        .collect()
}

/// The leaves of a `use` tree: `a::{b, c::d}` gives `a::b` and `a::c::d`;
/// `self` in a list gives the list's prefix; `a::*` gives `a`; `a::b as c`
/// gives `a::b`.
fn use_leaves(node: Node, source: &str) -> Vec<Leaf> {
    let mut leaves = Vec::new();
    collect_leaves(node, source, "", &mut leaves);
    leaves
}

fn collect_leaves(node: Node, source: &str, prefix: &str, leaves: &mut Vec<Leaf>) {
    let leaf = |path: String, glob| Leaf {
        path,
        alias: None,
        glob,
    };
    match node.kind() {
        "use_as_clause" => {
            if let Some(path) = node.child_by_field_name("path") {
                let first = leaves.len();
                collect_leaves(path, source, prefix, leaves);
                let alias = node
                    .child_by_field_name("alias")
                    .map(|a| text(a, source).to_string());
                for leaf in &mut leaves[first..] {
                    leaf.alias = alias.clone();
                }
            }
        }
        "use_wildcard" => {
            let mut cursor = node.walk();
            let path = match node.named_children(&mut cursor).next() {
                Some(path) => join(prefix, &path_text(path, source)),
                None => prefix.to_string(),
            };
            leaves.push(leaf(path, true));
        }
        "scoped_use_list" => {
            let prefix = match node.child_by_field_name("path") {
                Some(path) => join(prefix, &path_text(path, source)),
                None => prefix.to_string(),
            };
            if let Some(list) = node.child_by_field_name("list") {
                collect_leaves(list, source, &prefix, leaves);
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if !child.kind().ends_with("comment") {
                    collect_leaves(child, source, prefix, leaves);
                }
            }
        }
        "self" if !prefix.is_empty() => leaves.push(leaf(prefix.to_string(), false)),
        _ => leaves.push(leaf(join(prefix, &path_text(node, source)), false)),
    }
}

fn join(prefix: &str, rest: &str) -> String {
    if prefix.is_empty() {
        rest.to_string()
    } else {
        format!("{prefix}::{rest}")
    }
}

fn path_text(node: Node, source: &str) -> String {
    text(node, source).split_whitespace().collect()
}

/// The value of a `#[path = "…"]` attribute among the attributes before an
/// item.
fn path_attribute(item: Node, source: &str) -> Option<String> {
    let mut sibling = item.prev_sibling();
    while let Some(node) = sibling {
        match node.kind() {
            "attribute_item" => {
                if let Some(attribute) = node.named_child(0)
                    && attribute.named_child(0).map(|p| text(p, source)) == Some("path")
                    && let Some(value) = attribute.child_by_field_name("value")
                    && let Some(path) = string_value(value, source)
                {
                    return Some(path);
                }
            }
            "line_comment" | "block_comment" => {}
            _ => break,
        }
        sibling = node.prev_sibling();
    }
    None
}

fn string_value(node: Node, source: &str) -> Option<String> {
    let written = text(node, source);
    match node.kind() {
        "string_literal" => Some(unquote(written).to_string()),
        "raw_string_literal" => {
            Some(unquote(written.trim_start_matches('r').trim_matches('#')).to_string())
        }
        _ => None,
    }
}

/// `include!`, `include_str!` and `include_bytes!`: the string literal, or
/// the argument's text when it is not one.
fn include(node: Node, source: &str) -> Option<Relationship> {
    let name = text(node.child_by_field_name("macro")?, source);
    if !INCLUDE_MACROS.contains(&name.rsplit("::").next().unwrap_or(name)) {
        return None;
    }
    let tree = token_tree(node)?;
    let mut cursor = tree.walk();
    let arguments: Vec<Node> = tree.named_children(&mut cursor).collect();
    if let [only] = arguments.as_slice()
        && let Some(value) = string_value(*only, source)
    {
        return Some(Relationship::new(RelationshipKind::Include, &value, node));
    }
    Some(Relationship {
        literal: false,
        ..Relationship::new(
            RelationshipKind::Include,
            unquote(text(tree, source)).trim(),
            node,
        )
    })
}

fn token_tree(invocation: Node) -> Option<Node> {
    let mut cursor = invocation.walk();
    invocation
        .named_children(&mut cursor)
        .find(|c| c.kind() == "token_tree")
}

/// A macro invocation directly in a file or in an inline module.
fn at_module_level(invocation: Node) -> bool {
    let mut parent = invocation.parent();
    if parent.is_some_and(|p| p.kind() == "expression_statement") {
        parent = parent.and_then(|p| p.parent());
    }
    match parent {
        Some(p) if p.kind() == "source_file" => true,
        Some(p) if p.kind() == "declaration_list" => {
            p.parent().is_some_and(|m| m.kind() == "mod_item")
        }
        _ => false,
    }
}

/// The `mod`, `use` and item-name forms of a macro body.
fn macro_items(tree: Node, source: &str, items: &mut Vec<MacroItem>) {
    let mut cursor = tree.walk();
    let all: Vec<Node> = tree.children(&mut cursor).collect();
    let Some(tokens) = all.get(1..all.len().saturating_sub(1)) else {
        return;
    };
    let kind_at = |j: usize| tokens.get(j).map(|t| t.kind());
    // The `#[path]` value among the attributes read since the last item.
    let mut attribute_path = None;
    let mut i = 0;
    while i < tokens.len() {
        let token = tokens[i];
        if token.kind() == "#"
            && let Some(attribute) = tokens
                .get(i + 1)
                .filter(|t| t.kind() == "token_tree" && text(**t, source).starts_with('['))
        {
            if let Some(path) = path_in_attribute(*attribute, source) {
                attribute_path = Some(path);
            }
            i += 2;
            continue;
        }
        let path_attribute = attribute_path.take();
        let mut j = i;
        if token.kind() == "pub" {
            j += 1;
            if tokens
                .get(j)
                .is_some_and(|t| t.kind() == "token_tree" && text(*t, source).starts_with('('))
            {
                j += 1;
            }
        }
        if kind_at(j) == Some("mod")
            && kind_at(j + 1) == Some("identifier")
            && kind_at(j + 2) == Some(";")
        {
            let name = text(tokens[j + 1], source).to_string();
            items.push(MacroItem::Mod(
                name,
                span(token, tokens[j + 2]),
                path_attribute,
            ));
            i = j + 3;
            continue;
        }
        if kind_at(j) == Some("use")
            && let Some(end) = (j + 1..tokens.len()).find(|&k| tokens[k].kind() == ";")
            && let Some(leaves) = parse_use(&source[token.start_byte()..tokens[end].end_byte()])
        {
            items.push(MacroItem::Use(leaves, span(token, tokens[end])));
            i = end + 1;
            continue;
        }
        let word = text(token, source);
        if ITEM_KEYWORDS.contains(&word) && kind_at(i + 1) == Some("identifier") {
            items.push(MacroItem::Name(text(tokens[i + 1], source).to_string()));
        } else if word == "macro_rules"
            && kind_at(i + 1) == Some("!")
            && kind_at(i + 2) == Some("identifier")
        {
            items.push(MacroItem::Name(text(tokens[i + 2], source).to_string()));
        } else if token.kind() == "token_tree"
            && i >= 2
            && kind_at(i - 1) == Some("!")
            && kind_at(i - 2) == Some("identifier")
        {
            macro_items(token, source, items);
        }
        i += 1;
    }
}

/// The value of the attribute token tree `[path = "…"]`.
fn path_in_attribute(attribute: Node, source: &str) -> Option<String> {
    let mut cursor = attribute.walk();
    let tokens: Vec<Node> = attribute.children(&mut cursor).collect();
    match tokens.as_slice() {
        [_, name, equals, value, _]
            if name.kind() == "identifier"
                && text(*name, source) == "path"
                && equals.kind() == "=" =>
        {
            string_value(*value, source)
        }
        _ => None,
    }
}

/// The leaves of a `use` form read from a macro body.
fn parse_use(text_: &str) -> Option<Vec<Leaf>> {
    let tree = Parser::new().parse(text_, Grammar::Rust).ok()?;
    let root = tree.root_node();
    if root.has_error() || root.named_child_count() != 1 {
        return None;
    }
    let declaration = root
        .named_child(0)
        .filter(|d| d.kind() == "use_declaration")?;
    let argument = declaration.child_by_field_name("argument")?;
    Some(use_leaves(argument, text_))
}

fn span(first: Node, last: Node) -> CompactRange {
    let (start, end) = (first.start_position(), last.end_position());
    CompactRange::new(
        start.row as u32 + 1,
        start.column as u32 + 1,
        end.row as u32 + 1,
        end.column as u32 + 1,
    )
}
