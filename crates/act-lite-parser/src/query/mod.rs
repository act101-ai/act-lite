//! Query types and execution.

pub mod locals;

pub use locals::{LocalDefinition, LocalScope, LocalsModel};

use crate::{Grammar, NodeKind, SourceFile, error::Result};
use act_lite_core::{Location, Range, Symbol, SymbolKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use streaming_iterator::StreamingIterator;

/// All supported query types (17 total).
///
/// Some queries are implemented via tree-sitter,
/// others require LSP integration (dispatched from CLI layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueryType {
    // Tree-sitter implementable (Phase 3)
    Skeleton,
    Symbol,
    Symbols,
    Definition,

    // LSP-dependent (dispatched from CLI)
    References,
    Callers,
    Callees,
    Implementations,
    Hierarchy,
    Type,
    Diagnostics,
    Dependencies,
    Impact,
    Unused,
    Tests,
    Coverage,
    Suggest,
}

impl QueryType {
    /// Returns true if this query type is implemented in Phase 3.
    pub fn is_tree_sitter_only(&self) -> bool {
        matches!(
            self,
            QueryType::Skeleton
                | QueryType::Symbol
                | QueryType::Symbols
                | QueryType::References
                | QueryType::Definition
        )
    }

    /// Returns true if this query requires LSP.
    pub fn requires_lsp(&self) -> bool {
        !self.is_tree_sitter_only()
    }
}

/// Result of a skeleton query (file structure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonResult {
    pub declarations: Vec<Declaration>,
}

/// A top-level declaration in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Declaration {
    pub kind: String,
    pub name: String,
    pub range: Range,
    /// Span of the declaration's NAME node, when the pattern bound a distinct
    /// `@name` capture (the first one, for the patterns that compose a name
    /// from several). `None` when the grammar's pattern has no separate name
    /// node — the self-naming captures (`@import`/`@export`/`@include`/
    /// `@use`/`@require`) name themselves from the declaration node's own
    /// text, so there is nothing distinct to point at.
    ///
    /// This is NOT a second identity for the declaration: `range` remains the
    /// declaration's span, and every consumer that addresses or keys a
    /// declaration (graph node ranges, the pass-four callee cache) keeps using
    /// it. `name_range` exists for consumers that must ASK ABOUT the symbol at
    /// a position — a language server's `prepareCallHierarchy` answers at the
    /// identifier and returns `null` at the leading keyword (`def`, `fn`,
    /// `function`), so querying `range.start()` yields nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name_range: Option<Range>,
    /// Visibility for grammars that model it (`"public"`/`"internal"`/
    /// `"private"` for Rust — restricted `pub(crate)`/`pub(super)`/
    /// `pub(in ...)` is `"internal"`; `"export"`/`"private"` for the TS/JS
    /// family). `None` means the grammar doesn't model visibility — NOT
    /// "private" and NOT "public"; consumers document their own None policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}

/// Result of a symbol query (single symbol).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolResult {
    pub symbol: Symbol,
    pub source: String,
    pub context_before: String,
    pub context_after: String,
}

/// Diagnostic information from an LSP query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticInfo {
    pub range: Range,
    pub severity: String,
    pub message: String,
    pub source: Option<String>,
    pub code: Option<String>,
}

/// Import/dependency information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub module_path: String,
    pub range: Range,
    /// "import", "require", "use", etc.
    pub kind: String,
}

/// A suggested code action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionInfo {
    pub title: String,
    pub kind: Option<String>,
    pub is_preferred: bool,
}

/// A file skipped during a cross-file query workspace walk (e.g. the
/// `query references` tree-sitter fallback), with the reason it was not
/// scanned. Part of the honesty surface: skips are disclosed, never silent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedFile {
    pub file: PathBuf,
    pub reason: String,
}

pub const MAX_REFERENCE_WALK_FILES: usize = 10_000;

/// Cap on per-file bytes a references workspace walk will load. Oversize
/// siblings are skipped and disclosed, never silently dropped.
pub const MAX_REFERENCE_WALK_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Outcome of a references workspace walk: the loadable same-grammar siblings
/// plus the disclosed skip list.
pub struct ReferencesWalk {
    pub siblings: Vec<SourceFile>,
    pub skipped: Vec<SkippedFile>,
}

fn path_is_reference_sibling(anchor: &Path, path: &Path, grammar: Grammar) -> bool {
    grammar.walk_reads(anchor, path)
}

pub fn walk_reference_siblings(
    root: &Path,
    anchor: &Path,
    grammar: Grammar,
    max_files: usize,
    max_file_bytes: u64,
) -> Result<ReferencesWalk> {
    let anchor_canonical = std::fs::canonicalize(anchor).unwrap_or_else(|_| anchor.to_path_buf());

    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut skipped: Vec<SkippedFile> = Vec::new();
    let entries = walkdir::WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| {
            // Keep the root itself; prune every hidden path component.
            entry.depth() == 0 || !entry.file_name().to_string_lossy().starts_with('.')
        });
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                // Disclose even pathless errors (e.g. the root itself
                // unreadable) against the walked root — never silent.
                let file = err
                    .path()
                    .map_or_else(|| root.to_path_buf(), Path::to_path_buf);
                skipped.push(SkippedFile {
                    file,
                    reason: format!("walk error: {err}"),
                });
                continue;
            }
        };
        let path = entry.path();
        if entry.depth() > 0 && entry.path_is_symlink() {
            let family_relevant = path_is_reference_sibling(anchor, path, grammar);
            let reason = match std::fs::metadata(path) {
                Err(err) => Some(format!("symlink target unreadable: {err}")),
                Ok(meta) if meta.is_file() && !family_relevant => None,
                Ok(_) => Some(format!("symlink not followed: {}", path.display())),
            };
            if let Some(reason) = reason {
                skipped.push(SkippedFile {
                    file: path.to_path_buf(),
                    reason,
                });
            }
            continue;
        }
        if !entry.file_type().is_file() || !path_is_reference_sibling(anchor, path, grammar) {
            continue;
        }
        let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if canonical == anchor_canonical {
            continue;
        }
        candidates.push(path.to_path_buf());
    }

    if candidates.len() > max_files {
        return Err(crate::ParserError::ReferencesWalkCap {
            root: root.to_path_buf(),
            candidates: candidates.len(),
            max_files,
        });
    }

    let mut siblings = Vec::new();
    for path in candidates {
        let size = match std::fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(err) => {
                skipped.push(SkippedFile {
                    file: path,
                    reason: format!("cannot stat file: {err}"),
                });
                continue;
            }
        };
        if size > max_file_bytes {
            skipped.push(SkippedFile {
                file: path,
                reason: format!("exceeds size cap: {size} bytes > cap of {max_file_bytes} bytes"),
            });
            continue;
        }
        match SourceFile::load(&path) {
            Ok(source) => siblings.push(source),
            Err(err) => skipped.push(SkippedFile {
                file: path,
                reason: format!("cannot load: {err}"),
            }),
        }
    }

    Ok(ReferencesWalk { siblings, skipped })
}

/// Provenance of a `references` answer: which instrument produced it.
/// `TreeSitter` is the parser's name-level baseline and the default (every
/// in-crate producer is the tree-sitter engine); `Lsp` is set by the CLI when
/// a live language server answered. An empty LSP answer is a real answer —
/// this attribution is what makes it distinguishable from an empty baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceSource {
    Lsp,
    #[default]
    TreeSitter,
}

/// Result of any query operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QueryResult {
    // Tree-sitter queries
    Skeleton(SkeletonResult),
    Symbol(Box<SymbolResult>),
    Symbols {
        symbols: Vec<Symbol>,
    },
    References {
        references: Vec<Location>,
        /// Siblings skipped by the cross-file workspace walk (unloadable or
        /// oversize), with reasons. Single-file queries and the in-memory
        /// multi-file API produce an empty vec; the field is omitted from
        /// serialization when empty so clean-workspace output is unchanged.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skipped_files: Vec<SkippedFile>,
        /// Provenance of the answer: `tree-sitter` is the parser's name-level
        /// baseline; `lsp` is set by the CLI when a live language server
        /// answered (empty answers included — an empty LSP answer is a real
        /// answer, and this attribution is what makes it distinguishable).
        /// Always serialized; pre-change payloads without the key read back
        /// as `tree-sitter`.
        #[serde(default)]
        source: ReferenceSource,
    },
    Definition {
        location: Option<Location>,
    },

    // LSP queries
    Callers {
        callers: Vec<Symbol>,
    },
    Callees {
        callees: Vec<Symbol>,
    },
    Implementations {
        locations: Vec<Range>,
    },
    Hierarchy {
        incoming: Vec<Symbol>,
        outgoing: Vec<Symbol>,
    },
    Type {
        content: String,
        range: Option<Range>,
    },
    Diagnostics {
        diagnostics: Vec<DiagnosticInfo>,
    },
    Dependencies {
        imports: Vec<DependencyInfo>,
    },
    Impact {
        references_count: usize,
        callers_count: usize,
        files_affected: Vec<String>,
    },
    Unused {
        symbols: Vec<Symbol>,
    },
    Tests {
        tests: Vec<Symbol>,
    },
    Coverage {
        status: String,
        details: String,
    },
    Suggest {
        actions: Vec<SuggestionInfo>,
    },

    NotImplemented {
        query: QueryType,
        reason: String,
    },
}

/// Visibility for a captured declaration node, for grammars whose syntax
/// models it. Returns `None` for every other grammar — the honest "not
/// modeled" signal, distinct from private.
fn declaration_visibility(
    grammar: crate::Grammar,
    node: tree_sitter::Node,
    name: &str,
    source_bytes: &[u8],
) -> Option<String> {
    match grammar {
        crate::Grammar::Rust => {
            // tree-sitter-rust: `pub` parses as a visibility_modifier child.
            // The grammar models visibility, so absence means private. A
            // restricted modifier (`pub(crate)`, `pub(super)`, `pub(in ...)`)
            // is narrower than public — "internal".
            let mut cursor = node.walk();
            let modifier = node
                .children(&mut cursor)
                .find(|child| child.kind() == "visibility_modifier");
            Some(
                match modifier {
                    None => "private",
                    Some(m) if m.utf8_text(source_bytes).is_ok_and(|t| t.trim() == "pub") => {
                        "public"
                    }
                    Some(_) => "internal",
                }
                .to_string(),
            )
        }
        crate::Grammar::TypeScript
        | crate::Grammar::Tsx
        | crate::Grammar::JavaScript
        | crate::Grammar::Jsx => {
            // The TS/JS model is module export-ness: exported declarations are
            // wrapped in an export_statement; unexported TOP-LEVEL declarations
            // are module-private unless a separate `export { name }` or
            // `export default name` clause exports them. Nested declarations
            // (class members, local functions) aren't module exports, so the
            // model says nothing about them — None, not "private".
            // Namespace-inner `export` also reports "export" (module-export
            // overcount, accepted).
            match node.parent() {
                Some(parent) if parent.kind() == "export_statement" => Some("export".to_string()),
                Some(parent) if parent.kind() == "program" => Some(
                    if exported_by_clause(parent, name, source_bytes) {
                        "export"
                    } else {
                        "private"
                    }
                    .to_string(),
                ),
                _ => None,
            }
        }
        _ => None,
    }
}

