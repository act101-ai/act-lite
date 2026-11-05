//! Definition resolution for the `definition` query.
//!
//! The helpers here walk a tree-sitter parse for declaration nodes and their
//! name nodes; the resolver is shared by the CLI `definition` arm and the
//! MCP `definition` tool, so both answer with the same rules.

use std::path::{Path, PathBuf};

use act_lite_parser::{Grammar, SourceFile};
use tree_sitter::Node;

pub(crate) fn is_identifier_kind(kind: &str) -> bool {
    matches!(
        kind,
        "identifier"
            | "type_identifier"
            | "field_identifier"
            | "package_identifier"
            | "simple_identifier"
    )
}

pub(crate) fn is_definition_node_kind(kind: &str) -> bool {
    matches!(
        kind,
        "function_declaration"
            | "function_definition"
            | "method_declaration"
            | "method_elem"
            | "class_declaration"
            | "class_definition"
            | "type_alias_declaration"
            | "type_declaration"
            | "type_spec"
            | "interface_declaration"
            | "interface_type"
            | "struct_type"
            | "field_declaration"
            | "variable_declarator"
            | "var_declaration"
            | "var_spec"
            | "const_declaration"
            | "const_spec"
            | "short_var_declaration"
            | "method_definition"
            | "method"
            | "singleton_method"
            | "function_signature"
            | "object_pattern"
            | "array_pattern"
            | "rest_pattern"
            | "parameter"
            | "formal_parameters"
            | "property_declaration"
            | "companion_object"
            | "object_declaration"
            | "class_parameter"
            | "value_parameter"
            | "function_item"
            | "function_signature_item"
            | "struct_item"
            | "enum_item"
            | "trait_item"
            | "impl_item"
            | "const_item"
            | "static_item"
            | "let_declaration"
            | "type_item"
            | "mod_item"
            | "union_item"
            | "macro_definition"
            | "enum_variant"
    )
}

/// An identifier, a Bash `word` naming the command of a call, or a leaf that
/// names a declaration in a data, markup, style or query language.
fn is_symbol_node(node: Node<'_>, grammar: Grammar, source: &str) -> bool {
    is_identifier_kind(node.kind())
        || (node.kind() == "word"
            && node
                .parent()
                .is_some_and(|parent| parent.kind() == "command_name"))
        || is_data_name_leaf(node, grammar, source)
}

/// Leaves that can be named in these grammars; their declarations are found
/// by [`declaration_name`].
fn is_data_name_leaf(node: Node<'_>, grammar: Grammar, source: &str) -> bool {
    match (grammar, node.kind()) {
        (Grammar::Css, "id_name")
        | (Grammar::Dockerfile, "image_alias")
        | (Grammar::Toml, "bare_key")
        | (Grammar::Yaml, "string_scalar") => true,
        (Grammar::Dockerfile, "param") => name_text(node, grammar, source).len() < node_len(node),
        (Grammar::Html, "attribute_value") => {
            let mut ancestor = node.parent();
            if ancestor.is_some_and(|parent| parent.kind() == "quoted_attribute_value") {
                ancestor = ancestor.and_then(|parent| parent.parent());
            }
            ancestor.is_some_and(|attribute| {
                attribute.kind() == "attribute"
                    && child_of_kind(attribute, "attribute_name")
                        .is_some_and(|name| name_text(name, grammar, source) == "id")
            })
        }
        (Grammar::Json | Grammar::Jsonc, "string_content") => node
            .parent()
            .and_then(|string| string.parent().map(|pair| (string, pair)))
            .is_some_and(|(string, pair)| {
                pair.kind() == "pair"
                    && pair
                        .child_by_field_name("key")
                        .is_some_and(|key| key.id() == string.id())
            }),
        (Grammar::HclTerraform, "template_literal") => node
            .parent()
            .and_then(|label| label.parent())
            .is_some_and(|block| block.kind() == "block"),
        (Grammar::Markdown, "text") => node
            .parent()
            .is_some_and(|parent| parent.kind() == "heading_content"),
        _ => false,
    }
}

fn node_len(node: Node<'_>) -> usize {
    node.end_byte() - node.start_byte()
}

/// The name a node spells: its text without surrounding whitespace (a
/// Markdown heading's text starts with a space), or the stage of a
/// Dockerfile `--from=<stage>` flag.
fn name_text<'s>(node: Node<'_>, grammar: Grammar, source: &'s str) -> &'s str {
    let text = source
        .get(node.start_byte()..node.end_byte())
        .unwrap_or_default()
        .trim();
    match (grammar, node.kind()) {
        (Grammar::Dockerfile, "param") => text.strip_prefix("--from=").unwrap_or(text),
        _ => text,
    }
}

