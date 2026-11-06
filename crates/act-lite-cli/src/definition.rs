//! Definition resolution for the `definition` query.
//!
//! The helpers here walk a tree-sitter parse for declaration nodes and their
//! name nodes; the resolver is shared by the CLI `definition` arm and the
//! MCP `definition` tool, so both answer with the same rules.

use std::path::{Path, PathBuf};

use act_lite_parser::{Grammar, QueryResult, SourceFile};
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
        (Grammar::Html | Grammar::Vue | Grammar::Svelte | Grammar::Astro, "attribute_value")
            if node.named_child_count() == 0 =>
        {
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
        Grammar::Html | Grammar::Vue | Grammar::Svelte | Grammar::Astro => match kind {
            "attribute"
                if child_of_kind(node, "attribute_name")
                    .is_some_and(|name| name_text(name, grammar, source) == "id") =>
            {
                first_named_chain(
                    child_of_kind(node, "quoted_attribute_value")?,
                    "attribute_value",
                )
                .filter(|value| value.named_child_count() == 0)
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
    name_at_in_tree(
        tree.root_node(),
        source.grammar(),
        tree.source(),
        line,
        column,
    )
}

/// The leaf of `root` at the 1-indexed line and byte column.
fn leaf_at(root: Node<'_>, line: u32, column: u32) -> Option<Node<'_>> {
    let point = tree_sitter::Point {
        row: line.saturating_sub(1) as usize,
        column: column.saturating_sub(1) as usize,
    };
    root.descendant_for_point_range(point, point)
}

/// [`find_symbol_name_at_location`] in one tree of a file, parsed with
/// `grammar`: the file's own tree or a region tree. `text` is the whole
/// file.
fn name_at_in_tree(
    root_node: Node<'_>,
    grammar: Grammar,
    text: &str,
    line: u32,
    column: u32,
) -> Option<String> {
    let node = leaf_at(root_node, line, column)?;
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

/// Grammars whose declarations are the entries `symbols` reports, and whose
/// names at a position are the captures of their references and symbols
/// queries.
fn named_by_queries(grammar: Grammar) -> bool {
    matches!(
        grammar,
        Grammar::Xml
            | Grammar::GraphQl
            | Grammar::Protobuf
            | Grammar::Make
            | Grammar::CMake
            | Grammar::Nix
            | Grammar::Zig
            | Grammar::R
            | Grammar::Julia
    )
}

/// The names at the 1-indexed position: the identifier there, else the
/// innermost `@reference` or `@name` capture that contains it; then, in Nix
/// and Julia, the names around that identifier (each enclosing `attrpath`,
/// or Julia `field_expression`, `parametrized_type_expression` or
/// `parenthesized_expression`, climbing through a `typed_expression`),
/// innermost first.
fn names_at(source: &SourceFile, line: u32, column: u32) -> Vec<String> {
    let tree = source.tree();
    let bytes = tree.source().as_bytes();
    let point = tree_sitter::Point {
        row: line.saturating_sub(1) as usize,
        column: column.saturating_sub(1) as usize,
    };
    let Some(leaf) = tree.root_node().descendant_for_point_range(point, point) else {
        return Vec::new();
    };
    if !is_identifier_kind(leaf.kind()) {
        return innermost_capture(source, line, column)
            .into_iter()
            .collect();
    }
    let mut names: Vec<String> = leaf
        .utf8_text(bytes)
        .map(str::to_string)
        .into_iter()
        .collect();
    let enclosing: &[&str] = match source.grammar() {
        Grammar::Nix => &["attrpath"],
        Grammar::Julia => &[
            "field_expression",
            "parametrized_type_expression",
            "parenthesized_expression",
            "typed_expression",
        ],
        _ => return names,
    };
    let mut node = leaf;
    while let Some(parent) = node
        .parent()
        .filter(|parent| enclosing.contains(&parent.kind()))
    {
        if parent.kind() != "typed_expression"
            && let Ok(text) = parent.utf8_text(bytes)
        {
            names.push(text.to_string());
        }
        node = parent;
    }
    names
}

/// The text of the innermost `@reference` (`references.scm`) or `@name`
/// (`symbols.scm`) capture that contains the 1-indexed position.
fn innermost_capture(source: &SourceFile, line: u32, column: u32) -> Option<String> {
    let support = source.grammar().language_support();
    let at = (line, column);
    [support.references_query(), support.symbols_query()]
        .into_iter()
        .filter_map(|query| act_lite_parser::execute_raw_query(source, query).ok())
        .flatten()
        .flat_map(|found| found.captures)
        .filter(|capture| {
            let (start, end) = (capture.range.start(), capture.range.end());
            matches!(capture.name.as_str(), "reference" | "name")
                && (start.line(), start.column()) <= at
                && at < (end.line(), end.column())
        })
        .min_by_key(|capture| {
            let (start, end) = (capture.range.start(), capture.range.end());
            (
                std::cmp::Reverse((start.line(), start.column())),
                (end.line(), end.column()),
            )
        })
        .map(|capture| capture.text)
}

pub(crate) fn collect_definition_ranges_in_source(
    source: &SourceFile,
    symbol_name: &str,
) -> Vec<act_lite_core::Range> {
    if named_by_queries(source.grammar()) {
        return match act_lite_parser::execute_symbols(source) {
            Ok(QueryResult::Symbols { symbols }) => symbols
                .into_iter()
                .filter(|symbol| symbol.name() == symbol_name)
                .map(|symbol| symbol.range().clone())
                .collect(),
            _ => Vec::new(),
        };
    }
    let tree = source.tree();
    definition_ranges_in_tree(
        tree.root_node(),
        source.grammar(),
        tree.source(),
        source.path(),
        symbol_name,
    )
}

/// The declarations named `symbol_name` in one tree of a file, parsed with
/// `grammar`: the file's own tree or a region tree. `text` is the whole
/// file. In the outer tree of a component file, a declaration where nothing
/// is reported ([`act_lite_parser::regions::StructureFilter`]) is not one.
fn definition_ranges_in_tree(
    root_node: Node<'_>,
    grammar: Grammar,
    text: &str,
    path: &Path,
    symbol_name: &str,
) -> Vec<act_lite_core::Range> {
    let filter = act_lite_parser::regions::StructureFilter::new(grammar, root_node, text);
    let mut stack = vec![root_node];
    let mut ranges = Vec::new();
    while let Some(node) = stack.pop() {
        let name_node = if is_definition_node_kind(node.kind()) {
            definition_name_node(node, None)
        } else {
            declaration_name(node, grammar, text)
        };
        if let Some(name_node) = name_node
            && name_text(name_node, grammar, text) == symbol_name
            && !filter.hides(name_node)
        {
            ranges.push(node_to_range(path, name_node, text));
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

/// Every file under `root` that a walk from `anchor` reads for any of
/// `grammars` ([`Grammar::walk_reads`]), skipping `.git`, `target` and
/// `node_modules`. Unreadable directories contribute nothing. A symlink is
/// followed only while its target stays inside `root`, and each directory is
/// entered once, by its canonical path. Files over
/// [`act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES`] are skipped.
pub(crate) fn collect_workspace_paths(
    root: &Path,
    anchor: &Path,
    grammars: &[Grammar],
    paths: &mut Vec<PathBuf>,
) {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut visited = std::collections::HashSet::from([canonical_root.clone()]);
    collect_paths_inside(root, &canonical_root, &mut visited, anchor, grammars, paths);
}

fn collect_paths_inside(
    dir: &Path,
    canonical_root: &Path,
    visited: &mut std::collections::HashSet<PathBuf>,
    anchor: &Path,
    grammars: &[Grammar],
    paths: &mut Vec<PathBuf>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(canonical) = path
            .canonicalize()
            .ok()
            .filter(|canonical| canonical.starts_with(canonical_root))
        else {
            continue;
        };
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, ".git" | "target" | "node_modules"));
            if !skip && visited.insert(canonical) {
                collect_paths_inside(&path, canonical_root, visited, anchor, grammars, paths);
            }
        } else if path.is_file()
            && grammars
                .iter()
                .any(|grammar| grammar.walk_reads(anchor, &path))
            && std::fs::metadata(&path)
                .is_ok_and(|meta| meta.len() <= act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES)
        {
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
/// The names at the position are the identifier there and, in Nix and
/// Julia, the dotted names around it ([`names_at`]). A non-blank `symbol`
/// argument that is none of them is looked up as given, except in the
/// grammars of [`named_by_queries`], where it is not found. Otherwise the
/// symbol is the outermost name at the position that has a declaration,
/// else the innermost. Candidates are the declarations in the queried file
/// plus every same-grammar file under `root` (unreadable siblings skipped),
/// and the winner is the range closest to the position by [`range_priority`].
pub(crate) fn resolve_definition(
    source: &SourceFile,
    root: &Path,
    symbol: &str,
    line: u32,
    column: u32,
) -> Result<(String, act_lite_core::Range), DefinitionError> {
    if source.grammar().is_component() {
        return resolve_component_definition(source, root, symbol, line, column);
    }
    let names = if named_by_queries(source.grammar()) {
        names_at(source, line, column)
    } else {
        find_symbol_name_at_location(source, line, column)
            .into_iter()
            .collect()
    };
    if names.is_empty() {
        return Err(DefinitionError::NoSymbolAtPosition);
    }
    let lookup = if symbol.trim().is_empty() || names.iter().any(|name| name == symbol) {
        names
    } else if named_by_queries(source.grammar()) {
        return Err(DefinitionError::NotFound {
            symbol: symbol.to_string(),
        });
    } else {
        vec![symbol.to_string()]
    };
    let found = workspace_candidates(source, root, &lookup);
    let (symbol_name, candidates) = lookup
        .iter()
        .cloned()
        .zip(found)
        .rev()
        .find(|(_, candidates)| !candidates.is_empty())
        .unwrap_or_else(|| (lookup[0].clone(), Vec::new()));

    let path = source.path();
    let best = candidates
        .into_iter()
        .min_by_key(|range| range_priority(range, path, line, column))
        .ok_or_else(|| DefinitionError::NotFound {
            symbol: symbol_name.clone(),
        })?;
    Ok((symbol_name, best))
}

/// [`resolve_definition`] from a component file. At a position in a template
/// expression there is no symbol. At a position in the outer document, the
/// name there is looked up in the outer trees of the file and of every file
/// of the same component grammar under `root`. At a position in a region of
/// language L, it is looked up in the regions of L of those files and in
/// every file of L under `root`.
fn resolve_component_definition(
    source: &SourceFile,
    root: &Path,
    symbol: &str,
    line: u32,
    column: u32,
) -> Result<(String, act_lite_core::Range), DefinitionError> {
    use act_lite_parser::regions::{Position, outside_structure, position_at};
    let component = source.grammar();
    let text = source.content();
    let position = byte_at(text, line, column).map_or(Position::TemplateExpression, |byte| {
        position_at(source, byte)
    });
    // The tree searched at the position, and its grammar.
    let (root_node, grammar) = match position {
        Position::TemplateExpression => return Err(DefinitionError::NoSymbolAtPosition),
        Position::Outer => (source.tree().root_node(), component),
        Position::Region(index) => {
            let tree = source
                .region_trees()
                .iter()
                .find(|tree| tree.region == index)
                .ok_or(DefinitionError::NoSymbolAtPosition)?;
            (tree.tree.root_node(), tree.grammar)
        }
    };
    if grammar == component
        && leaf_at(root_node, line, column)
            .is_none_or(|leaf| outside_structure(grammar, root_node, leaf, text))
    {
        return Err(DefinitionError::NoSymbolAtPosition);
    }
    let found = name_at_in_tree(root_node, grammar, text, line, column)
        .ok_or(DefinitionError::NoSymbolAtPosition)?;
    let symbol_name = if symbol.trim().is_empty() || symbol == found {
        found
    } else {
        symbol.to_string()
    };

    // The trees of one component or plain file that hold the declarations.
    let ranges_in = |file: &SourceFile| -> Vec<act_lite_core::Range> {
        let in_tree = |root_node| {
            definition_ranges_in_tree(
                root_node,
                grammar,
                file.content(),
                file.path(),
                &symbol_name,
            )
        };
        if grammar == component {
            in_tree(file.tree().root_node())
        } else if file.grammar() == component {
            file.region_trees()
                .iter()
                .filter(|tree| tree.grammar == grammar)
                .flat_map(|tree| in_tree(tree.tree.root_node()))
                .collect()
        } else {
            in_tree(file.tree().root_node())
        }
    };
    let path = source.path();
    let mut candidates = ranges_in(source);
    let mut grammars = vec![component];
    if grammar != component {
        grammars.push(grammar);
    }
    let mut workspace_paths = Vec::new();
    collect_workspace_paths(root, path, &grammars, &mut workspace_paths);
    for other in workspace_paths {
        if other == path {
            continue;
        }
        // A sibling that cannot be read or parsed has no definitions to offer.
        let Ok(other_source) = SourceFile::load_within(&other, root) else {
            continue;
        };
        candidates.extend(ranges_in(&other_source));
    }
    let best = candidates
        .into_iter()
        .min_by_key(|range| range_priority(range, path, line, column))
        .ok_or_else(|| DefinitionError::NotFound {
            symbol: symbol_name.clone(),
        })?;
    Ok((symbol_name, best))
}

/// The byte offset of a 1-indexed line and byte column of `text`, or `None`
/// when the position lies past the end of its line or of the text.
fn byte_at(text: &str, line: u32, column: u32) -> Option<usize> {
    let start = if line <= 1 {
        0
    } else {
        text.match_indices('\n').nth(line as usize - 2)?.0 + 1
    };
    let end = text[start..].find('\n').map_or(text.len(), |i| start + i);
    let byte = start + (column as usize).checked_sub(1)?;
    (line > 0 && byte <= end).then_some(byte)
}

/// The declarations of each of `names`, in order, in `source` and in every
/// same-grammar file under `root`; each file is read and parsed once.
fn workspace_candidates(
    source: &SourceFile,
    root: &Path,
    names: &[String],
) -> Vec<Vec<act_lite_core::Range>> {
    let path = source.path();
    let mut candidates: Vec<_> = names
        .iter()
        .map(|name| collect_definition_ranges_in_source(source, name))
        .collect();
    let mut workspace_paths = Vec::new();
    collect_workspace_paths(root, path, &[source.grammar()], &mut workspace_paths);
    for other in workspace_paths {
        if other == path {
            continue;
        }
        // A sibling that cannot be read or parsed has no definitions to offer.
        let Ok(other_source) = SourceFile::load_within(&other, root) else {
            continue;
        };
        for (name, ranges) in names.iter().zip(&mut candidates) {
            ranges.extend(collect_definition_ranges_in_source(&other_source, name));
        }
    }
    candidates
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

    #[cfg(unix)]
    #[test]
    fn each_file_is_collected_once_despite_symlinks_back_to_the_root() {
        let dir = tempfile::tempdir().unwrap();
        for file in ["a.py", "b.py"] {
            std::fs::write(dir.path().join(file), "x = 1\n").unwrap();
        }
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        for link in ["up", "up2"] {
            std::os::unix::fs::symlink("..", dir.path().join("sub").join(link)).unwrap();
        }
        let mut paths = Vec::new();
        collect_workspace_paths(
            dir.path(),
            &dir.path().join("a.py"),
            &[Grammar::Python],
            &mut paths,
        );
        assert_eq!(paths.len(), 2, "{paths:?}");
    }

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