/// True if a top-level `export { name }` / `export { name as alias }` /
/// `export default name` statement in `program` exports the LOCAL binding
/// `name`.
fn exported_by_clause(program: tree_sitter::Node, name: &str, source_bytes: &[u8]) -> bool {
    let mut cursor = program.walk();
    for stmt in program.children(&mut cursor) {
        if stmt.kind() != "export_statement" {
            continue;
        }
        let mut stmt_cursor = stmt.walk();
        for child in stmt.children(&mut stmt_cursor) {
            match child.kind() {
                // `export default <identifier>;` re-exports a local binding.
                "identifier" if child.utf8_text(source_bytes) == Ok(name) => {
                    return true;
                }
                // `export { a, b as c };` — the LOCAL name is the specifier's
                // `name` field (the identifier before `as`).
                "export_clause" => {
                    let mut clause_cursor = child.walk();
                    for spec in child.children(&mut clause_cursor) {
                        if spec.kind() == "export_specifier"
                            && spec
                                .child_by_field_name("name")
                                .is_some_and(|n| n.utf8_text(source_bytes) == Ok(name))
                        {
                            return true;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    false
}

fn check_source_format(source: &SourceFile) -> Result<()> {
    let support = source.grammar().language_support();
    if let Some((detected, guidance)) = support.preparse_guard(source.tree().source()) {
        return Err(crate::error::ParserError::UnsupportedSourceFormat {
            file: source.path().to_path_buf(),
            detected: detected.to_string(),
            guidance: guidance.to_string(),
        });
    }
    Ok(())
}

/// Executes a skeleton query on a source file.
///
/// Returns the file structure without function/method bodies.
pub fn execute_skeleton(source: &SourceFile) -> Result<QueryResult> {
    let support = source.grammar().language_support();
    let grammar = source.grammar();

    check_source_format(source)?;

    let query_source = support.skeleton_query();
    let query = tree_sitter::Query::new(&support.ts_language(), query_source)?;

    let tree = source.tree();
    let root_node = tree.root_node();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, tree.source().as_bytes());

    let mut declarations = Vec::new();

    while let Some(match_) = matches.next() {
        let mut name_captures = String::new();
        // D3: the span of the FIRST `@name` capture. Patterns that compose a
        // name from several captures (Thrift's split dotted namespace) still
        // have one identifier a language server will answer about, and it is
        // the first — the later captures are continuations of the same path.
        let mut name_range: Option<Range> = None;
        let mut self_name: Option<String> = None;
        let mut range = None;
        let mut kind = None;
        let mut decl_node = None;
        let mut dotted_definition: Option<(tree_sitter::Node, &str)> = None;
        let mut is_class_attribute = false;
        let mut name_node = None;

        for capture in match_.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            let node = capture.node;

            match *capture_name {
                "attr_name" => {
                    is_class_attribute = node.utf8_text(tree.source().as_bytes()) == Ok("class");
                }
                "name" => {
                    let text = node
                        .utf8_text(tree.source().as_bytes())
                        .unwrap_or("<invalid UTF-8>");
                    name_captures.push_str(text);
                    name_node = Some(node);
                    if name_range.is_none() {
                        name_range = Some(node_to_range(node, source.path()));
                    }
                }
                "import" | "export" | "include" | "use" | "require" => {
                    kind = Some(*capture_name);
                    range = Some(node_to_range(node, source.path()));
                    self_name = Some(
                        node.utf8_text(tree.source().as_bytes())
                            .unwrap_or("<invalid UTF-8>")
                            .to_string(),
                    );
                }
                "function" | "class" | "interface" | "trait" | "type_alias" | "enum" | "method"
                | "namespace" | "struct" | "delegate" | "field" | "constructor" | "event"
                | "property" | "variable" | "docstring" | "impl" | "module" | "macro" | "union"
                | "typedef" | "concept" | "using" | "static_initializer" | "record"
                | "annotation" | "enum_constant" | "package" | "indexer" | "constant"
                | "element" | "block" | "tag" => {
                    kind = Some(*capture_name);
                    range = Some(node_to_range(node, source.path()));
                    decl_node = Some(node);
                }
                "definition" => {
                    kind = Some(skeleton_kind_for(node_kind_to_symbol_kind(
                        support.node_kind_with_source(&node, tree.source().as_bytes()),
                    )));
                    range = Some(node_to_range(node, source.path()));
                    decl_node = Some(node);
                }
                // `@definition.<kind>` states the kind in the query. See
                // `dotted_definition`'s declaration for why this is recorded
                // as a candidate instead of written straight through.
                dotted if dotted.starts_with("definition.") => {
                    dotted_definition = Some((node, &dotted["definition.".len()..]));
                }
                _ => {}
            }
        }

        if kind.is_none()
            && let Some((node, suffix)) = dotted_definition
        {
            let symbol_kind = symbol_kind_from_suffix(suffix).unwrap_or_else(|| {
                node_kind_to_symbol_kind(
                    support.node_kind_with_source(&node, tree.source().as_bytes()),
                )
            });
            kind = Some(skeleton_kind_for(symbol_kind));
            range = Some(node_to_range(node, source.path()));
            decl_node = Some(node);
        }

        if let (Some(k), Some(r)) = (kind, range) {
            // D3: `name_range` only describes a name this declaration actually
            // TOOK from a `@name` capture. A pattern whose `@name` captured a
            // zero-width node contributes no text and falls through to the
            // self-naming/fallback arm below, so its span names nothing and is
            // dropped rather than pointing a caller at an empty position.
            let name_range = if name_captures.is_empty() {
                None
            } else {
                name_range
            };
            let mut name = if !name_captures.is_empty() {
                name_captures
            } else {
                self_name.unwrap_or_default()
            };
            name = name
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or_default()
                .to_string();
            let name = name.trim().to_string();
            let name = if grammar == crate::Grammar::Ruby {
                name.strip_prefix(':').unwrap_or(&name).to_string()
            } else {
                name
            };
            if name.is_empty() {
                continue;
            }
            // A class attribute value lists several classes; each is a declaration.
            if is_class_attribute && let Some(value) = name_node {
                for (token, token_range) in
                    whitespace_separated_tokens(value, tree.source().as_bytes(), source.path())
                {
                    declarations.push(Declaration {
                        kind: k.to_string(),
                        name: token,
                        range: r.clone(),
                        name_range: Some(token_range),
                        visibility: None,
                    });
                }
                continue;
            }
            let visibility = decl_node.and_then(|node| {
                declaration_visibility(grammar, node, &name, tree.source().as_bytes())
            });
            declarations.push(Declaration {
                kind: k.to_string(),
                name,
                range: r,
                name_range,
                visibility,
            });
        }
    }

    Ok(QueryResult::Skeleton(SkeletonResult { declarations }))
}

/// Executes a symbols query on a source file.
///
/// Returns all symbol definitions in the file.
pub fn execute_symbols(source: &SourceFile) -> Result<QueryResult> {
    let support = source.grammar().language_support();

    check_source_format(source)?;

    let query_source = support.symbols_query();
    let query = tree_sitter::Query::new(&support.ts_language(), query_source)?;

    let tree = source.tree();
    let root_node = tree.root_node();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, tree.source().as_bytes());

    let mut symbols = Vec::new();

    while let Some(match_) = matches.next() {
        let mut name = None;
        let mut range = None;
        let mut name_node = None;
        let mut is_class_attribute = false;
        let mut definition_node = None;
        let mut kinded_definition = None;

        for capture in match_.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            let node = capture.node;

            match *capture_name {
                "name" => {
                    // A CSS class or id is declared with its sigil, a sibling token.
                    let sigil = css_selector_sigil(node, source.grammar());
                    let text_start = sigil.map_or(node.start_byte(), |sigil| sigil.start_byte());
                    name = Some(
                        tree.source()
                            .get(text_start..node.end_byte())
                            .unwrap_or("<invalid UTF-8>")
                            .to_string(),
                    );
                    // Use the name node's range instead of definition node's range
                    range = Some(match sigil {
                        Some(sigil) => Range::new(
                            node_to_range(sigil, source.path()).start().clone(),
                            node_to_range(node, source.path()).end().clone(),
                        ),
                        None => node_to_range(node, source.path()),
                    });
                    name_node = Some(node);
                }
                "attr_name" => {
                    is_class_attribute = node.utf8_text(tree.source().as_bytes()) == Ok("class");
                }
                "definition" => {
                    definition_node = Some(node);
                }
                dotted if dotted.starts_with("definition.") => {
                    kinded_definition = Some((node, &dotted["definition.".len()..]));
                }
                _ => {}
            }
        }

        let resolved = match definition_node {
            Some(node) => Some((node, None)),
            None => kinded_definition.map(|(node, suffix)| (node, symbol_kind_from_suffix(suffix))),
        };

        if let Some((node, stated_kind)) = resolved {
            let symbol_kind = stated_kind.unwrap_or_else(|| {
                let node_kind = support.node_kind_with_source(&node, tree.source().as_bytes());
                node_kind_to_symbol_kind(node_kind)
            });
            // Use the @name range when available; fall back to the definition node's range.
            let symbol_range = range.unwrap_or_else(|| node_to_range(node, source.path()));
            let symbol_name = name.unwrap_or_default();
            // A class attribute value lists several classes; each is a symbol.
            if is_class_attribute && let Some(value) = name_node {
                for (token, range) in
                    whitespace_separated_tokens(value, tree.source().as_bytes(), source.path())
                {
                    symbols.push(Symbol::new(token, symbol_kind, range));
                }
                continue;
            }
            symbols.push(Symbol::new(symbol_name, symbol_kind, symbol_range));
        }
    }

    Ok(QueryResult::Symbols { symbols })
}

/// The `.` or `#` token that precedes a CSS class or id name node.
fn css_selector_sigil<'a>(
    node: tree_sitter::Node<'a>,
    grammar: Grammar,
) -> Option<tree_sitter::Node<'a>> {
    if grammar != Grammar::Css || !matches!(node.kind(), "class_name" | "id_name") {
        return None;
    }
    node.prev_sibling()
        .filter(|sigil| matches!(sigil.kind(), "." | "#"))
}

/// The whitespace-separated tokens of `node`'s text, each with its own range.
/// The separators are HTML's ASCII whitespace, so a multi-class `class` value
/// yields one token per class.
fn whitespace_separated_tokens(
    node: tree_sitter::Node,
    source: &[u8],
    file: &std::path::Path,
) -> Vec<(String, Range)> {
    let text = node.utf8_text(source).unwrap_or_default();
    let (mut row, mut column) = (
        node.start_position().row as u32 + 1,
        node.start_position().column as u32 + 1,
    );
    let mut byte = node.start_byte();
    let mut tokens = Vec::new();
    let mut open: Option<(usize, u32, u32)> = None;
    let location =
        |row, column, byte| act_lite_core::Location::full(file.to_path_buf(), row, column, byte);
    for (offset, ch) in text.char_indices() {
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0C');
        match (is_space, open) {
            (false, None) => open = Some((offset, row, column)),
            (true, Some((start, start_row, start_column))) => {
                tokens.push((
                    text[start..offset].to_string(),
                    Range::new(
                        location(start_row, start_column, node.start_byte() + start),
                        location(row, column, byte),
                    ),
                ));
                open = None;
            }
            _ => {}
        }
        byte += ch.len_utf8();
        if ch == '\n' {
            row += 1;
            column = 1;
        } else {
            column += ch.len_utf8() as u32;
        }
    }
    if let Some((start, start_row, start_column)) = open {
        tokens.push((
            text[start..].to_string(),
            Range::new(
                location(start_row, start_column, node.start_byte() + start),
                location(row, column, byte),
            ),
        ));
    }
    tokens
}

/// Executes a references query on a source file.
///
/// Returns all identifier references in the file.
pub fn execute_references(source: &SourceFile) -> Result<QueryResult> {
    execute_references_matching(source, None)
}

/// Executes a references query for the exact symbol at a 1-indexed position.
pub fn execute_references_for_symbol(
    source: &SourceFile,
    name: &str,
    line: u32,
    column: u32,
) -> Result<QueryResult> {
    let mut node = node_at_byte_position(source, line, column).ok_or_else(|| {
        crate::ParserError::InvalidPosition {
            file: source.path().to_path_buf(),
            line,
            column,
        }
    })?;

    loop {
        if node_declares_name(source.grammar(), node, source.content().as_bytes(), name) {
            return execute_references_matching(source, Some(name));
        }
        match node.parent() {
            Some(parent) => node = parent,
            None => break,
        }
    }

    Err(crate::ParserError::SymbolNotFound {
        name: name.to_string(),
        file: source.path().to_path_buf(),
    })
}

/// Whether `node` is the text `name`. A Markdown heading's text node carries
/// the space after the `#` marker, so it matches on its trimmed text.
fn node_declares_name(
    grammar: Grammar,
    node: tree_sitter::Node,
    source_bytes: &[u8],
    name: &str,
) -> bool {
    match node.utf8_text(source_bytes) {
        Ok(text) if text == name => true,
        Ok(text) => match (grammar, node.kind()) {
            (Grammar::Markdown, "text") => text.trim() == name,
            (Grammar::Dockerfile, "param") => text.strip_prefix("--from=") == Some(name),
            (Grammar::Yaml, "alias") => text.strip_prefix('*') == Some(name),
            _ => false,
        },
        Err(_) => false,
    }
}

fn node_at_byte_position(
    source: &SourceFile,
    line: u32,
    column: u32,
) -> Option<tree_sitter::Node<'_>> {
    if line == 0 || column == 0 {
        return None;
    }

    let bytes = source.content().as_bytes();
    let mut line_start = 0;
    let mut current_line = 1;

    loop {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| line_start + offset);

        if current_line == line {
            let column_offset = (column - 1) as usize;
            if column_offset > line_end - line_start {
                return None;
            }
            return Some(source.tree().node_at_byte(line_start + column_offset));
        }

        if line_end == bytes.len() {
            return None;
        }
        line_start = line_end + 1;
        current_line += 1;
    }
}