/// The name node of an Elixir `def`-family call: the first argument's call
/// target, or the first argument itself (`def hello, do: 1`, `defmodule Foo`).
fn elixir_definition_name<'a>(node: Node<'a>, source: &str) -> Option<Node<'a>> {
    if node.kind() != "call" {
        return None;
    }
    let target = node.child_by_field_name("target")?;
    if !matches!(
        target.utf8_text(source.as_bytes()).ok()?,
        "def" | "defp" | "defmacro" | "defmacrop"
    ) {
        return None;
    }
    let mut cursor = node.walk();
    let arguments = node
        .children(&mut cursor)
        .find(|child| child.kind() == "arguments")?;
    let first = arguments.named_child(0)?;
    match first.kind() {
        "call" => first.child_by_field_name("target"),
        "identifier" | "alias" => Some(first),
        _ => None,
    }
}

/// The first node of `kind` on the chain of first named children from `node`.
fn first_named_chain<'a>(mut node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    loop {
        if node.kind() == kind {
            return Some(node);
        }
        node = node.named_child(0)?;
    }
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .find(|child| child.kind() == kind)
}

/// The name node of a declaration in a grammar whose declarations are not in
/// [`is_definition_node_kind`]: Elixir `def` calls and the data, markup,
/// style and query languages.
fn declaration_name<'a>(node: Node<'a>, grammar: Grammar, source: &str) -> Option<Node<'a>> {
    let kind = node.kind();
    match grammar {
        Grammar::Elixir => elixir_definition_name(node, source),
        Grammar::Css => match kind {
            "class_name" => node.named_child(0),
            "id_name" => Some(node),
            _ => None,
        },
        Grammar::Scss => match kind {
            "mixin_statement" | "function_statement" => node.child_by_field_name("name"),
            _ => None,
        },
        Grammar::Dockerfile => match kind {
            "from_instruction" => node.child_by_field_name("as"),
            _ => None,
        },
        Grammar::HclTerraform => match kind {
            "string_lit" if node.parent().is_some_and(|parent| parent.kind() == "block") => {
                child_of_kind(node, "template_literal")
            }
            _ => None,
        },
        Grammar::Html => match kind {
            "attribute"
                if child_of_kind(node, "attribute_name")
                    .is_some_and(|name| name_text(name, grammar, source) == "id") =>
            {
                first_named_chain(
                    child_of_kind(node, "quoted_attribute_value")?,
                    "attribute_value",
                )
            }
            _ => None,
        },
        Grammar::Json | Grammar::Jsonc => match kind {
            "pair" => first_named_chain(node.child_by_field_name("key")?, "string_content"),
            _ => None,
        },
        Grammar::Markdown => match kind {
            "atx_heading" => first_named_chain(child_of_kind(node, "heading_content")?, "text"),
            _ => None,
        },
        Grammar::Sql => match kind {
            "column_definition" => node.child_by_field_name("name"),
            _ if kind.starts_with("create_") => {
                child_of_kind(node, "object_reference")?.child_by_field_name("name")
            }
            _ => None,
        },
        Grammar::Toml => match kind {
            "table" | "pair" => child_of_kind(node, "bare_key"),
            _ => None,
        },
        Grammar::Yaml => match kind {
            "block_mapping_pair" => {
                first_named_chain(node.child_by_field_name("key")?, "string_scalar")
            }
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn extract_identifier_from_declarator(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if is_identifier_kind(node.kind()) {
            return Some(node);
        }
        if let Some(next) = node.child_by_field_name("declarator") {
            node = next;
            continue;
        }
        let mut cursor = node.walk();
        return node
            .children(&mut cursor)
            .find(|child| is_identifier_kind(child.kind()));
    }
}

pub(crate) fn definition_name_node<'a>(
    declaration: Node<'a>,
    identifier_hint: Option<Node<'a>>,
) -> Option<Node<'a>> {
    if is_identifier_kind(declaration.kind()) {
        return Some(declaration);
    }
    if declaration.kind() == "short_var_declaration"
        && let Some(left) = declaration.child_by_field_name("left")
        && let Some(first) = left.child(0)
    {
        return Some(first);
    }
    if let Some(name_node) = declaration.child_by_field_name("name") {
        return Some(name_node);
    }
    if let Some(declarator_node) = declaration.child_by_field_name("declarator")
        && let Some(identifier) = extract_identifier_from_declarator(declarator_node)
    {
        return Some(identifier);
    }
    let mut cursor = declaration.walk();
    if let Some(found) = declaration
        .children(&mut cursor)
        .find(|child| matches!(child.kind(), "simple_identifier" | "type_identifier"))
    {
        return Some(found);
    }
    identifier_hint
}