/// Executes a references query for the exact symbol at a 1-indexed position
/// across the anchor file and every sibling file.
///
/// Anchor validation matches [`execute_references_for_symbol`] exactly: the
/// position and `name` are validated against the anchor file only, returning
/// [`crate::ParserError::InvalidPosition`] or [`crate::ParserError::SymbolNotFound`].
/// Matches from the anchor and all siblings are merged, deduplicated by
/// (file path, byte offset), and sorted by (file path, line, column), so the
/// output is deterministic regardless of sibling order. Grammar filtering of
/// siblings is the caller's responsibility.
pub fn execute_references_for_symbol_across(
    anchor: &SourceFile,
    siblings: &[SourceFile],
    name: &str,
    line: u32,
    column: u32,
) -> Result<QueryResult> {
    let mut node = node_at_byte_position(anchor, line, column).ok_or_else(|| {
        crate::ParserError::InvalidPosition {
            file: anchor.path().to_path_buf(),
            line,
            column,
        }
    })?;

    loop {
        if node_declares_name(anchor.grammar(), node, anchor.content().as_bytes(), name) {
            break;
        }
        match node.parent() {
            Some(parent) => node = parent,
            None => {
                return Err(crate::ParserError::SymbolNotFound {
                    name: name.to_string(),
                    file: anchor.path().to_path_buf(),
                });
            }
        }
    }

    let mut references = collect_references_matching(anchor, Some(name))?;
    for sibling in siblings {
        references.extend(collect_references_matching(sibling, Some(name))?);
    }

    references.sort_by(|a, b| {
        a.file()
            .cmp(b.file())
            .then_with(|| a.line().cmp(&b.line()))
            .then_with(|| a.column().cmp(&b.column()))
    });
    references.dedup_by(|a, b| a.file() == b.file() && a.byte_offset() == b.byte_offset());

    Ok(QueryResult::References {
        references,
        skipped_files: Vec::new(),
        source: ReferenceSource::TreeSitter,
    })
}

fn execute_references_matching(
    source: &SourceFile,
    expected_name: Option<&str>,
) -> Result<QueryResult> {
    Ok(QueryResult::References {
        references: collect_references_matching(source, expected_name)?,
        skipped_files: Vec::new(),
        source: ReferenceSource::TreeSitter,
    })
}

fn collect_references_matching(
    source: &SourceFile,
    expected_name: Option<&str>,
) -> Result<Vec<Location>> {
    Ok(
        collect_reference_ranges_matching(source, expected_name, None)?
            .into_iter()
            .map(|range| range.start().clone())
            .collect(),
    )
}

pub(crate) enum ComposedReference {
    /// The name occurs here in modeled composed form(s); carry the inner
    /// (in-node) byte offsets of each occurrence (a shell fragment can
    /// carry several).
    Modeled(Vec<(usize, usize)>),
    /// Plain node — the exact-text comparison decides.
    Plain,
    /// The name occurs in a form this grammar does not model; refuse.
    Unmodeled(String),
    /// No name involvement.
    None,
}

fn heredoc_delimiter_quoted(node: tree_sitter::Node, source_bytes: &[u8]) -> bool {
    fn find_kind<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == kind {
            return Some(node);
        }
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find_map(|child| find_kind(child, kind))
    }
    node.parent()
        .and_then(|block| block.prev_sibling())
        .and_then(|command| find_kind(command, "heredoc_marker"))
        .and_then(|marker| marker.utf8_text(source_bytes).ok())
        // Unfindable marker: report QUOTED so the caller refuses (fail
        // closed — expansion cannot be proven).
        .is_none_or(|delim| delim.contains('\'') || delim.contains('"'))
}

/// Whether some `FROM ... AS <name>` in the file declares the stage `name`.
fn dockerfile_declares_stage(node: tree_sitter::Node, source_bytes: &[u8], name: &str) -> bool {
    let mut root = node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    let mut cursor = root.walk();
    root.children(&mut cursor).any(|instruction| {
        instruction
            .child_by_field_name("as")
            .is_some_and(|alias| alias.utf8_text(source_bytes) == Ok(name))
    })
}

fn classify_composed_reference(
    grammar: Grammar,
    node: tree_sitter::Node,
    source_bytes: &[u8],
    text: &str,
    name: &str,
) -> ComposedReference {
    let node_kind = node.kind();
    if name.is_empty() {
        return ComposedReference::Plain;
    }
    match grammar {
        Grammar::Dockerfile if node_kind == "param" => text
            .strip_prefix("--from=")
            .filter(|stage| *stage == name)
            .map_or(ComposedReference::None, |_| {
                ComposedReference::Modeled(vec![("--from=".len(), text.len())])
            }),
        Grammar::Dockerfile if node_kind == "image_name" => {
            let tagged = node
                .parent()
                .is_some_and(|spec| spec.child_by_field_name("tag").is_some());
            if !tagged && text == name && dockerfile_declares_stage(node, source_bytes, name) {
                ComposedReference::Plain
            } else {
                ComposedReference::None
            }
        }
        Grammar::Yaml if node_kind == "alias" => text
            .strip_prefix('*')
            .filter(|anchor| *anchor == name)
            .map_or(ComposedReference::None, |_| {
                ComposedReference::Modeled(vec![(1, text.len())])
            }),
        Grammar::Markdown if node_kind == "heading_content" => {
            let lead = text.len() - text.trim_start().len();
            if text.trim() == name {
                ComposedReference::Modeled(vec![(lead, lead + name.len())])
            } else {
                ComposedReference::None
            }
        }
        Grammar::Dockerfile if node_kind == "shell_fragment" || node_kind == "heredoc_line" => {
            if !text.contains(name) {
                return ComposedReference::None;
            }
            let is_heredoc_line = node_kind == "heredoc_line";
            if is_heredoc_line && heredoc_delimiter_quoted(node, source_bytes) {
                return ComposedReference::Unmodeled(
                    "dockerfile quoted-delimiter heredoc (expansion suppressed — literal text)"
                        .to_string(),
                );
            }
            let single_quoted: Vec<(usize, usize)> = if is_heredoc_line {
                Vec::new()
            } else {
                let mut spans = Vec::new();
                let mut in_single = false;
                let mut in_double = false;
                let mut escaped = false;
                let mut open: Option<usize> = None;
                for (i, &b) in text.as_bytes().iter().enumerate() {
                    if escaped {
                        escaped = false;
                        continue;
                    }
                    // Backslash escapes the next byte except inside single
                    // quotes (where it is literal).
                    if !in_single && b == b'\\' {
                        escaped = true;
                        continue;
                    }
                    match b {
                        b'\'' if !in_double => {
                            in_single = !in_single;
                            if in_single {
                                open = Some(i);
                            } else if let Some(start) = open.take() {
                                spans.push((start, i + 1));
                            }
                        }
                        b'"' if !in_single => in_double = !in_double,
                        _ => {}
                    }
                }
                // An unterminated single quote makes the rest literal.
                if let Some(start) = open {
                    spans.push((start, text.len()));
                }
                spans
            };
            let is_var_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
            let mut modeled = Vec::new();
            let mut unmodeled: Option<String> = None;
            let bytes = text.as_bytes();
            let mut off = 0;
            while let Some(rel) = text[off..].find(name) {
                let at = off + rel;
                let after = text[at + name.len()..].chars().next();
                let in_single_quotes = single_quoted.iter().any(|(s, e)| at >= *s && at < *e);
                let preceded_braces = at >= 2 && text[..at].ends_with("${");
                let preceded_dollar = at >= 1 && bytes[at - 1] == b'$';
                let preceded_escape = at >= 2 && text[..at].ends_with("$$");
                // `\$NAME`: the backslash escapes the DOLLAR (the byte before
                // the name), making the occurrence literal shell text.
                let dollar_backslash_escaped = preceded_dollar && at >= 2 && bytes[at - 2] == b'\\';
                if in_single_quotes {
                    unmodeled = Some(
                        "dockerfile single-quoted text (literal — not a variable use)".to_string(),
                    );
                } else if dollar_backslash_escaped {
                    unmodeled = Some(
                        "dockerfile backslash-escaped $ (literal — not a variable use)".to_string(),
                    );
                } else if preceded_escape {
                    unmodeled =
                        Some("dockerfile $$-escaped variable (compose semantics)".to_string());
                } else if (preceded_braces && after == Some('}'))
                    || (preceded_dollar && !after.is_some_and(is_var_char))
                {
                    modeled.push((at, at + name.len()));
                } else {
                    // A bare word in shell text is not a variable USE, but it
                    // is an occurrence the resolver cannot classify — the
                    // allowlist refuses rather than skip.
                    unmodeled = Some("dockerfile shell text (bare name)".to_string());
                }
                off = at + name.len();
            }
            if let Some(form) = unmodeled {
                return ComposedReference::Unmodeled(form);
            }
            if modeled.is_empty() {
                return ComposedReference::None;
            }
            ComposedReference::Modeled(modeled)
        }
        _ => ComposedReference::Plain,
    }
}

fn collect_reference_ranges_matching(
    source: &SourceFile,
    expected_name: Option<&str>,
    exclude_byte_range: Option<(usize, usize)>,
) -> Result<Vec<Range>> {
    check_source_format(source)?;

    let support = source.grammar().language_support();

    let query_source = support.references_query();
    let query = tree_sitter::Query::new(&support.ts_language(), query_source)?;

    let tree = source.tree();
    let root_node = tree.root_node();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, tree.source().as_bytes());

    let mut references = Vec::new();

    while let Some(match_) = matches.next() {
        for capture in match_.captures {
            let capture_name = &query.capture_names()[capture.index as usize];
            if *capture_name == "reference" {
                let node = capture.node;
                let text = node.utf8_text(source.content().as_bytes()).unwrap_or("");
                let name = expected_name.unwrap_or("");
                let classified = classify_composed_reference(
                    source.grammar(),
                    node,
                    source.content().as_bytes(),
                    text,
                    name,
                );
                // The definition's own node is not a reference — but only
                // composed carriers skip: a PLAIN identifier at the anchor IS
                // the long-standing contract (the anchor file's def span is
                // returned to callers; reference_resolution pins it). A
                // composed carrier at the anchor (gn's `target("name")`
                // string) would otherwise duplicate or self-refuse the def.
                if let Some((lo, hi)) = exclude_byte_range
                    && node.start_byte() <= lo
                    && node.end_byte() >= hi
                    && matches!(
                        classified,
                        ComposedReference::Modeled(_) | ComposedReference::Unmodeled(_)
                    )
                {
                    continue;
                }
                match classified {
                    ComposedReference::Modeled(occurrences) => {
                        for (start_off, end_off) in occurrences {
                            let base = node.start_byte();
                            let start_loc = Location::full(
                                source.path().to_path_buf(),
                                node.start_position().row as u32 + 1,
                                node.start_position().column as u32 + 1 + (start_off as u32),
                                base + start_off,
                            );
                            let end_loc = Location::full(
                                source.path().to_path_buf(),
                                node.start_position().row as u32 + 1,
                                node.start_position().column as u32 + 1 + (end_off as u32),
                                base + end_off,
                            );
                            references.push(Range::new(start_loc, end_loc));
                        }
                    }
                    // Composed refusals fire only when a name is in play.
                    ComposedReference::Unmodeled(form) => {
                        return Err(crate::ParserError::UnmodeledReferenceForm {
                            file: source.path().to_path_buf(),
                            line: node.start_position().row as u32 + 1,
                            form,
                        });
                    }
                    ComposedReference::None => {}
                    ComposedReference::Plain => {
                        if expected_name.is_some_and(|n| text != n) {
                            continue;
                        }
                        let start_loc = Location::full(
                            source.path().to_path_buf(),
                            node.start_position().row as u32 + 1,
                            node.start_position().column as u32 + 1,
                            node.start_byte(),
                        );
                        let end_loc = Location::full(
                            source.path().to_path_buf(),
                            node.end_position().row as u32 + 1,
                            node.end_position().column as u32 + 1,
                            node.end_byte(),
                        );
                        references.push(Range::new(start_loc, end_loc));
                    }
                }
            }
        }
    }

    Ok(references)
}

/// Executes a references query for the exact symbol at a 1-indexed position
/// across the anchor file and every sibling, returning full spans.
///
/// The range-level twin of [`execute_references_for_symbol_across`]: same
/// anchor validation, same matching, same dedup-and-sort, but every site is a
/// [`Range`] covering the whole captured node — the shape an *editing* caller
/// (rename) needs, where a start-only location is not enough to rewrite.
pub fn execute_reference_ranges_for_symbol_across(
    anchor: &SourceFile,
    siblings: &[SourceFile],
    name: &str,
    line: u32,
    column: u32,
) -> Result<Vec<Range>> {
    let mut node = node_at_byte_position(anchor, line, column).ok_or_else(|| {
        crate::ParserError::InvalidPosition {
            file: anchor.path().to_path_buf(),
            line,
            column,
        }
    })?;

    loop {
        if node.utf8_text(anchor.content().as_bytes()) == Ok(name) {
            break;
        }
        match node.parent() {
            Some(parent) => node = parent,
            None => {
                return Err(crate::ParserError::SymbolNotFound {
                    name: name.to_string(),
                    file: anchor.path().to_path_buf(),
                });
            }
        }
    }

    // The anchor node (walking up from the position until the text equals
    // the name) IS the definition — exclude its byte span so a composed-form
    // classifier never treats the definition's own string as an occurrence.
    let anchor_span = (node.start_byte(), node.end_byte());
    let mut references = collect_reference_ranges_matching(anchor, Some(name), Some(anchor_span))?;
    for sibling in siblings {
        references.extend(collect_reference_ranges_matching(
            sibling,
            Some(name),
            None,
        )?);
    }

    references.sort_by(|a, b| {
        a.start()
            .file()
            .cmp(b.start().file())
            .then_with(|| a.start().line().cmp(&b.start().line()))
            .then_with(|| a.start().column().cmp(&b.start().column()))
    });
    references.dedup_by(|a, b| {
        a.start().file() == b.start().file() && a.start().byte_offset() == b.start().byte_offset()
    });

    Ok(references)
}

/// Executes a symbol query returning the first symbol found.
///
/// Reuses `execute_symbols()` and returns the first result wrapped in `QueryResult::Symbol`.
fn execute_symbol_first(source: &SourceFile) -> Result<QueryResult> {
    let symbols_result = execute_symbols(source)?;
    match symbols_result {
        QueryResult::Symbols { symbols } => {
            if let Some(first) = symbols.into_iter().next() {
                let source_text = extract_symbol_source(source, &first);
                let (context_before, context_after) = extract_context(source, &first, 3);
                Ok(QueryResult::Symbol(Box::new(SymbolResult {
                    symbol: first,
                    source: source_text,
                    context_before,
                    context_after,
                })))
            } else {
                // No symbols found — return an empty symbol result
                let range = Range::from_byte_offsets(source.path().to_path_buf(), 0, 0);
                Ok(QueryResult::Symbol(Box::new(SymbolResult {
                    symbol: Symbol::new(String::new(), SymbolKind::Unknown, range),
                    source: String::new(),
                    context_before: String::new(),
                    context_after: String::new(),
                })))
            }
        }
        other => Ok(other),
    }
}

/// Searches for a symbol by name and returns it with source text and context.
///
/// If the symbol is not found, returns a `QueryResult::Symbol` with an empty source string
/// and the requested name.
pub fn execute_symbol(source: &SourceFile, name: &str) -> Result<QueryResult> {
    let symbols_result = execute_symbols(source)?;
    match symbols_result {
        QueryResult::Symbols { symbols } => {
            // JSON and JSONC keep the string quotes and Markdown the heading's leading space in
            // a symbol's name; the bare name finds the same declaration.
            let bare = |s: &Symbol| match source.grammar() {
                Grammar::Json | Grammar::Jsonc => s.name().trim_matches('"') == name,
                Grammar::Markdown => s.name().trim() == name,
                _ => false,
            };
            if let Some(found) = symbols.into_iter().find(|s| s.name() == name || bare(s)) {
                let source_text = extract_symbol_source(source, &found);
                let (context_before, context_after) = extract_context(source, &found, 3);
                Ok(QueryResult::Symbol(Box::new(SymbolResult {
                    symbol: found,
                    source: source_text,
                    context_before,
                    context_after,
                })))
            } else {
                Err(crate::ParserError::SymbolNotFound {
                    name: name.to_string(),
                    file: source.path().to_path_buf(),
                })
            }
        }
        other => Ok(other),
    }
}

/// Executes a definition query returning the location of the first symbol.
fn execute_definition_first(source: &SourceFile) -> Result<QueryResult> {
    let symbols_result = execute_symbols(source)?;
    match symbols_result {
        QueryResult::Symbols { symbols } => {
            let location = symbols
                .into_iter()
                .next()
                .map(|s| s.range().start().clone());
            Ok(QueryResult::Definition { location })
        }
        _ => Ok(QueryResult::Definition { location: None }),
    }
}

/// One capture from a raw tree-sitter query match.
#[derive(Debug, Clone)]
pub struct QueryCapture {
    /// The capture name without the leading `@`.
    pub name: String,
    /// The captured node's source text.
    pub text: String,
    /// The captured node's range (1-indexed, byte offsets populated).
    pub range: Range,
}

/// One match from a raw tree-sitter query: all captures in that match.
#[derive(Debug, Clone)]
pub struct RawMatch {
    /// All captures in this match, in tree-sitter emission order.
    pub captures: Vec<QueryCapture>,
}

/// Compile and run an arbitrary tree-sitter query against a parsed source file.
///
/// Unlike the fixed `execute_skeleton`/`execute_symbols` queries, this accepts
/// any query string for the file's grammar and returns every match with its
/// captures (name, text, 1-indexed range). Used by the recipe match stage.
/// Returns `Err` if the query does not compile for the grammar.
pub fn execute_raw_query(source: &SourceFile, query_source: &str) -> Result<Vec<RawMatch>> {
    let support = source.grammar().language_support();
    check_source_format(source)?;
    let query = tree_sitter::Query::new(&support.ts_language(), query_source)?;

    let tree = source.tree();
    let root_node = tree.root_node();
    let bytes = tree.source().as_bytes();
    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(&query, root_node, bytes);

    let mut out = Vec::new();
    while let Some(m) = matches.next() {
        let mut captures = Vec::new();
        for capture in m.captures {
            let name = query.capture_names()[capture.index as usize].to_string();
            let node = capture.node;
            let text = node
                .utf8_text(bytes)
                .unwrap_or("<invalid UTF-8>")
                .to_string();
            captures.push(QueryCapture {
                name,
                text,
                range: node_to_range(node, source.path()),
            });
        }
        out.push(RawMatch { captures });
    }
    Ok(out)
}

/// Executes any query type, returning NotImplemented for LSP-dependent ones.
pub fn execute_query(query_type: QueryType, source: &SourceFile) -> Result<QueryResult> {
    match query_type {
        QueryType::Skeleton => execute_skeleton(source),
        QueryType::Symbols => execute_symbols(source),
        QueryType::References => execute_references(source),
        QueryType::Symbol => execute_symbol_first(source),
        QueryType::Definition => execute_definition_first(source),

        // LSP-dependent queries — dispatched from CLI layer, not parser
        QueryType::Callers
        | QueryType::Callees
        | QueryType::Implementations
        | QueryType::Hierarchy
        | QueryType::Type
        | QueryType::Diagnostics
        | QueryType::Dependencies
        | QueryType::Impact
        | QueryType::Unused
        | QueryType::Tests
        | QueryType::Coverage
        | QueryType::Suggest => Ok(QueryResult::NotImplemented {
            query: query_type,
            reason: format!(
                "{:?} requires LSP integration but LSP is disabled (remove --disable-lsp or unset ACT_DISABLE_LSP)",
                query_type
            ),
        }),
    }
}

/// Helper to convert tree-sitter node to act-lite-core Range.
pub(super) fn node_to_range(node: tree_sitter::Node, file: &std::path::Path) -> Range {
    let start = act_lite_core::Location::full(
        file.to_path_buf(),
        node.start_position().row as u32 + 1,
        node.start_position().column as u32 + 1,
        node.start_byte(),
    );
    let end = act_lite_core::Location::full(
        file.to_path_buf(),
        node.end_position().row as u32 + 1,
        node.end_position().column as u32 + 1,
        node.end_byte(),
    );
    Range::new(start, end)
}

fn symbol_kind_from_suffix(suffix: &str) -> Option<SymbolKind> {
    Some(match suffix {
        "function" | "procedure" | "macro" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "class" | "contract" | "impl" | "object" => SymbolKind::Class,
        "struct" | "record" => SymbolKind::Struct,
        "interface" => SymbolKind::Interface,
        "trait" => SymbolKind::Trait,
        // A tagged union is the closest thing the model has to an enum.
        "enum" | "union" => SymbolKind::Enum,
        "type" | "type_alias" | "typedef" => SymbolKind::TypeAlias,
        "module" | "namespace" | "package" | "program" | "unit" => SymbolKind::Module,
        "variable" | "loop_variable" | "global" | "storage" => SymbolKind::Variable,
        "constant" => SymbolKind::Constant,
        "enum_member" => SymbolKind::EnumMember,
        "field" | "property" | "port" => SymbolKind::Field,
        "parameter" => SymbolKind::Parameter,
        "constructor" => SymbolKind::Constructor,
        "import" | "include" | "open" => SymbolKind::Import,
        "attribute" | "annotation" | "pragma" => SymbolKind::Attribute,
        _ => return None,
    })
}

fn skeleton_kind_for(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Method => "method",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Interface => "interface",
        SymbolKind::Trait => "trait",
        SymbolKind::Enum => "enum",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Module => "module",
        SymbolKind::Field => "field",
        // `execute_skeleton` has no separate getter/setter kind; both are
        // accessors for a property, which is the vocabulary's nearest term.
        SymbolKind::Getter | SymbolKind::Setter => "property",
        SymbolKind::Constructor => "constructor",
        SymbolKind::Event => "event",
        SymbolKind::Attribute => "annotation",
        SymbolKind::Variable => "variable",
        SymbolKind::Constant => "constant",
        SymbolKind::EnumMember => "enum_constant",
        SymbolKind::Import => "import",
        SymbolKind::Export => "export",
        // A parameter is a named slot on a declaration — `field` is the
        // vocabulary's term for that, and there is no `parameter` capture.
        SymbolKind::Parameter => "field",
        // A type parameter names a type, and `type_alias` is the only
        // type-naming kind in the vocabulary.
        SymbolKind::TypeParameter => "type_alias",
        // `block` is the vocabulary's catch-all container kind, and is what
        // `NodeKind::Block` already maps to through `SymbolKind::Unknown`.
        SymbolKind::Label | SymbolKind::Unknown => "block",
    }
}

fn node_kind_to_symbol_kind(node_kind: NodeKind) -> SymbolKind {
    match node_kind {
        NodeKind::Function => SymbolKind::Function,
        NodeKind::Class => SymbolKind::Class,
        NodeKind::Object => SymbolKind::Class,
        NodeKind::Struct => SymbolKind::Struct,
        NodeKind::Interface => SymbolKind::Interface,
        NodeKind::Trait => SymbolKind::Trait,
        NodeKind::TypeAlias => SymbolKind::TypeAlias,
        NodeKind::Enum => SymbolKind::Enum,
        NodeKind::Variable => SymbolKind::Variable,
        NodeKind::Constant => SymbolKind::Constant,
        NodeKind::Parameter => SymbolKind::Parameter,
        NodeKind::Property => SymbolKind::Field,
        NodeKind::Method => SymbolKind::Method,
        NodeKind::Constructor => SymbolKind::Constructor,
        NodeKind::Event => SymbolKind::Event,
        NodeKind::Module => SymbolKind::Module,
        NodeKind::TypeParameter => SymbolKind::TypeParameter,
        NodeKind::Implementation => SymbolKind::Class,
        NodeKind::Decorator => SymbolKind::Attribute,
        NodeKind::Import => SymbolKind::Import,
        NodeKind::Export => SymbolKind::Export,
        NodeKind::Block => SymbolKind::Unknown,
        NodeKind::Element => SymbolKind::Unknown,
        NodeKind::Call => SymbolKind::Unknown,
        NodeKind::Identifier => SymbolKind::Unknown,
        NodeKind::Comment => SymbolKind::Unknown,
        NodeKind::Type => SymbolKind::Unknown,
        NodeKind::Expression => SymbolKind::Unknown,
        NodeKind::Unknown => SymbolKind::Unknown,
    }
}

/// Extracts the source text of a symbol from the file content.
fn extract_symbol_source(source: &SourceFile, symbol: &Symbol) -> String {
    let start_byte = symbol.range().start().byte_offset();
    let end_byte = symbol.range().end().byte_offset();
    match (start_byte, end_byte) {
        (Some(start), Some(end)) => {
            let content = source.content();
            if end <= content.len() {
                content[start..end].to_string()
            } else {
                String::new()
            }
        }
        _ => String::new(),
    }
}