/// The range of `node`, without leading whitespace in its text.
fn node_to_range(file_path: &Path, node: Node<'_>, source: &str) -> act_lite_core::Range {
    let start = node.start_position();
    let end = node.end_position();
    let leading = source
        .get(node.start_byte()..node.end_byte())
        .map_or(0, |text| text.len() - text.trim_start().len());
    act_lite_core::Range::new(
        act_lite_core::Location::full(
            file_path.to_path_buf(),
            start.row as u32 + 1,
            (start.column + leading) as u32 + 1,
            node.start_byte() + leading,
        ),
        act_lite_core::Location::full(
            file_path.to_path_buf(),
            end.row as u32 + 1,
            end.column as u32 + 1,
            node.end_byte(),
        ),
    )
}

pub(crate) fn find_symbol_name_at_location(
    source: &SourceFile,
    line: u32,
    column: u32,
) -> Option<String> {
    let tree = source.tree();
    let root_node = tree.root_node();
    let row = line.saturating_sub(1) as usize;
    let col = column.saturating_sub(1) as usize;
    let node = root_node.descendant_for_point_range(
        tree_sitter::Point { row, column: col },
        tree_sitter::Point { row, column: col },
    )?;

    let grammar = source.grammar();
    let text = tree.source();
    let mut current = node;
    let mut identifier_node = is_symbol_node(current, grammar, text).then_some(current);
    let mut depth = 0usize;
    const MAX_DEPTH: usize = 20;
    while !is_definition_node_kind(current.kind()) {
        let Some(parent) = current.parent() else {
            break;
        };
        if is_symbol_node(current, grammar, text) {
            identifier_node = identifier_node.or(Some(current));
        }
        current = parent;
        depth += 1;
        if depth > MAX_DEPTH {
            break;
        }
    }

    let name_node = if is_definition_node_kind(current.kind()) {
        definition_name_node(current, identifier_node)
    } else {
        identifier_node.or_else(|| is_symbol_node(node, grammar, text).then_some(node))
    }?;
    Some(name_text(name_node, grammar, text).to_string())
}

pub(crate) fn collect_definition_ranges_in_source(
    source: &SourceFile,
    symbol_name: &str,
) -> Vec<act_lite_core::Range> {
    let tree = source.tree();
    let mut stack = vec![tree.root_node()];
    let mut ranges = Vec::new();
    let grammar = source.grammar();
    while let Some(node) = stack.pop() {
        let name_node = if is_definition_node_kind(node.kind()) {
            definition_name_node(node, None)
        } else {
            declaration_name(node, grammar, tree.source())
        };
        if let Some(name_node) = name_node
            && name_text(name_node, grammar, tree.source()) == symbol_name
        {
            ranges.push(node_to_range(source.path(), name_node, tree.source()));
        }
        let mut cursor = node.walk();
        stack.extend(node.children(&mut cursor));
    }
    ranges
}

pub(crate) fn range_priority(
    range: &act_lite_core::Range,
    file_path: &Path,
    line: u32,
    column: u32,
) -> (u8, u32, u32) {
    let same_file = range.start().file() == file_path;
    let contains_location = same_file
        && range.start().line() <= line
        && range.end().line() >= line
        && (range.start().line() != line || range.start().column() <= column)
        && (range.end().line() != line || range.end().column() >= column);
    let line_distance = range.start().line().abs_diff(line);
    let column_distance = range.start().column().abs_diff(column);
    match (contains_location, same_file) {
        (true, _) => (0, line_distance, column_distance),
        (false, true) => (1, line_distance, column_distance),
        (false, false) => (2, line_distance, column_distance),
    }
}

/// Every file under `root` that a walk from `anchor`, a file in `grammar`,
/// reads ([`Grammar::walk_reads`]), skipping `.git`, `target` and
/// `node_modules`. Unreadable directories contribute nothing. A symlink is
/// followed only while its target stays inside `root`.
pub(crate) fn collect_workspace_paths(
    root: &Path,
    anchor: &Path,
    grammar: Grammar,
    paths: &mut Vec<PathBuf>,
) {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    collect_paths_inside(root, &canonical_root, anchor, grammar, paths);
}

fn collect_paths_inside(
    dir: &Path,
    canonical_root: &Path,
    anchor: &Path,
    grammar: Grammar,
    paths: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path
            .canonicalize()
            .is_ok_and(|canonical| canonical.starts_with(canonical_root))
        {
            continue;
        }
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules"));
            if !skip {
                collect_paths_inside(&path, canonical_root, anchor, grammar, paths);
            }
        } else if path.is_file() && grammar.walk_reads(anchor, &path) {
            paths.push(path);
        }
    }
}

/// Why a definition could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DefinitionError {
    /// No identifier sits at the queried position.
    NoSymbolAtPosition,
    /// The named symbol is defined in no file of the workspace.
    NotFound { symbol: String },
}

/// Resolves the definition of a symbol used at `line`/`column` of `source`.
///
/// The symbol name is the non-blank `symbol` argument, else the identifier
/// at the position. Candidates are the declarations in the queried file plus
/// every same-grammar file under `root` (unreadable siblings skipped), and
/// the winner is the range closest to the position by [`range_priority`].
pub(crate) fn resolve_definition(
    source: &SourceFile,
    root: &Path,
    symbol: &str,
    line: u32,
    column: u32,
) -> Result<(String, act_lite_core::Range), DefinitionError> {
    let location_symbol = find_symbol_name_at_location(source, line, column)
        .ok_or(DefinitionError::NoSymbolAtPosition)?;
    let symbol_name = if symbol.trim().is_empty() || symbol == location_symbol {
        location_symbol
    } else {
        symbol.to_string()
    };

    let path = source.path();
    let mut candidates = collect_definition_ranges_in_source(source, &symbol_name);
    let mut workspace_paths = Vec::new();
    collect_workspace_paths(root, path, source.grammar(), &mut workspace_paths);
    for other in workspace_paths {
        if other == path {
            continue;
        }
        // A sibling that cannot be read or parsed has no definitions to offer.
        let Ok(other_source) = SourceFile::load(&other) else {
            continue;
        };
        candidates.extend(collect_definition_ranges_in_source(
            &other_source,
            &symbol_name,
        ));
    }

    let best = candidates
        .into_iter()
        .min_by_key(|range| range_priority(range, path, line, column))
        .ok_or_else(|| DefinitionError::NotFound {
            symbol: symbol_name.clone(),
        })?;
    Ok((symbol_name, best))
}

/// The definition payload: `{"symbol", "definition": {file, line, column,
/// end_line, end_column}}`.
pub(crate) fn definition_json(symbol: &str, range: &act_lite_core::Range) -> serde_json::Value {
    serde_json::json!({
        "symbol": symbol,
        "definition": {
            "file": range.start().file().display().to_string(),
            "line": range.start().line(),
            "column": range.start().column(),
            "end_line": range.end().line(),
            "end_column": range.end().column(),
        }
    })
}

/// The error text when no identifier sits at the position.
pub(crate) fn no_symbol_text(file: &Path, line: u32, column: u32) -> String {
    format!(
        "No symbol found at {}:{}:{}. Use `symbols` to list the file's symbols and get valid line/column values.",
        file.display(),
        line,
        column
    )
}

/// The error text when the symbol is defined in no file of the workspace.
pub(crate) fn not_found_text(symbol: &str, file: &Path, line: u32, column: u32) -> String {
    format!(
        "No definition found for symbol '{}' from {}:{}:{}",
        symbol,
        file.display(),
        line,
        column
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_singleton_method_is_found_from_its_call_site() {
        let content = "class A\n  def self.make\n  end\nend\nA.make\n";
        let source = SourceFile::from_content("a.rb", content.to_string(), Grammar::Ruby).unwrap();
        let ranges = collect_definition_ranges_in_source(&source, "make");
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start().line(), 2);
    }

    fn symbol_at(name: &str, content: &str, grammar: Grammar, col: u32) -> Option<String> {
        let source = SourceFile::from_content(name, content.to_string(), grammar).unwrap();
        find_symbol_name_at_location(&source, 1, col)
    }

    #[test]
    fn html_class_value_is_not_a_symbol() {
        let (a, b) = (
            "<div class=\"x\" id=\"x\"></div>",
            "<div id=\"x\" class=\"x\"></div>",
        );
        assert_eq!(symbol_at("a.html", a, Grammar::Html, 13), None);
        assert_eq!(symbol_at("b.html", b, Grammar::Html, 20), None);
        assert_eq!(
            symbol_at("a.html", a, Grammar::Html, 20).as_deref(),
            Some("x")
        );
        assert_eq!(
            symbol_at("b.html", b, Grammar::Html, 10).as_deref(),
            Some("x")
        );
    }

    #[test]
    fn json_string_value_is_not_a_symbol() {
        let content = "{\"name\": \"name\", \"other\": {\"name\": 2}}";
        assert_eq!(symbol_at("a.json", content, Grammar::Json, 12), None);
        assert_eq!(
            symbol_at("a.json", content, Grammar::Json, 3).as_deref(),
            Some("name")
        );
        assert_eq!(
            symbol_at("a.json", content, Grammar::Json, 29).as_deref(),
            Some("name")
        );
    }
}