/// Extracts context lines before and after a symbol.
///
/// Returns (context_before, context_after) with up to `n` lines each.
fn extract_context(source: &SourceFile, symbol: &Symbol, n: usize) -> (String, String) {
    let content = source.content();
    let lines: Vec<&str> = content.lines().collect();

    let start_line = symbol.range().start().line() as usize;
    let end_line = symbol.range().end().line() as usize;

    // Lines are 1-indexed, convert to 0-indexed
    let start_idx = if start_line > 0 { start_line - 1 } else { 0 };
    let end_idx = if end_line > 0 { end_line - 1 } else { 0 };

    let before_start = start_idx.saturating_sub(n);
    let context_before = lines[before_start..start_idx].join("\n");

    let after_end = (end_idx + 1 + n).min(lines.len());
    let context_after = if end_idx + 1 < lines.len() {
        lines[(end_idx + 1)..after_end].join("\n")
    } else {
        String::new()
    };

    (context_before, context_after)
}

#[cfg(test)]
mod executor_tests {
    use super::*;
    use crate::Grammar;

    #[test]
    fn skeleton_visibility_rust_pub_vs_private() {
        let source = SourceFile::from_content(
            "lib.rs",
            "pub fn exported() {}\nfn hidden() {}\npub struct Out;\nstruct In;\n".to_string(),
            Grammar::Rust,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let vis = |name: &str| {
            skel.declarations
                .iter()
                .find(|d| d.name == name)
                .unwrap_or_else(|| panic!("missing decl {name}"))
                .visibility
                .clone()
        };
        // Rust models visibility: absence of `pub` IS private, not unknown.
        assert_eq!(vis("exported").as_deref(), Some("public"));
        assert_eq!(vis("hidden").as_deref(), Some("private"));
        assert_eq!(vis("Out").as_deref(), Some("public"));
        assert_eq!(vis("In").as_deref(), Some("private"));
    }

    #[test]
    fn skeleton_visibility_typescript_export_vs_local() {
        let source = SourceFile::from_content(
            "types.ts",
            "export type UserId = string;\ntype Internal = number;\nexport function api() {}\nfunction helper() {}\n"
                .to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let vis = |name: &str| {
            skel.declarations
                .iter()
                .find(|d| d.name == name)
                .unwrap_or_else(|| panic!("missing decl {name}"))
                .visibility
                .clone()
        };
        assert_eq!(vis("UserId").as_deref(), Some("export"));
        assert_eq!(vis("Internal").as_deref(), Some("private"));
        assert_eq!(vis("api").as_deref(), Some("export"));
        assert_eq!(vis("helper").as_deref(), Some("private"));
    }

    #[test]
    fn skeleton_visibility_ts_export_clause_marks_exported() {
        let source = SourceFile::from_content(
            "mod.ts",
            "function local1() {}\nfunction local2() {}\nexport { local1, local2 as renamed };\nfunction stays() {}\n"
                .to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let vis = |name: &str| {
            skel.declarations
                .iter()
                .find(|d| d.name == name)
                .unwrap_or_else(|| panic!("missing decl {name}"))
                .visibility
                .clone()
        };
        assert_eq!(vis("local1").as_deref(), Some("export"));
        assert_eq!(vis("local2").as_deref(), Some("export"));
        assert_eq!(vis("stays").as_deref(), Some("private"));
    }

    #[test]
    fn skeleton_visibility_ts_export_default_identifier_marks_exported() {
        let source = SourceFile::from_content(
            "component.tsx",
            "const App = () => null;\nexport default App;\n".to_string(),
            Grammar::Tsx,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let app = skel
            .declarations
            .iter()
            .find(|d| d.name == "App")
            .expect("missing decl App");
        assert_eq!(app.visibility.as_deref(), Some("export"));
    }

    #[test]
    fn skeleton_visibility_jsx_export_vs_local() {
        let source = SourceFile::from_content(
            "app.jsx",
            "export function App() {}\nfunction helper() {}\n".to_string(),
            Grammar::Jsx,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let vis = |name: &str| {
            skel.declarations
                .iter()
                .find(|d| d.name == name)
                .unwrap_or_else(|| panic!("missing decl {name}"))
                .visibility
                .clone()
        };
        assert_eq!(vis("App").as_deref(), Some("export"));
        assert_eq!(vis("helper").as_deref(), Some("private"));
    }

    #[test]
    fn skeleton_visibility_rust_restricted_pub_is_internal() {
        let source = SourceFile::from_content(
            "lib.rs",
            "pub fn open() {}\npub(crate) fn crate_only() {}\npub(super) fn super_only() {}\nfn hidden() {}\n"
                .to_string(),
            Grammar::Rust,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let vis = |name: &str| {
            skel.declarations
                .iter()
                .find(|d| d.name == name)
                .unwrap_or_else(|| panic!("missing decl {name}"))
                .visibility
                .clone()
        };
        assert_eq!(vis("open").as_deref(), Some("public"));
        assert_eq!(vis("crate_only").as_deref(), Some("internal"));
        assert_eq!(vis("super_only").as_deref(), Some("internal"));
        assert_eq!(vis("hidden").as_deref(), Some("private"));
    }

    #[test]
    fn skeleton_visibility_ts_class_members_not_modeled() {
        // The TS/JS model is module export-ness. Class members aren't module
        // exports — calling a public method "private" would be false, so
        // nested declarations stay None (not modeled at that nesting level).
        let source = SourceFile::from_content(
            "svc.ts",
            "export class Svc {\n  run() {}\n}\n".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        let method = skel
            .declarations
            .iter()
            .find(|d| d.kind == "method" && d.name == "run")
            .expect("missing method decl");
        assert_eq!(method.visibility, None);
        let class = skel
            .declarations
            .iter()
            .find(|d| d.kind == "class" && d.name == "Svc")
            .expect("missing class decl");
        assert_eq!(class.visibility.as_deref(), Some("export"));
    }

    #[test]
    fn skeleton_visibility_none_for_unmodeled_grammar() {
        // Python has no visibility modifiers the skeleton models — None means
        // "grammar doesn't model visibility", the documented contract.
        let source = SourceFile::from_content(
            "app.py",
            "def f():\n    pass\n".to_string(),
            Grammar::Python,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        let QueryResult::Skeleton(skel) = result else {
            panic!("expected skeleton")
        };
        assert!(skel.declarations.iter().all(|d| d.visibility.is_none()));
    }

    #[test]
    fn skeleton_query_typescript() {
        let source = SourceFile::from_content(
            "test.ts",
            r#"
function foo() {}
class Bar {}
interface Baz {}
"#
            .to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert_eq!(skel.declarations.len(), 3);
                assert_eq!(skel.declarations[0].kind, "function");
                assert_eq!(skel.declarations[0].name, "foo");
                assert_eq!(skel.declarations[1].kind, "class");
                assert_eq!(skel.declarations[1].name, "Bar");
                assert_eq!(skel.declarations[2].kind, "interface");
                assert_eq!(skel.declarations[2].name, "Baz");
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_type_alias() {
        let source = SourceFile::from_content(
            "test.ts",
            "type MyType = string | number;".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "type_alias"));
                assert!(skel.declarations.iter().any(|d| d.name == "MyType"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_enum() {
        let source = SourceFile::from_content(
            "test.ts",
            "enum Color { Red, Green, Blue }".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "enum"));
                assert!(skel.declarations.iter().any(|d| d.name == "Color"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_import() {
        let source = SourceFile::from_content(
            "test.ts",
            "import { foo } from 'bar';".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "import"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_export() {
        let source = SourceFile::from_content(
            "test.ts",
            "export const x = 1;".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "export"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_method() {
        let source = SourceFile::from_content(
            "test.ts",
            "class Foo { bar() {} }".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "class"));
                // Methods may or may not be in skeleton depending on query
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_empty_file() {
        let source =
            SourceFile::from_content("test.ts", "".to_string(), Grammar::TypeScript).unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert_eq!(skel.declarations.len(), 0);
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_csharp_namespace() {
        let source = SourceFile::from_content(
            "test.cs",
            "namespace MyNamespace { class MyClass {} }".to_string(),
            Grammar::CSharp,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "namespace"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_csharp_struct() {
        let source = SourceFile::from_content(
            "test.cs",
            "public struct MyStruct { int x; }".to_string(),
            Grammar::CSharp,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "struct"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_python_function() {
        let source = SourceFile::from_content(
            "test.py",
            "def my_function():\n    pass".to_string(),
            Grammar::Python,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "function"));
                assert!(skel.declarations.iter().any(|d| d.name == "my_function"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    /// D3: a declaration's `range` starts at its first token — for Python that
    /// is the `def` keyword, where pyright's `prepareCallHierarchy` answers
    /// `null`. `name_range` carries the NAME node's span so a consumer can
    /// query where the server does answer. `range` itself is untouched: it is
    /// the graph node's identity and the pass-four callee cache key.
    #[test]
    fn skeleton_name_range_spans_the_name_not_the_keyword() {
        let source = SourceFile::from_content(
            "test.py",
            "def one():\n    return 1\n".to_string(),
            Grammar::Python,
        )
        .unwrap();

        let QueryResult::Skeleton(skel) = execute_skeleton(&source).unwrap() else {
            panic!("Expected Skeleton result")
        };
        let decl = skel
            .declarations
            .iter()
            .find(|d| d.name == "one")
            .expect("python function declaration");

        // The declaration range is unchanged: it still starts at `def`.
        assert_eq!(
            (decl.range.start().line(), decl.range.start().column()),
            (1, 1),
            "declaration range must still start at the `def` keyword"
        );
        let name_range = decl
            .name_range
            .as_ref()
            .expect("a python function has a distinct name node");
        // `def one():` — `one` occupies 1-indexed columns 5..8.
        assert_eq!(
            (name_range.start().line(), name_range.start().column()),
            (1, 5),
            "name_range must start at the identifier, not the keyword"
        );
        assert_eq!(
            (name_range.end().line(), name_range.end().column()),
            (1, 8),
            "name_range must end at the identifier's end"
        );
    }

    /// D3: the self-naming captures (`@import`/`@export`/`@include`/`@use`/
    /// `@require`) take their name from the declaration node's own text, so
    /// there is no distinct name node to point at. `name_range` is `None`
    /// rather than a duplicate of `range`, and pass four falls back to the
    /// declaration start — the declaration is never dropped.
    #[test]
    fn skeleton_name_range_absent_when_no_distinct_name_node() {
        let source = SourceFile::from_content(
            "test.ts",
            "import { foo } from 'bar';".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let QueryResult::Skeleton(skel) = execute_skeleton(&source).unwrap() else {
            panic!("Expected Skeleton result")
        };
        let decl = skel
            .declarations
            .iter()
            .find(|d| d.kind == "import")
            .expect("import declaration");
        assert!(
            decl.name_range.is_none(),
            "a self-naming capture has no distinct name node, got {:?}",
            decl.name_range
        );
    }

    #[test]
    fn skeleton_query_rust_impl() {
        let source = SourceFile::from_content(
            "test.rs",
            "impl MyStruct { fn new() -> Self { Self } }".to_string(),
            Grammar::Rust,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "impl"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_rust_module() {
        let source =
            SourceFile::from_content("test.rs", "mod my_module;".to_string(), Grammar::Rust)
                .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "module"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_go_function() {
        let source =
            SourceFile::from_content("test.go", "func myFunction() {}".to_string(), Grammar::Go)
                .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "function"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_c_function() {
        let source = SourceFile::from_content(
            "test.c",
            "int myFunction() { return 0; }".to_string(),
            Grammar::C,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "function"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_c_union() {
        let source = SourceFile::from_content(
            "test.c",
            "union MyUnion { int x; float y; };".to_string(),
            Grammar::C,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "union"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_c_include_produces_include_kind() {
        let source = SourceFile::from_content(
            "test.c",
            "#include <stdio.h>\n#include \"myfile.h\"\nint main() { return 0; }\n".to_string(),
            Grammar::C,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                let include_decls: Vec<_> = skel
                    .declarations
                    .iter()
                    .filter(|d| d.kind == "include")
                    .collect();
                assert_eq!(include_decls.len(), 2, "expected 2 include declarations");
                assert!(
                    include_decls.iter().any(|d| d.name.contains("stdio.h")),
                    "stdio.h include should be present"
                );
                assert!(
                    include_decls.iter().any(|d| d.name.contains("myfile.h")),
                    "myfile.h include should be present"
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_cpp_class() {
        let source = SourceFile::from_content(
            "test.cpp",
            "class MyClass { int x; };".to_string(),
            Grammar::Cpp,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "class"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_ruby_class() {
        let source = SourceFile::from_content(
            "test.rb",
            "class MyClass\n  def my_method\n  end\nend".to_string(),
            Grammar::Ruby,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "class"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_java_class() {
        let source = SourceFile::from_content(
            "Test.java",
            "public class Test { void method() {} }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "class"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_java_static_initializer_recognized() {
        // `@static_init` was a non-standard capture name that fell through to `_ => {}`
        // in the query executor, causing static initializers to be silently dropped from
        // skeleton output. After renaming to `@static_initializer` (and adding it to the
        // recognised set), the static block should appear as a skeleton declaration.
        let source = SourceFile::from_content(
            "Foo.java",
            "class Foo { static { System.out.println(\"init\"); } }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations
                        .iter()
                        .any(|d| d.kind == "static_initializer"),
                    "static_initializer should appear in Java skeleton; got: {:?}",
                    skel.declarations
                        .iter()
                        .map(|d| &d.kind)
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_java_module_declaration() {
        // module-info.java files declare a Java 9+ module. Without a `module_declaration`
        // pattern in skeleton.scm, these files produce an empty skeleton.
        let source = SourceFile::from_content(
            "module-info.java",
            "module com.example.app { requires java.base; }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations.iter().any(|d| d.kind == "module"),
                    "module declaration should appear in Java skeleton; got: {:?}",
                    skel.declarations
                        .iter()
                        .map(|d| &d.kind)
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn symbols_java_switch_expression_parses() {
        // Switch expressions with arrow labels (Java 14+) should parse without error
        // and the containing method should appear in the symbols output.
        let source = SourceFile::from_content(
            "Calc.java",
            r#"class Calc {
    int compute(int x) {
        return switch (x) {
            case 1 -> 10;
            case 2 -> 20;
            default -> 0;
        };
    }
}"#
            .to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(
                    symbols.iter().any(|s| s.name() == "compute"),
                    "method 'compute' should appear in symbols; got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_html_class_attribute_lists_one_symbol_per_class() {
        let source = SourceFile::from_content(
            "page.html",
            "<p class=\"a  b\tc\">x</p>\n<i class=d>y</i>\n".to_string(),
            Grammar::Html,
        )
        .unwrap();

        let QueryResult::Symbols { symbols } = execute_symbols(&source).unwrap() else {
            panic!("Expected Symbols result");
        };
        let found: Vec<(&str, u32, u32)> = symbols
            .iter()
            .filter(|s| ["a", "b", "c", "d", "a  b\tc"].contains(&s.name()))
            .map(|s| {
                (
                    s.name(),
                    s.range().start().line(),
                    s.range().start().column(),
                )
            })
            .collect();
        // Columns are 1-based: `<p class="` is 10 bytes, so `a` is at 11.
        assert_eq!(
            found,
            vec![("a", 1, 11), ("b", 1, 14), ("c", 1, 16), ("d", 2, 10)]
        );
    }

    #[test]
    fn symbol_finds_json_key_and_markdown_heading_by_bare_name() {
        for (grammar, path, content, name, line) in [
            (Grammar::Json, "a.json", "{\n  \"name\": 1\n}\n", "name", 2),
            (
                Grammar::Markdown,
                "a.md",
                "# Title\n\n## Install\n",
                "Install",
                3,
            ),
        ] {
            let source = SourceFile::from_content(path, content.to_string(), grammar).unwrap();
            let QueryResult::Symbol(found) = execute_symbol(&source, name).unwrap() else {
                panic!("Expected Symbol result");
            };
            assert_eq!(found.symbol.range().start().line(), line, "{path}");
            assert!(!found.source.is_empty(), "{path}");
        }
    }

    /// Declaration names from the skeleton and the symbols query, in order.
    fn declared_names(grammar: Grammar, path: &str, content: &str) -> (Vec<String>, Vec<String>) {
        let source = SourceFile::from_content(path, content.to_string(), grammar).unwrap();
        let QueryResult::Skeleton(skeleton) = execute_skeleton(&source).unwrap() else {
            panic!("Expected Skeleton result");
        };
        let QueryResult::Symbols { symbols } = execute_symbols(&source).unwrap() else {
            panic!("Expected Symbols result");
        };
        (
            skeleton.declarations.into_iter().map(|d| d.name).collect(),
            symbols.iter().map(|s| s.name().to_string()).collect(),
        )
    }

    #[test]
    fn dockerfile_declared_names_are_stage_aliases() {
        let (skeleton, symbols) = declared_names(
            Grammar::Dockerfile,
            "Dockerfile",
            "FROM alpine:3.19 AS build\nRUN echo hi\nFROM scratch\n",
        );
        assert_eq!(skeleton, vec!["build"]);
        assert_eq!(symbols, vec!["build"]);
    }

    #[test]
    fn hcl_declared_names_are_labels_and_attributes() {
        let (skeleton, symbols) = declared_names(
            Grammar::HclTerraform,
            "main.tf",
            "variable \"region\" {\n  default = \"us-east-1\"\n}\n\nresource \"aws_s3_bucket\" \"logs\" {\n  bucket = var.region\n}\n",
        );
        let want = ["region", "default", "aws_s3_bucket", "logs", "bucket"];
        assert_eq!(skeleton, want);
        assert_eq!(symbols, want);
    }

    #[test]
    fn html_skeleton_lists_ids_and_one_name_per_class() {
        let (skeleton, symbols) = declared_names(
            Grammar::Html,
            "page.html",
            "<div id=\"main\"><p class=\"a  b\">x</p></div>\n",
        );
        for names in [skeleton, symbols] {
            for want in ["div", "main", "p", "a", "b"] {
                assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
            }
            assert!(!names.iter().any(|n| n == "a  b"), "{names:?}");
        }
    }

    #[test]
    fn symbols_java_text_block_parses() {
        // Text blocks (Java 15+, triple-quoted strings) should parse without error.
        // The containing method should appear in symbols normally.
        let source = SourceFile::from_content(
            "HtmlUtil.java",
            r#"class HtmlUtil {
    String template() {
        return """
                <html>
                    <body>Hello</body>
                </html>
                """;
    }
}"#
            .to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(
                    symbols.iter().any(|s| s.name() == "template"),
                    "method 'template' should appear in symbols; got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_java_pattern_matching_parses() {
        // Pattern matching for instanceof (Java 16+) should parse without error.
        // The binding variable introduced by the pattern should appear in symbols.
        let source = SourceFile::from_content(
            "Checker.java",
            r#"class Checker {
    void check(Object obj) {
        if (obj instanceof String s) {
            System.out.println(s.length());
        }
    }
}"#
            .to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(
                    symbols.iter().any(|s| s.name() == "check"),
                    "method 'check' should appear in symbols; got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn skeleton_query_kotlin_function() {
        let source = SourceFile::from_content(
            "test.kt",
            "fun myFunction() {}".to_string(),
            Grammar::Kotlin,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "function"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_query_elixir_module() {
        let source = SourceFile::from_content(
            "test.ex",
            "defmodule MyModule do\n  def my_function do\n  end\nend".to_string(),
            Grammar::Elixir,
        )
        .unwrap();

        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(skel.declarations.iter().any(|d| d.kind == "module"));
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_captures_variable_declaration() {
        let source = SourceFile::from_content(
            "test.ts",
            "const API_URL = 'https://api.example.com';".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations.iter().any(|d| d.kind == "variable"),
                    "Should capture top-level variable declarations, got: {:?}",
                    skel.declarations
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_captures_namespace() {
        let source = SourceFile::from_content(
            "test.ts",
            "namespace MyApp { export const x = 1; }".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations.iter().any(|d| d.kind == "namespace"),
                    "Should capture namespace declarations, got: {:?}",
                    skel.declarations
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn skeleton_captures_abstract_class() {
        let source = SourceFile::from_content(
            "test.ts",
            "abstract class Base { abstract method(): void; }".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_skeleton(&source).unwrap();
        match result {
            QueryResult::Skeleton(skel) => {
                assert!(
                    skel.declarations
                        .iter()
                        .any(|d| d.kind == "class" && d.name == "Base"),
                    "Should capture abstract class declarations, got: {:?}",
                    skel.declarations
                );
            }
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn symbols_captures_arrow_function_via_variable() {
        let source = SourceFile::from_content(
            "test.ts",
            "const handler = (req: Request) => { return req; };".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(
                    symbols.iter().any(|s| s.name() == "handler"),
                    "Should capture arrow function assigned to variable, got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_captures_namespace() {
        let source = SourceFile::from_content(
            "test.ts",
            "namespace MyApp { export const x = 1; }".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(
                    symbols.iter().any(|s| s.name() == "MyApp"),
                    "Should capture namespace name, got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_csharp() {
        let source = SourceFile::from_content(
            "test.cs",
            "namespace Test { class MyClass { void Method() {} } }".to_string(),
            Grammar::CSharp,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_python() {
        let source = SourceFile::from_content(
            "test.py",
            "def func():\n    pass\nclass MyClass:\n    pass".to_string(),
            Grammar::Python,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
                assert!(symbols.iter().any(|s| s.name() == "func"));
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_rust() {
        let source = SourceFile::from_content(
            "test.rs",
            "fn function() {}\nstruct MyStruct {}".to_string(),
            Grammar::Rust,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_go() {
        let source = SourceFile::from_content(
            "test.go",
            "package main\nfunc main() {}".to_string(),
            Grammar::Go,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_c() {
        let source =
            SourceFile::from_content("test.c", "int main() { return 0; }".to_string(), Grammar::C)
                .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_cpp() {
        let source = SourceFile::from_content(
            "test.cpp",
            "int main() { return 0; }".to_string(),
            Grammar::Cpp,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_ruby() {
        let source = SourceFile::from_content(
            "test.rb",
            "def my_method\nend\nclass MyClass\nend".to_string(),
            Grammar::Ruby,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_java() {
        let source = SourceFile::from_content(
            "Test.java",
            "class Test { void method() {} }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_java_type_parameter_has_name() {
        // The `type_parameter` pattern in symbols.scm was missing the `@name` capture.
        // Without it, the match is silently dropped because `name` is None, and the
        // type parameter `T` never appears in the symbols output.
        let source = SourceFile::from_content(
            "Box.java",
            "public class Box<T> { }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                let type_param = symbols.iter().find(|s| s.name() == "T");
                assert!(
                    type_param.is_some(),
                    "Type parameter 'T' should appear in symbols; got: {:?}",
                    symbols.iter().map(|s| s.name()).collect::<Vec<_>>()
                );
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_kotlin() {
        let source = SourceFile::from_content(
            "test.kt",
            "fun function() {}\nclass MyClass {}".to_string(),
            Grammar::Kotlin,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_elixir() {
        let source = SourceFile::from_content(
            "test.ex",
            "defmodule MyModule do\n  def my_function do\n  end\nend".to_string(),
            Grammar::Elixir,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(!symbols.is_empty());
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_elixir_finds_a_def_without_parentheses() {
        let source = SourceFile::from_content(
            "app.ex",
            "defmodule Fixture.App do\n  def invoke do\n    Fixture.Lib.compute(21)\n  end\n  \
             defp helper, do: 1\n  defmacro m do\n  end\nend\n"
                .to_string(),
            Grammar::Elixir,
        )
        .unwrap();

        for (name, line, column) in [("invoke", 2, 7), ("helper", 5, 8), ("m", 6, 12)] {
            match execute_symbol(&source, name) {
                Ok(QueryResult::Symbol(found)) => {
                    let start = found.symbol.range().start();
                    assert_eq!((start.line(), start.column()), (line, column), "{name}");
                }
                other => panic!("{name}: expected Symbol, got {other:?}"),
            }
        }
    }

    #[test]
    fn references_query_python() {
        let source =
            SourceFile::from_content("test.py", "x = 1\nprint(x)".to_string(), Grammar::Python)
                .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_rust() {
        let source = SourceFile::from_content(
            "test.rs",
            "fn main() { let x = 1; println!(\"{}\", x); }".to_string(),
            Grammar::Rust,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_csharp() {
        let source = SourceFile::from_content(
            "test.cs",
            "class Test { void Method() { var x = 1; Console.WriteLine(x); } }".to_string(),
            Grammar::CSharp,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_cpp() {
        let source = SourceFile::from_content(
            "test.cpp",
            "int main() { int x = 1; return x; }".to_string(),
            Grammar::Cpp,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_ruby() {
        let source =
            SourceFile::from_content("test.rb", "x = 1\nputs x".to_string(), Grammar::Ruby)
                .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_java() {
        let source = SourceFile::from_content(
            "Test.java",
            "class Test { void method() { int x = 1; System.out.println(x); } }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_java_no_duplicate_captures() {
        // The broad leading `(identifier) @reference` and `(type_identifier) @reference`
        // patterns in references.scm cause every identifier that also matches a more
        // specific pattern (e.g., `(method_invocation name: (identifier) @reference)`) to
        // be captured twice. This test verifies each source position appears exactly once.
        let source = SourceFile::from_content(
            "Test.java",
            "class Test { void m() { foo.bar(); } }".to_string(),
            Grammar::Java,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                use std::collections::HashSet;
                let offsets: Vec<usize> =
                    references.iter().filter_map(|r| r.byte_offset()).collect();
                let unique: HashSet<usize> = offsets.iter().copied().collect();
                assert_eq!(
                    offsets.len(),
                    unique.len(),
                    "Duplicate reference captures: {} total vs {} unique byte positions",
                    offsets.len(),
                    unique.len()
                );
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_kotlin() {
        let source = SourceFile::from_content(
            "test.kt",
            "fun main() { val x = 1; println(x) }".to_string(),
            Grammar::Kotlin,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_elixir() {
        let source = SourceFile::from_content(
            "test.ex",
            "x = 1\nIO.inspect(x)".to_string(),
            Grammar::Elixir,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_c() {
        let source = SourceFile::from_content(
            "test.c",
            "int main() { int x = 1; return x; }".to_string(),
            Grammar::C,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                assert!(!references.is_empty());
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn symbols_query_typescript() {
        let source = SourceFile::from_content(
            "test.ts",
            r#"
function foo() {}
const x = 1;
class Bar {
  method() {}
}
"#
            .to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert!(symbols.len() >= 3);
                assert!(symbols.iter().any(|s| s.name() == "foo"));
                assert!(symbols.iter().any(|s| s.name() == "x"));
                assert!(symbols.iter().any(|s| s.name() == "Bar"));
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn symbols_query_empty_file() {
        let source =
            SourceFile::from_content("test.ts", "".to_string(), Grammar::TypeScript).unwrap();

        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                assert_eq!(symbols.len(), 0);
            }
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn references_query_typescript() {
        let source = SourceFile::from_content(
            "test.ts",
            r#"
function foo() {
    const x = 1;
    return x;
}
foo();
"#
            .to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                // Should find references to x, foo, etc.
                assert!(!references.is_empty(), "Should find some references");
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_query_go() {
        let source = SourceFile::from_content(
            "test.go",
            r#"
package main

func foo() int {
    x := 1
    return x
}

func main() {
    foo()
}
"#
            .to_string(),
            Grammar::Go,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        match result {
            QueryResult::References { references, .. } => {
                // Should find references to x, foo, main, etc.
                assert!(!references.is_empty(), "Should find some references");
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn execute_query_references() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();

        let result = execute_query(QueryType::References, &source).unwrap();
        match result {
            QueryResult::References { .. } => {}
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn references_result_serializes_tree_sitter_source() {
        let source = SourceFile::from_content(
            "test.ts",
            "const x = 1;\nconsole.log(x);".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_references(&source).unwrap();
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["source"], "tree-sitter");
    }

    #[test]
    fn references_result_with_lsp_source_serializes_lsp() {
        let result = QueryResult::References {
            references: Vec::new(),
            skipped_files: Vec::new(),
            source: ReferenceSource::Lsp,
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["source"], "lsp");
    }

    #[test]
    fn references_result_without_source_deserializes_as_tree_sitter() {
        // Payload shape from before the `source` field existed.
        let payload = serde_json::json!({
            "type": "References",
            "references": [],
        });

        let result: QueryResult = serde_json::from_value(payload).unwrap();
        match result {
            QueryResult::References { source, .. } => {
                assert_eq!(source, ReferenceSource::TreeSitter);
            }
            _ => panic!("Expected References result"),
        }
    }

    #[test]
    fn lsp_queries_not_implemented() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();

        let result = execute_query(QueryType::Callers, &source).unwrap();
        match result {
            QueryResult::NotImplemented { query, reason } => {
                assert_eq!(query, QueryType::Callers);
                assert!(reason.contains("LSP"));
            }
            _ => panic!("Expected NotImplemented"),
        }
    }

    #[test]
    fn tree_sitter_queries_work() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_query(QueryType::Skeleton, &source).unwrap();
        match result {
            QueryResult::Skeleton(_) => {}
            _ => panic!("Expected Skeleton result"),
        }
    }

    #[test]
    fn execute_query_symbols() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();

        let result = execute_query(QueryType::Symbols, &source).unwrap();
        match result {
            QueryResult::Symbols { .. } => {}
            _ => panic!("Expected Symbols result"),
        }
    }

    #[test]
    fn execute_query_symbol_returns_first_symbol() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}\nconst x = 1;".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_query(QueryType::Symbol, &source).unwrap();
        match result {
            QueryResult::Symbol(sym_result) => {
                assert_eq!(sym_result.symbol.name(), "foo");
            }
            _ => panic!("Expected Symbol result, got {:?}", result),
        }
    }

    #[test]
    fn execute_symbol_by_name() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}\nfunction bar() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let result = execute_symbol(&source, "bar").unwrap();
        match result {
            QueryResult::Symbol(sym_result) => {
                assert_eq!(sym_result.symbol.name(), "bar");
                assert!(!sym_result.source.is_empty());
            }
            _ => panic!("Expected Symbol result"),
        }
    }

    #[test]
    fn execute_symbol_not_found() {
        let source = SourceFile::from_content(
            "test.ts",
            "function foo() {}".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        let err = execute_symbol(&source, "nonexistent").unwrap_err();
        assert!(
            matches!(err, crate::ParserError::SymbolNotFound { ref name, .. } if name == "nonexistent"),
            "{err:?}"
        );
    }

    #[test]
    fn execute_query_definition_returns_location() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();
        let result = execute_query(QueryType::Definition, &source).unwrap();
        match result {
            QueryResult::Definition { location } => {
                assert!(location.is_some());
            }
            _ => panic!("Expected Definition result"),
        }
    }

    #[test]
    fn execute_query_all_lsp_queries() {
        let source =
            SourceFile::from_content("test.ts", "const x = 1;".to_string(), Grammar::TypeScript)
                .unwrap();

        let lsp_queries = [
            QueryType::Callers,
            QueryType::Callees,
            QueryType::Implementations,
            QueryType::Hierarchy,
            QueryType::Type,
            QueryType::Diagnostics,
            QueryType::Dependencies,
            QueryType::Impact,
            QueryType::Unused,
            QueryType::Tests,
            QueryType::Coverage,
            QueryType::Suggest,
        ];

        for query_type in lsp_queries {
            let result = execute_query(query_type, &source).unwrap();
            match result {
                QueryResult::NotImplemented { query, reason } => {
                    assert_eq!(query, query_type);
                    assert!(
                        reason.contains("LSP"),
                        "Query {:?} should mention LSP",
                        query_type
                    );
                }
                _ => panic!("Expected NotImplemented for {:?}", query_type),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_member_suffix_maps_to_enum_member() {
        assert_eq!(
            symbol_kind_from_suffix("enum_member"),
            Some(SymbolKind::EnumMember)
        );
    }

    #[test]
    fn tree_sitter_only() {
        assert!(QueryType::Skeleton.is_tree_sitter_only());
        assert!(QueryType::Symbol.is_tree_sitter_only());
        assert!(QueryType::Symbols.is_tree_sitter_only());
        assert!(QueryType::References.is_tree_sitter_only());
        assert!(QueryType::Definition.is_tree_sitter_only());
        assert!(!QueryType::Type.is_tree_sitter_only());
    }

    #[test]
    fn requires_lsp() {
        assert!(!QueryType::Skeleton.requires_lsp());
        assert!(!QueryType::Symbol.requires_lsp());
        assert!(!QueryType::Symbols.requires_lsp());
        assert!(!QueryType::References.requires_lsp());
        assert!(!QueryType::Definition.requires_lsp());
        assert!(QueryType::Callers.requires_lsp());
        assert!(QueryType::Callees.requires_lsp());
        assert!(QueryType::Implementations.requires_lsp());
        assert!(QueryType::Hierarchy.requires_lsp());
        assert!(QueryType::Type.requires_lsp());
        assert!(QueryType::Diagnostics.requires_lsp());
        assert!(QueryType::Dependencies.requires_lsp());
        assert!(QueryType::Impact.requires_lsp());
        assert!(QueryType::Unused.requires_lsp());
        assert!(QueryType::Tests.requires_lsp());
        assert!(QueryType::Coverage.requires_lsp());
        assert!(QueryType::Suggest.requires_lsp());
    }

    #[test]
    fn node_kind_to_symbol_kind_mapping() {
        // Table-driven test for all NodeKind -> SymbolKind mappings
        let cases = [
            (NodeKind::Function, SymbolKind::Function),
            (NodeKind::Class, SymbolKind::Class),
            (NodeKind::Object, SymbolKind::Class),
            (NodeKind::Struct, SymbolKind::Struct),
            (NodeKind::Interface, SymbolKind::Interface),
            (NodeKind::TypeAlias, SymbolKind::TypeAlias),
            (NodeKind::Enum, SymbolKind::Enum),
            (NodeKind::Variable, SymbolKind::Variable),
            (NodeKind::Constant, SymbolKind::Constant),
            (NodeKind::Parameter, SymbolKind::Parameter),
            (NodeKind::Property, SymbolKind::Field),
            (NodeKind::Method, SymbolKind::Method),
            (NodeKind::Constructor, SymbolKind::Constructor),
            (NodeKind::Event, SymbolKind::Event),
            (NodeKind::Module, SymbolKind::Module),
            (NodeKind::TypeParameter, SymbolKind::TypeParameter),
            (NodeKind::Implementation, SymbolKind::Class),
            (NodeKind::Decorator, SymbolKind::Attribute),
            (NodeKind::Element, SymbolKind::Unknown),
            (NodeKind::Import, SymbolKind::Import),
            (NodeKind::Export, SymbolKind::Export),
            (NodeKind::Block, SymbolKind::Unknown),
            (NodeKind::Call, SymbolKind::Unknown),
            (NodeKind::Identifier, SymbolKind::Unknown),
            (NodeKind::Comment, SymbolKind::Unknown),
            (NodeKind::Type, SymbolKind::Unknown),
            (NodeKind::Expression, SymbolKind::Unknown),
            (NodeKind::Unknown, SymbolKind::Unknown),
        ];
        for (node_kind, expected) in cases {
            assert_eq!(
                node_kind_to_symbol_kind(node_kind),
                expected,
                "node_kind_to_symbol_kind({:?}) should be {:?}",
                node_kind,
                expected
            );
        }
    }

    #[test]
    fn location_accessors() {
        let loc = Location::new(std::path::PathBuf::from("test.ts"), 5, 10);
        assert_eq!(loc.line(), 5);
        assert_eq!(loc.column(), 10);
        assert_eq!(loc.file(), &std::path::PathBuf::from("test.ts"));
        assert!(!loc.is_resolved());
    }

    #[test]
    fn location_with_byte_offset() {
        let loc = Location::full(std::path::PathBuf::from("test.ts"), 5, 10, 100);
        assert_eq!(loc.byte_offset(), Some(100));
        assert!(loc.is_resolved());
    }

    #[test]
    fn location_display() {
        let loc = Location::new(std::path::PathBuf::from("test.ts"), 5, 10);
        let display = format!("{}", loc);
        assert!(display.contains("test.ts"));
        assert!(display.contains("5"));
        assert!(display.contains("10"));
    }

    #[test]
    fn range_accessors() {
        let start = Location::new(std::path::PathBuf::from("test.ts"), 1, 1);
        let end = Location::new(std::path::PathBuf::from("test.ts"), 1, 10);
        let range = Range::new(start, end);
        assert_eq!(range.start().line(), 1);
        assert_eq!(range.end().column(), 10);
        assert_eq!(range.file(), &std::path::PathBuf::from("test.ts"));
    }

    #[test]
    fn range_from_byte_offsets() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 0, 100);
        assert_eq!(range.byte_len(), Some(100));
        assert!(range.is_resolved());
    }

    #[test]
    fn range_contains_location() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 10, 50);
        let inside = Location::full(std::path::PathBuf::from("test.ts"), 1, 1, 25);
        let outside = Location::full(std::path::PathBuf::from("test.ts"), 1, 1, 60);
        assert!(range.contains(&inside));
        assert!(!range.contains(&outside));
    }

    #[test]
    fn range_overlaps() {
        let r1 = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 10, 30);
        let r2 = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 20, 40);
        let r3 = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 40, 50);
        assert!(r1.overlaps(&r2));
        assert!(!r1.overlaps(&r3));
    }

    #[test]
    fn range_point() {
        let loc = Location::new(std::path::PathBuf::from("test.ts"), 1, 1);
        let range = Range::point(loc);
        assert!(range.is_point());
    }

    #[test]
    fn range_display() {
        let start = Location::new(std::path::PathBuf::from("test.ts"), 1, 5);
        let end = Location::new(std::path::PathBuf::from("test.ts"), 1, 15);
        let range = Range::new(start, end);
        let display = format!("{}", range);
        assert!(display.contains("test.ts"));
    }

    #[test]
    fn symbol_accessors() {
        let range = Range::new(
            Location::new(std::path::PathBuf::from("test.ts"), 1, 1),
            Location::new(std::path::PathBuf::from("test.ts"), 1, 10),
        );
        let sym = Symbol::new("foo".to_string(), SymbolKind::Function, range);
        assert_eq!(sym.name(), "foo");
        assert_eq!(sym.kind(), SymbolKind::Function);
    }

    #[test]
    fn symbol_kind_variants() {
        assert_eq!(SymbolKind::Function, SymbolKind::Function);
        assert_eq!(SymbolKind::Class, SymbolKind::Class);
        assert_eq!(SymbolKind::Method, SymbolKind::Method);
        assert_eq!(SymbolKind::Variable, SymbolKind::Variable);
        assert_eq!(SymbolKind::Interface, SymbolKind::Interface);
        assert_eq!(SymbolKind::Enum, SymbolKind::Enum);
        assert_eq!(SymbolKind::TypeAlias, SymbolKind::TypeAlias);
        assert_eq!(SymbolKind::Constant, SymbolKind::Constant);
        assert_eq!(SymbolKind::Parameter, SymbolKind::Parameter);
        assert_eq!(SymbolKind::Field, SymbolKind::Field);
        assert_eq!(SymbolKind::Import, SymbolKind::Import);
        assert_eq!(SymbolKind::Export, SymbolKind::Export);
        assert_eq!(SymbolKind::Unknown, SymbolKind::Unknown);
    }

    #[test]
    fn range_contains_line_column_fallback() {
        // Test fallback when byte offsets are not available
        let start = Location::new(std::path::PathBuf::from("test.ts"), 5, 1);
        let end = Location::new(std::path::PathBuf::from("test.ts"), 10, 20);
        let range = Range::new(start, end);

        // Location inside (by line/column)
        let inside = Location::new(std::path::PathBuf::from("test.ts"), 7, 10);
        assert!(range.contains(&inside));

        // Location before
        let before = Location::new(std::path::PathBuf::from("test.ts"), 3, 1);
        assert!(!range.contains(&before));

        // Location after
        let after = Location::new(std::path::PathBuf::from("test.ts"), 12, 1);
        assert!(!range.contains(&after));
    }

    #[test]
    fn range_overlaps_line_column_fallback() {
        // Test fallback when byte offsets are not available
        let r1_start = Location::new(std::path::PathBuf::from("test.ts"), 5, 1);
        let r1_end = Location::new(std::path::PathBuf::from("test.ts"), 10, 20);
        let r1 = Range::new(r1_start, r1_end);

        let r2_start = Location::new(std::path::PathBuf::from("test.ts"), 8, 1);
        let r2_end = Location::new(std::path::PathBuf::from("test.ts"), 15, 20);
        let r2 = Range::new(r2_start, r2_end);

        assert!(r1.overlaps(&r2));

        let r3_start = Location::new(std::path::PathBuf::from("test.ts"), 20, 1);
        let r3_end = Location::new(std::path::PathBuf::from("test.ts"), 25, 20);
        let r3 = Range::new(r3_start, r3_end);

        assert!(!r1.overlaps(&r3));
    }

    #[test]
    fn range_different_files_contains() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("a.ts"), 10, 50);
        let loc = Location::full(std::path::PathBuf::from("b.ts"), 1, 1, 25);
        assert!(!range.contains(&loc));
    }

    #[test]
    fn range_different_files_overlaps() {
        let r1 = Range::from_byte_offsets(std::path::PathBuf::from("a.ts"), 10, 30);
        let r2 = Range::from_byte_offsets(std::path::PathBuf::from("b.ts"), 20, 40);
        assert!(!r1.overlaps(&r2));
    }

    #[test]
    fn location_set_byte_offset() {
        let mut loc = Location::new(std::path::PathBuf::from("test.ts"), 1, 1);
        assert!(!loc.is_resolved());
        loc.set_byte_offset(100);
        assert!(loc.is_resolved());
        assert_eq!(loc.byte_offset(), Some(100));
    }

    #[test]
    fn symbol_with_builders() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 0, 100);
        let name_range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 10, 20);
        use act_lite_core::Visibility;
        let sym = Symbol::new("foo".to_string(), SymbolKind::Function, range)
            .with_name_range(name_range)
            .with_visibility(Visibility::Public)
            .with_qualified_name("module::foo".to_string())
            .with_container("module".to_string())
            .with_documentation("A function".to_string())
            .with_type_signature("() => void".to_string());

        assert_eq!(sym.name(), "foo");
        assert!(sym.name_range().is_some());
        assert_eq!(sym.visibility(), Visibility::Public);
        assert_eq!(sym.qualified_name(), Some("module::foo"));
        assert_eq!(sym.container(), Some("module"));
        assert_eq!(sym.documentation(), Some("A function"));
        assert_eq!(sym.type_signature(), Some("() => void"));
    }

    #[test]
    fn symbol_file_accessor() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("my/path.ts"), 0, 100);
        let sym = Symbol::new("bar".to_string(), SymbolKind::Variable, range);
        assert_eq!(sym.file(), &std::path::PathBuf::from("my/path.ts"));
    }

    #[test]
    fn symbol_range_accessor() {
        let range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 10, 50);
        let sym = Symbol::new("baz".to_string(), SymbolKind::Class, range);
        assert_eq!(sym.range().byte_len(), Some(40));
    }

    #[test]
    fn symbol_kind_is_type_definition() {
        assert!(SymbolKind::Class.is_type_definition());
        assert!(SymbolKind::Interface.is_type_definition());
        assert!(SymbolKind::Enum.is_type_definition());
        assert!(SymbolKind::TypeAlias.is_type_definition());
        assert!(SymbolKind::Struct.is_type_definition());
        assert!(!SymbolKind::Function.is_type_definition());
        assert!(!SymbolKind::Variable.is_type_definition());
    }

    #[test]
    fn symbol_kind_is_callable() {
        assert!(SymbolKind::Function.is_callable());
        assert!(SymbolKind::Method.is_callable());
        assert!(SymbolKind::Constructor.is_callable());
        assert!(!SymbolKind::Class.is_callable());
        assert!(!SymbolKind::Variable.is_callable());
    }

    #[test]
    fn symbol_kind_is_member() {
        assert!(SymbolKind::Method.is_member());
        assert!(SymbolKind::Field.is_member());
        assert!(SymbolKind::Constructor.is_member());
        assert!(!SymbolKind::Function.is_member());
        assert!(!SymbolKind::Class.is_member());
    }

    #[test]
    fn symbol_kind_is_value() {
        assert!(SymbolKind::Variable.is_value());
        assert!(SymbolKind::Constant.is_value());
        assert!(SymbolKind::Parameter.is_value());
        assert!(!SymbolKind::Function.is_value());
        assert!(!SymbolKind::Class.is_value());
    }

    #[test]
    fn visibility_variants() {
        use act_lite_core::Visibility;
        assert_eq!(Visibility::Public, Visibility::Public);
        assert_eq!(Visibility::Private, Visibility::Private);
        assert_eq!(Visibility::Protected, Visibility::Protected);
        assert_eq!(Visibility::Internal, Visibility::Internal);
        assert_ne!(Visibility::Public, Visibility::Private);
    }

    #[test]
    fn symbol_reference_types() {
        use act_lite_core::SymbolReference;
        let range = Range::from_byte_offsets(std::path::PathBuf::from("test.ts"), 0, 10);

        let read_ref = SymbolReference::read(range.clone());
        assert!(!read_ref.is_write());
        assert!(!read_ref.is_definition());

        let write_ref = SymbolReference::write(range.clone());
        assert!(write_ref.is_write());
        assert!(!write_ref.is_definition());

        let def_ref = SymbolReference::definition(range);
        assert!(!def_ref.is_write());
        assert!(def_ref.is_definition());
    }

    #[test]
    fn symbol_reference_accessors() {
        use act_lite_core::SymbolReference;
        let range = Range::from_byte_offsets(std::path::PathBuf::from("ref.ts"), 100, 200);
        let ref_ = SymbolReference::read(range);
        assert_eq!(ref_.range().byte_len(), Some(100));
        assert_eq!(ref_.file(), &std::path::PathBuf::from("ref.ts"));
    }

    #[test]
    fn location_with_byte_offset_constructor() {
        let loc = Location::with_byte_offset(std::path::PathBuf::from("test.ts"), 50);
        assert_eq!(loc.byte_offset(), Some(50));
        assert!(loc.is_resolved());
    }
}

#[cfg(test)]
mod bash_debug {
    use super::*;

    #[test]
    fn debug_bash_symbols_from_delete_scenario() {
        let content = "#!/usr/bin/env bash\nold_deploy() {\n    echo \"deprecated\"\n    exit 1\n}\nnew_deploy() {\n    echo \"deploying\"\n}\nnew_deploy\n";
        let tmp = tempfile::NamedTempFile::with_suffix(".sh").unwrap();
        std::fs::write(tmp.path(), content).unwrap();
        let source = SourceFile::load(tmp.path()).unwrap();
        let result = execute_symbols(&source).unwrap();
        match result {
            QueryResult::Symbols { symbols } => {
                println!("Symbols found: {}", symbols.len());
                for sym in &symbols {
                    println!("  - {:?} '{}' at {:?}", sym.kind(), sym.name(), sym.range());
                }
                assert!(!symbols.is_empty(), "should find at least one symbol");
                assert!(
                    symbols.iter().any(|s| s.name() == "old_deploy"),
                    "should find old_deploy"
                );
            }
            _ => panic!("Expected symbols"),
        }
    }
}

#[cfg(test)]
mod raw_query_tests {
    use crate::Workspace;
    use tempfile::tempdir;

    #[test]
    fn raw_query_captures_with_ranges() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.ts");
        std::fs::write(&file, "async function f() { await g(); await h(); }\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap();
        let source = ws.get_file(&file).unwrap();

        let q = "(await_expression) @await";
        let matches = super::execute_raw_query(source, q).unwrap();
        assert_eq!(matches.len(), 2, "two await expressions");
        for m in &matches {
            let cap = m.captures.iter().find(|c| c.name == "await").unwrap();
            assert!(cap.text.starts_with("await"));
            assert!(cap.range.start().line() >= 1);
            assert!(cap.range.start().column() >= 1);
        }
    }

    #[test]
    fn raw_query_invalid_query_errors() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("a.ts");
        std::fs::write(&file, "const x = 1;\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap();
        let source = ws.get_file(&file).unwrap();
        assert!(super::execute_raw_query(source, "(this is not a query").is_err());
    }

    #[test]
    fn raw_query_multi_capture_per_match() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("b.ts");
        std::fs::write(&file, "function greet() {}\nfunction bye() {}\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap();
        let source = ws.get_file(&file).unwrap();

        let q = "(function_declaration name: (identifier) @name) @fn";
        let matches = super::execute_raw_query(source, q).unwrap();
        assert_eq!(matches.len(), 2);
        for m in &matches {
            assert_eq!(m.captures.len(), 2);
            let names: Vec<&str> = m.captures.iter().map(|c| c.name.as_str()).collect();
            assert!(names.contains(&"name") && names.contains(&"fn"));
        }
    }

    #[test]
    fn raw_query_no_matches_returns_empty_vec() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("c.ts");
        std::fs::write(&file, "const x = 1;\n").unwrap();
        let mut ws = Workspace::new(dir.path()).unwrap();
        ws.open_file(&file).unwrap();
        let source = ws.get_file(&file).unwrap();

        let q = "(await_expression) @await"; // no awaits in this file
        let matches = super::execute_raw_query(source, q).unwrap();
        assert!(matches.is_empty());
    }
}
