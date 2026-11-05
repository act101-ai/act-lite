//! Query command implementation.

use crate::cli::{OutputFormat, QueryArgs, QueryType};
use crate::output::output;
use act_lite_core::Location;
use act_lite_lsp::{LspPool, LspServer};
use act_lite_parser::{Grammar, QueryResult, ReferenceSource};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Build the `QueryResult::References` payload for a live LSP answer.
///
/// Factored out of the `References` arm so the attribution is unit-testable
/// without a live server: an LSP `Ok` — empty ranges included — is a real
/// answer and is attributed `ReferenceSource::Lsp`. The tree-sitter fallback
/// path keeps the parser-produced `TreeSitter` attribution.
fn lsp_references_result(ranges: Vec<act_lite_core::Range>) -> QueryResult {
    QueryResult::References {
        references: ranges.into_iter().map(|r| r.start().clone()).collect(),
        skipped_files: Vec::new(),
        source: ReferenceSource::Lsp,
    }
}

/// Serialize a `QueryResult` and inject a top-level `"parse_error": true`
/// when `source`'s tree carries ERROR nodes; the key is entirely absent
/// (never `false`) on a clean parse, so clean output stays byte-identical.
/// Without this, an invalid file's partial symbol list is indistinguishable
/// from a complete answer.
fn with_parse_state(
    result: &QueryResult,
    source: &act_lite_parser::SourceFile,
) -> serde_json::Value {
    let mut value = serde_json::to_value(result).expect("QueryResult must serialize to JSON");
    if source.tree().root_node().has_error()
        && let serde_json::Value::Object(ref mut map) = value
    {
        map.insert("parse_error".to_string(), serde_json::Value::Bool(true));
    }
    value
}

/// Execute a query command.
pub async fn execute(
    args: &QueryArgs,
    format: OutputFormat,
    workspace: Option<&Path>,
    enable_lsp: bool,
) -> Result<()> {
    match &args.query_type {
        // === Tree-sitter queries (no LSP needed) ===
        QueryType::Skeleton { file } => {
            let source = load_source(file, workspace)?;
            let result = act_lite_parser::execute_skeleton(&source).with_context(|| {
                format!("failed to execute skeleton query on {}", file.display())
            })?;
            output(&result, format)?;
        }

        QueryType::Symbols { file } => {
            let source = load_source(file, workspace)?;
            let result = act_lite_parser::execute_symbols(&source).with_context(|| {
                format!("failed to execute symbols query on {}", file.display())
            })?;
            let result = with_parse_state(&result, &source);
            output(&result, format)?;
        }

        QueryType::Symbol { name, file } => {
            let file = file
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--file is required for symbol query"))?;
            let source = load_source(file, workspace)?;
            let result = act_lite_parser::execute_symbol(&source, name).with_context(|| {
                format!("failed to find symbol '{}' in {}", name, file.display())
            })?;
            output(&result, format)?;
        }

        QueryType::Definition {
            name,
            file,
            line,
            column,
        } => {
            // The same resolver the MCP `definition` tool uses: the name is
            // matched across every same-grammar file under the workspace
            // root, and the candidates are ranked by the position.
            let file = &validate_file_path(file, workspace)
                .with_context(|| format!("invalid path: {}", file.display()))?;
            let source = load_source(file, workspace)?;
            let root = resolve_workspace_root(workspace);
            let (symbol, range) =
                crate::definition::resolve_definition(&source, &root, name, *line, *column)
                    .map_err(|e| match e {
                        crate::definition::DefinitionError::NoSymbolAtPosition => {
                            anyhow::anyhow!(crate::definition::no_symbol_text(file, *line, *column))
                        }
                        crate::definition::DefinitionError::NotFound { symbol } => {
                            anyhow::anyhow!(crate::definition::not_found_text(
                                &symbol, file, *line, *column
                            ))
                        }
                    })?;
            output(&crate::definition::definition_json(&symbol, &range), format)?;
        }

        QueryType::References {
            name,
            file,
            line,
            column,
        } => {
            // LSP first when enabled: a live language server answers
            // cross-file at binding level and wins over the tree-sitter
            // baseline. The workspace walk is computed only when the
            // fallback will actually be used.
            // A language server rejects a relative file URI, so the file is
            // loaded by its canonical path.
            let file = &validate_file_path(file, workspace)
                .with_context(|| format!("invalid path: {}", file.display()))?;
            let source = load_source(file, workspace)?;
            let workspace_root = resolve_workspace_root(workspace);
            if enable_lsp {
                let pool = LspPool::new(workspace_root.clone());
                if let Some(client) = pool.get_query_client(source.grammar()).await {
                    let location = Location::new(file.clone(), *line, *column);
                    if let Ok(lsp_refs) =
                        act_lite_lsp::query_references(client.as_ref(), file, &location).await
                    {
                        let result = lsp_references_result(lsp_refs.ranges);
                        output(&result, format)?;
                        return Ok(());
                    }
                }
                if let Some(hint) = lsp_install_hint_once(source.grammar(), &pool) {
                    eprintln!("hint: {hint}");
                }
            }

            // Tree-sitter fallback: a name-level (not binding-level)
            // cross-file baseline over same-grammar files in the workspace.
            // The result's `source` field says `tree-sitter`.
            let walk = act_lite_parser::walk_reference_siblings(
                &workspace_root,
                file,
                source.grammar(),
                act_lite_parser::MAX_REFERENCE_WALK_FILES,
                act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES,
            )?;
            let mut result = act_lite_parser::execute_references_for_symbol_across(
                &source,
                &walk.siblings,
                name,
                *line,
                *column,
            )
            .with_context(|| {
                format!(
                    "failed to find references for '{}' in {} at {}:{}",
                    name,
                    file.display(),
                    line,
                    column
                )
            })?;
            // The multi-file API scans in-memory files and produces no skips
            // itself; the walk's disclosed skip list is attached here.
            if let QueryResult::References { skipped_files, .. } = &mut result {
                *skipped_files = walk.skipped;
            }
            output(&result, format)?;
        }

        // === LSP queries (fail if --disable-lsp was passed) ===
        QueryType::Callers { name, file } => {
            require_lsp(enable_lsp, "callers")?;
            let (pool, grammar, file) = create_lsp_pool(file, workspace)?;
            let file = &file;
            let client = get_lsp_client(&pool, grammar, workspace, "callers").await?;
            let position = find_symbol_position(file, name)?;
            let result = act_lite_lsp::query_callers(client.as_ref(), file, &position)
                .await
                .with_context(|| format!("failed to query callers of '{}'", name))?;
            let qr = QueryResult::Callers {
                callers: result.callers,
            };
            output(&qr, format)?;
        }

        QueryType::Callees { name, file } => {
            require_lsp(enable_lsp, "callees")?;
            let (pool, grammar, file) = create_lsp_pool(file, workspace)?;
            let file = &file;
            let client = get_lsp_client(&pool, grammar, workspace, "callees").await?;
            let position = find_symbol_position(file, name)?;
            let result = act_lite_lsp::query_callees(client.as_ref(), file, &position)
                .await
                .with_context(|| format!("failed to query callees of '{}'", name))?;
            let qr = QueryResult::Callees {
                callees: result.callees,
            };
            output(&qr, format)?;
        }

        QueryType::Implementations { name, file } => {
            require_lsp(enable_lsp, "implementations")?;
            let (pool, grammar, file) = create_lsp_pool(file, workspace)?;
            let file = &file;
            let client = get_lsp_client(&pool, grammar, workspace, "implementations").await?;
            let position = find_symbol_position(file, name)?;
            let result = act_lite_lsp::query_implementations(client.as_ref(), file, &position)
                .await
                .with_context(|| format!("failed to query implementations of '{}'", name))?;
            let qr = QueryResult::Implementations {
                locations: result.locations,
            };
            output(&qr, format)?;
        }

        QueryType::SymbolsBatch { files, ids, kinds } => {
            let ws_root = resolve_workspace_root(workspace);
            if let Some(files_str) = files {
                let file_paths: Vec<PathBuf> = files_str
                    .split(',')
                    .map(|f| ws_root.join(f.trim()))
                    .collect();
                let result =
                    execute_symbols_batch_by_files(&file_paths, kinds.as_deref(), &ws_root)?;
                output(&result, format)?;
            } else if let Some(ids_str) = ids {
                let id_list: Vec<String> =
                    ids_str.split(',').map(|s| s.trim().to_string()).collect();
                let result = execute_symbols_batch_by_ids(&id_list, &ws_root)?;
                output(&result, format)?;
            } else {
                bail!("symbols-batch requires either --files or --ids");
            }
        }

        QueryType::RepoOutline {
            depth,
            include,
            exclude,
            symbols,
            max_files,
        } => {
            let ws_root = resolve_workspace_root(workspace);
            let result = execute_repo_outline(
                &ws_root,
                &ws_root,
                *depth,
                include.as_deref(),
                exclude.as_deref(),
                *symbols,
                *max_files,
                false,
            )?;
            output(&result, format)?;
        }
    }
    Ok(())
}

// === Helper functions ===

/// The error for a file outside act-lite's grammars. It names the file's
/// extension and lists every supported language, so the caller knows both
/// what was refused and what would be accepted.
pub fn unsupported_language_error(path: &Path) -> anyhow::Error {
    let mut languages: Vec<&str> = Grammar::all().iter().map(|g| g.name()).collect();
    languages.sort_unstable();
    let what = match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("unsupported file type '.{ext}'"),
        None => "unsupported file type (no extension)".to_string(),
    };
    anyhow::anyhow!(
        "{what}: {}. act-lite supports these languages: {}",
        path.display(),
        languages.join(", ")
    )
}

/// Validates a file path for safety.
///
/// Checks:
/// - Path exists
/// - Path is a file (not directory)
/// - Path is within workspace (if workspace specified)
/// - No path traversal components (..)
///
/// Canonicalization is done atomically to prevent TOCTOU races. Returns the
/// canonical path.
pub fn validate_file_path(path: &Path, workspace: Option<&Path>) -> Result<PathBuf> {
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            bail!(
                "path traversal not allowed: {} contains '..'",
                path.display()
            );
        }
    }

    // Canonicalize first to resolve symlinks and check existence atomically.
    let canonical_path = path
        .canonicalize()
        .with_context(|| format!("file not found or inaccessible: {}", path.display()))?;

    if !canonical_path.is_file() {
        bail!("path is not a file: {}", path.display());
    }

    if let Some(workspace) = workspace {
        let canonical_workspace = workspace
            .canonicalize()
            .with_context(|| format!("workspace not found: {}", workspace.display()))?;

        if !canonical_path.starts_with(&canonical_workspace) {
            bail!(
                "path {} is outside workspace {}",
                path.display(),
                workspace.display()
            );
        }
    }

    Ok(canonical_path)
}

/// Load and parse a source file, validating the path and its language.
fn load_source(file: &Path, workspace: Option<&Path>) -> Result<act_lite_parser::SourceFile> {
    validate_file_path(file, workspace)
        .with_context(|| format!("invalid path: {}", file.display()))?;
    if grammar_for_file(file).is_none() {
        return Err(unsupported_language_error(file));
    }
    act_lite_parser::SourceFile::load(file)
        .with_context(|| format!("failed to parse {}", file.display()))
}

/// Get the grammar for a file: filename conventions first, then extension.
fn grammar_for_file(file: &Path) -> Option<Grammar> {
    Grammar::from_path(file)
}

/// Resolve the workspace root from the optional workspace path.
///
/// The root is canonicalized when it exists, so paths derived from it match
/// the canonical file paths handed to language servers.
pub(crate) fn resolve_workspace_root(workspace: Option<&Path>) -> PathBuf {
    let root = workspace
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    root.canonicalize().unwrap_or(root)
}

/// Check that LSP is enabled, returning a helpful error if not.
fn require_lsp(enable_lsp: bool, query_name: &str) -> Result<()> {
    if !enable_lsp {
        bail!(
            "query type '{}' requires LSP but LSP is disabled. Remove --disable-lsp or unset ACT_LITE_DISABLE_LSP",
            query_name
        );
    }
    Ok(())
}

/// Validate a file for an LSP query, and create the pool for its workspace.
/// Returns the file's canonical absolute path: a language server rejects a
/// relative file URI.
fn create_lsp_pool(file: &Path, workspace: Option<&Path>) -> Result<(LspPool, Grammar, PathBuf)> {
    let canonical = validate_file_path(file, workspace)
        .with_context(|| format!("invalid path: {}", file.display()))?;
    let grammar = grammar_for_file(file).ok_or_else(|| unsupported_language_error(file))?;
    let pool = LspPool::new(resolve_workspace_root(workspace));
    Ok((pool, grammar, canonical))
}

/// Get an LSP client from the pool. When none is available the error names
/// the server binary to install, and how to install it when that is known.
async fn get_lsp_client(
    pool: &LspPool,
    grammar: Grammar,
    workspace: Option<&Path>,
    query_name: &str,
) -> Result<std::sync::Arc<act_lite_lsp::LspClient>> {
    pool.get_query_client(grammar).await.ok_or_else(|| {
        lsp_unavailable_error(grammar, &resolve_workspace_root(workspace), query_name)
    })
}

/// The error for a query that needs a language server none is available
/// for. It names the server binary to install, and how to install it when
/// that is known, or says that act-lite has no server for the language.
pub fn lsp_unavailable_error(
    grammar: Grammar,
    workspace_root: &Path,
    query_name: &str,
) -> anyhow::Error {
    let Some(server) = LspServer::for_grammar(grammar) else {
        return anyhow::anyhow!(
            "query type '{}' needs a language server, and act-lite has none configured for {} files",
            query_name,
            grammar.name()
        );
    };
    let install =
        act_lite_lsp::installer::install_hint(&server.config(workspace_root.to_path_buf()));
    anyhow::anyhow!(
        "query type '{}' needs the {} language server, which is not available. \
         Install `{}` with {} and make sure it is on PATH",
        query_name,
        grammar.name(),
        server.binary_name(),
        install
    )
}

/// Set once an install hint has been given in this process.
static LSP_HINT_GIVEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The hint to install the language server for `grammar`, the first time in
/// this process that the server is configured but not on PATH; `None`
/// otherwise. It names the server binary, its install command and its
/// homepage, or, when `pool` already ran the install command, why the server
/// is still unavailable.
pub(crate) fn lsp_install_hint_once(grammar: Grammar, pool: &LspPool) -> Option<String> {
    let server = LspServer::for_grammar(grammar)?;
    let config = server.config(pool.root().clone());
    if act_lite_lsp::installer::is_installed(&config)
        || LSP_HINT_GIVEN.swap(true, std::sync::atomic::Ordering::Relaxed)
    {
        return None;
    }
    Some(match pool.install_failure(grammar) {
        Some(failure) => format!("{failure}."),
        None => format!(
            "Install `{}` for binding-level answers with {}.",
            server.binary_name(),
            act_lite_lsp::installer::install_hint(&config)
        ),
    })
}

/// Find a symbol's position in a file by name using tree-sitter.
fn find_symbol_position(file: &Path, name: &str) -> Result<Location> {
    let source = act_lite_parser::SourceFile::load(file)
        .with_context(|| format!("failed to parse {}", file.display()))?;
    let result = act_lite_parser::execute_symbol(&source, name)?;
    match result {
        QueryResult::Symbol(sym) => Ok(sym.symbol.range().start().clone()),
        _ => bail!("symbol '{}' not found in {}", name, file.display()),
    }
}

// === Repo outline types and implementation ===

/// Result of repo-outline query.
#[derive(Debug, Clone, Serialize)]
pub struct RepoOutlineResult {
    pub root: String,
    pub files: Vec<RepoOutlineFile>,
    pub total_files: usize,
    /// True when `max_files` cut the list: more files matched than shown.
    pub truncated: bool,
    pub languages: Vec<LanguageSummary>,
}

/// A single file entry in a repo outline.
#[derive(Debug, Clone, Serialize)]
pub struct RepoOutlineFile {
    pub path: String,
    pub language: String,
    pub lines: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<String>>,
    /// Why the symbols could not be queried for this file, when the grammar
    /// refused it — an absent `symbols` field alone is indistinguishable from
    /// "no symbols" and would hide the refusal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Summary of a language's presence in the repo.
#[derive(Debug, Clone, Serialize)]
pub struct LanguageSummary {
    pub language: String,
    pub files: usize,
    pub lines: usize,
}

/// Execute a repo-outline query, walking the workspace directory.
#[allow(clippy::too_many_arguments)]
pub fn execute_repo_outline(
    workspace_root: &Path,
    traversal_root: &Path,
    depth: Option<u32>,
    include: Option<&str>,
    exclude: Option<&str>,
    symbols: bool,
    max_files: u32,
    relative: bool,
) -> Result<RepoOutlineResult> {
    use std::collections::HashMap;
    use std::io::BufRead;

    let mut files = Vec::new();
    let mut lang_stats: HashMap<String, (usize, usize)> = HashMap::new();
    let mut truncated = false;
    // A symlink may point out of the workspace; its target is not read.
    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    for entry in walkdir::WalkDir::new(traversal_root)
        .max_depth(depth.map(|d| d as usize).unwrap_or(usize::MAX))
        .sort_by_file_name()
        .into_iter()
        // Build output, dependencies and VCS state are not the repository's
        // code; the same directories `definition` skips.
        .filter_entry(|e| {
            e.depth() == 0
                || !(e.file_type().is_dir()
                    && matches!(
                        e.file_name().to_str(),
                        Some(".git" | "target" | "node_modules")
                    ))
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file()
            || !path
                .canonicalize()
                .is_ok_and(|canonical| canonical.starts_with(&canonical_root))
        {
            continue;
        }

        // Always workspace-relative for glob filter matching
        let ws_rel = path
            .strip_prefix(workspace_root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        // Apply include/exclude glob filters against workspace-relative path
        if let Some(inc) = include
            && !simple_glob_match(inc, &ws_rel)
        {
            continue;
        }
        if let Some(exc) = exclude
            && simple_glob_match(exc, &ws_rel)
        {
            continue;
        }

        // Display path respects the relative flag
        let display_path = if relative {
            path.strip_prefix(traversal_root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(ws_rel.clone())
        } else {
            ws_rel.clone()
        };

        // Detect language via the loader's classifier (filename conventions
        // first, then extension)
        let language = match Grammar::from_path(path) {
            Some(g) => g.name().to_string(),
            None => continue,
        };

        if files.len() >= max_files as usize {
            truncated = true;
            break;
        }

        // Count lines
        let line_count = match std::fs::File::open(path) {
            Ok(f) => std::io::BufReader::new(f).lines().count(),
            Err(_) => 0,
        };

        // Optionally get top-level symbols. A query failure is reported per
        // file rather than swallowed.
        let mut per_file_error = None;
        let symbol_names = if symbols {
            match act_lite_parser::SourceFile::load(path) {
                Ok(source) => match act_lite_parser::execute_skeleton(&source) {
                    Ok(QueryResult::Skeleton(sk)) => {
                        Some(sk.declarations.into_iter().map(|d| d.name).collect())
                    }
                    Ok(_) => None,
                    Err(e) => {
                        per_file_error = Some(e.to_string());
                        None
                    }
                },
                Err(_) => None,
            }
        } else {
            None
        };

        // Update language stats
        let stats = lang_stats.entry(language.clone()).or_insert((0, 0));
        stats.0 += 1;
        stats.1 += line_count;

        files.push(RepoOutlineFile {
            path: display_path,
            language,
            lines: line_count,
            symbols: symbol_names,
            error: per_file_error,
        });
    }

    let total_files = files.len();
    let mut languages: Vec<LanguageSummary> = lang_stats
        .into_iter()
        .map(|(language, (file_count, line_count))| LanguageSummary {
            language,
            files: file_count,
            lines: line_count,
        })
        .collect();
    languages.sort_by_key(|l| std::cmp::Reverse(l.lines));

    Ok(RepoOutlineResult {
        root: traversal_root.display().to_string(),
        files,
        total_files,
        truncated,
        languages,
    })
}

/// Simple glob matching supporting `*` (any chars within a segment) and `**` (any path).
///
/// Supports patterns like `*.ts`, `src/**/*.rs`, `*.rs`.
fn simple_glob_match(pattern: &str, path: &str) -> bool {
    // Handle simple extension patterns like "*.ts"
    if let Some(ext_pattern) = pattern.strip_prefix("*.")
        && !ext_pattern.contains('*')
        && !ext_pattern.contains('/')
    {
        // Simple extension match: *.ts matches "foo.ts" and "src/foo.ts"
        return path.ends_with(&format!(".{}", ext_pattern));
    }

    // Fallback: basic fnmatch-style with * matching non-/ chars
    glob_match_recursive(pattern.as_bytes(), path.as_bytes())
}

fn glob_match_recursive(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (Some(b'*'), _) => {
            if pattern.get(1) == Some(&b'*') {
                // ** matches everything including /
                let rest = if pattern.get(2) == Some(&b'/') {
                    &pattern[3..]
                } else {
                    &pattern[2..]
                };
                // Try matching rest at every position
                for i in 0..=text.len() {
                    if glob_match_recursive(rest, &text[i..]) {
                        return true;
                    }
                }
                false
            } else {
                // * matches everything except /
                for i in 0..=text.len() {
                    if i > 0 && text[i - 1] == b'/' {
                        break;
                    }
                    if glob_match_recursive(&pattern[1..], &text[i..]) {
                        return true;
                    }
                }
                false
            }
        }
        (Some(b'?'), Some(_)) => glob_match_recursive(&pattern[1..], &text[1..]),
        (Some(a), Some(b)) if a == b => glob_match_recursive(&pattern[1..], &text[1..]),
        _ => false,
    }
}

// === Symbols-batch types and implementation ===

/// Result of symbols-batch query.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolsBatchResult {
    pub results: Vec<FileSymbolGroup>,
    pub total_symbols: usize,
}

/// Symbols grouped by file.
#[derive(Debug, Clone, Serialize)]
pub struct FileSymbolGroup {
    pub file: String,
    pub symbols: Vec<SymbolEntry>,
    /// `Some(true)` when the file's tree carries ERROR nodes; absent (never
    /// `Some(false)`) on a clean parse, so clean batch entries stay
    /// byte-identical.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<bool>,
}

/// A symbol entry in batch results.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: String,
    pub range: act_lite_core::CompactRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// Execute symbols-batch by file paths, optionally filtering by kind.
pub fn execute_symbols_batch_by_files(
    files: &[PathBuf],
    kinds: Option<&str>,
    workspace_root: &Path,
) -> Result<SymbolsBatchResult> {
    let kind_filter: Option<Vec<&str>> = kinds.map(|k| k.split(',').map(|s| s.trim()).collect());
    let mut results = Vec::new();
    let mut total = 0;

    for file in files {
        if Grammar::from_path(file).is_none() {
            return Err(unsupported_language_error(file));
        }
        let source = act_lite_parser::SourceFile::load(file)
            .with_context(|| format!("failed to load {}", file.display()))?;

        let skeleton = act_lite_parser::execute_skeleton(&source)?;
        if let QueryResult::Skeleton(sk) = skeleton {
            let rel_path = file.strip_prefix(workspace_root).unwrap_or(file);
            let rel_str = rel_path.display().to_string();

            let mut symbols = Vec::new();
            for decl in &sk.declarations {
                if let Some(ref filter) = kind_filter
                    && !filter.iter().any(|k| decl.kind == *k)
                {
                    continue;
                }

                let sym_id = act_lite_parser::SymbolId::new(&rel_str, &[&decl.name], &decl.kind);
                symbols.push(SymbolEntry {
                    name: decl.name.clone(),
                    kind: decl.kind.clone(),
                    range: act_lite_core::CompactRange::from(&decl.range),
                    id: Some(sym_id.to_string()),
                    source: None,
                });
            }
            total += symbols.len();
            let parse_error = source.tree().root_node().has_error().then_some(true);
            results.push(FileSymbolGroup {
                file: rel_str,
                symbols,
                parse_error,
            });
        }
    }

    Ok(SymbolsBatchResult {
        results,
        total_symbols: total,
    })
}

/// Execute symbols-batch by stable symbol IDs.
pub fn execute_symbols_batch_by_ids(
    ids: &[String],
    workspace_root: &Path,
) -> Result<SymbolsBatchResult> {
    use std::collections::HashMap;

    // Group IDs by file
    let mut file_ids: HashMap<String, Vec<&str>> = HashMap::new();
    for id_str in ids {
        if let Some(parsed) = act_lite_parser::SymbolId::parse(id_str) {
            file_ids
                .entry(parsed.file().to_string())
                .or_default()
                .push(id_str);
        }
    }

    let mut results = Vec::new();
    let mut total = 0;

    for (file_path, id_list) in &file_ids {
        let full_path = workspace_root.join(file_path);
        let source = act_lite_parser::SourceFile::load(&full_path)
            .with_context(|| format!("failed to load {}", file_path))?;

        let skeleton = act_lite_parser::execute_skeleton(&source)?;

        if let QueryResult::Skeleton(sk) = skeleton {
            let mut symbols = Vec::new();
            for id_str in id_list {
                if let Some(parsed) = act_lite_parser::SymbolId::parse(id_str) {
                    for decl in &sk.declarations {
                        if (decl.name == parsed.qualified_name()
                            || decl.name == parsed.simple_name())
                            && decl.kind == parsed.kind()
                        {
                            symbols.push(SymbolEntry {
                                name: decl.name.clone(),
                                kind: decl.kind.clone(),
                                range: act_lite_core::CompactRange::from(&decl.range),
                                id: Some(id_str.to_string()),
                                source: None,
                            });
                            break;
                        }
                    }
                }
            }
            total += symbols.len();
            let parse_error = source.tree().root_node().has_error().then_some(true);
            results.push(FileSymbolGroup {
                file: file_path.clone(),
                symbols,
                parse_error,
            });
        }
    }

    Ok(SymbolsBatchResult {
        results,
        total_symbols: total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::QueryType;
    use std::path::PathBuf;

    fn create_temp_file(content: &str) -> (tempfile::TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(&file_path, content).unwrap();
        (temp_dir, file_path)
    }

    #[tokio::test]
    async fn test_skeleton_query_success() {
        let (_temp_dir, file_path) = create_temp_file("function foo() { return 1; }");
        let args = QueryArgs {
            query_type: QueryType::Skeleton { file: file_path },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_symbols_query_success() {
        let (_temp_dir, file_path) = create_temp_file("const x = 1;\nclass Bar {}");
        let args = QueryArgs {
            query_type: QueryType::Symbols { file: file_path },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skeleton_query_with_text_format() {
        let (_temp_dir, file_path) = create_temp_file("function foo() {}");
        let args = QueryArgs {
            query_type: QueryType::Skeleton { file: file_path },
        };
        let result = execute(&args, OutputFormat::Text, None, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_skeleton_query_invalid_file() {
        let args = QueryArgs {
            query_type: QueryType::Skeleton {
                file: PathBuf::from("/nonexistent/file.ts"),
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_skeleton_query_unsupported_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("prog.hs");
        std::fs::write(&file_path, "main = putStrLn \"hi\"\n").unwrap();
        let args = QueryArgs {
            query_type: QueryType::Skeleton { file: file_path },
        };
        let err = execute(&args, OutputFormat::Json, None, false)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("'.hs'"), "must name the extension: {err}");
        assert!(
            err.contains("Python"),
            "must list supported languages: {err}"
        );
    }

    #[tokio::test]
    async fn test_symbol_query_success() {
        let (_temp_dir, file_path) = create_temp_file("function myFunc() { return 1; }");
        let args = QueryArgs {
            query_type: QueryType::Symbol {
                name: "myFunc".to_string(),
                file: Some(file_path),
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_definition_query_success() {
        let (_temp_dir, file_path) = create_temp_file("function myFunc() { return 1; }");
        let args = QueryArgs {
            query_type: QueryType::Definition {
                name: "myFunc".to_string(),
                file: file_path,
                line: 1,
                column: 10,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lsp_query_requires_flag() {
        let (_temp_dir, file_path) = create_temp_file("function foo() {}");
        let args = QueryArgs {
            query_type: QueryType::Callers {
                name: "foo".to_string(),
                file: file_path,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("--disable-lsp"),
            "Error should mention --disable-lsp flag: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_query_with_workspace_constraint() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let args = QueryArgs {
            query_type: QueryType::Skeleton {
                file: file_path.clone(),
            },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_workspace_constraint_violation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let other_dir = tempfile::tempdir().unwrap();
        let file_path = other_dir.path().join("test.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let args = QueryArgs {
            query_type: QueryType::Skeleton { file: file_path },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_references_query_treesitter_fallback() {
        let (_temp_dir, file_path) =
            create_temp_file("function foo() { return 1; }\nconst x = foo();");
        let args = QueryArgs {
            query_type: QueryType::References {
                name: "foo".to_string(),
                file: file_path,
                line: 1,
                column: 10,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(
            result.is_ok(),
            "references tree-sitter fallback should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_references_query_invalid_file() {
        let args = QueryArgs {
            query_type: QueryType::References {
                name: "foo".to_string(),
                file: PathBuf::from("/nonexistent/file.ts"),
                line: 1,
                column: 10,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_references_query_text_format() {
        let (_temp_dir, file_path) =
            create_temp_file("function foo() { return 1; }\nconst x = foo();");
        let args = QueryArgs {
            query_type: QueryType::References {
                name: "foo".to_string(),
                file: file_path,
                line: 1,
                column: 10,
            },
        };
        let result = execute(&args, OutputFormat::Text, None, false).await;
        assert!(
            result.is_ok(),
            "references with text format should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_callees_requires_lsp() {
        let (_temp_dir, file_path) = create_temp_file("function foo() {}");
        let args = QueryArgs {
            query_type: QueryType::Callees {
                name: "foo".to_string(),
                file: file_path,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("callees") && err_msg.contains("--disable-lsp"),
            "should mention callees and --disable-lsp: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_implementations_requires_lsp() {
        let (_temp_dir, file_path) = create_temp_file("interface Foo {}");
        let args = QueryArgs {
            query_type: QueryType::Implementations {
                name: "Foo".to_string(),
                file: file_path,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("implementations") && err_msg.contains("--disable-lsp"),
            "should mention implementations and --disable-lsp: {}",
            err_msg
        );
    }

    #[test]
    fn test_grammar_for_file_known_extensions() {
        for (path, name) in [
            ("/some/file.ts", "TypeScript"),
            ("/some/file.rs", "Rust"),
            ("/some/file.py", "Python"),
            ("/some/file.go", "Go"),
            ("/some/Dockerfile", "Dockerfile"),
        ] {
            let grammar = grammar_for_file(&PathBuf::from(path));
            assert_eq!(grammar.map(|g| g.name()), Some(name), "{path}");
        }
    }

    #[test]
    fn test_grammar_for_file_unknown_extension() {
        assert!(grammar_for_file(&PathBuf::from("/some/file.xyz")).is_none());
    }

    #[test]
    fn test_grammar_for_file_no_extension() {
        assert!(grammar_for_file(&PathBuf::from("/some/LICENSE")).is_none());
    }

    #[test]
    fn test_unsupported_language_error_without_extension() {
        let err = unsupported_language_error(Path::new("/some/LICENSE")).to_string();
        assert!(err.contains("no extension"), "{err}");
        assert!(err.contains("/some/LICENSE"), "{err}");
        assert!(err.contains("TypeScript"), "{err}");
    }

    #[test]
    fn test_resolve_workspace_root_with_path() {
        let path = Path::new("/some/workspace");
        let result = resolve_workspace_root(Some(path));
        assert_eq!(result, PathBuf::from("/some/workspace"));
    }

    #[test]
    fn test_resolve_workspace_root_without_path() {
        let result = resolve_workspace_root(None);
        // Should return current dir or "."
        assert!(!result.as_os_str().is_empty());
    }

    #[test]
    fn test_require_lsp_enabled() {
        let result = require_lsp(true, "callers");
        assert!(result.is_ok());
    }

    #[test]
    fn test_require_lsp_disabled() {
        let result = require_lsp(false, "callers");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("callers"));
        assert!(err_msg.contains("--disable-lsp"));
        assert!(err_msg.contains("ACT_LITE_DISABLE_LSP"));
    }

    #[test]
    fn test_require_lsp_disabled_different_query() {
        let result = require_lsp(false, "implementations");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("implementations"));
    }

    #[cfg(unix)]
    #[test]
    fn test_create_lsp_pool_gives_lsp_a_canonical_absolute_path() {
        // A language server rejects a relative file URI ("url is not a
        // file"), and a symlinked one names a file it never opened.
        let real = tempfile::tempdir().unwrap();
        std::fs::write(real.path().join("a.ts"), "export const a = 1;\n").unwrap();
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("ws");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();

        let (_pool, _grammar, file) = create_lsp_pool(&link.join("a.ts"), None).unwrap();
        assert_eq!(file, real.path().join("a.ts").canonicalize().unwrap());

        let relative = Path::new("tests/fixtures/sample.ts");
        let (_pool, _grammar, file) = create_lsp_pool(relative, None).unwrap();
        assert!(file.is_absolute(), "{}", file.display());
        assert_eq!(file, relative.canonicalize().unwrap());
    }

    #[tokio::test]
    async fn test_create_lsp_pool_known_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(&file_path, "").unwrap();
        let result = create_lsp_pool(&file_path, None);
        assert!(result.is_ok());
        let (_pool, grammar, _) = result.unwrap();
        assert_eq!(grammar.name(), "TypeScript");
    }

    #[test]
    fn test_create_lsp_pool_unknown_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.xyz");
        std::fs::write(&file_path, "").unwrap();
        let result = create_lsp_pool(&file_path, None);
        let err_msg = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expected error for unknown extension"),
        };
        assert!(
            err_msg.contains("unsupported file type '.xyz'"),
            "should name the unsupported extension: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_create_lsp_pool_with_workspace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.py");
        std::fs::write(&file_path, "").unwrap();
        let result = create_lsp_pool(&file_path, Some(temp_dir.path()));
        assert!(result.is_ok());
        let (_pool, grammar, _) = result.unwrap();
        assert_eq!(grammar.name(), "Python");
    }

    #[tokio::test]
    async fn test_get_lsp_client_serverless_grammar_names_the_gap() {
        let pool = LspPool::disabled();
        let err = get_lsp_client(&pool, Grammar::Json, None, "callers")
            .await
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("callers") && err.contains("JSON"),
            "must say which query lacks a server for which language: {err}"
        );
    }

    #[tokio::test]
    async fn test_get_lsp_client_unavailable_server_names_binary() {
        let pool = LspPool::disabled();
        let err = get_lsp_client(&pool, Grammar::Rust, None, "callers")
            .await
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("rust-analyzer"),
            "must name the binary to install: {err}"
        );
    }

    #[tokio::test]
    async fn test_symbol_query_missing_file() {
        let args = QueryArgs {
            query_type: QueryType::Symbol {
                name: "foo".to_string(),
                file: None,
            },
        };
        let result = execute(&args, OutputFormat::Json, None, false).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("--file is required"),
            "should mention --file is required: {}",
            err_msg
        );
    }

    #[test]
    fn test_find_symbol_position_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(&file_path, "function myFunc() { return 1; }").unwrap();
        let result = find_symbol_position(&file_path, "myFunc");
        assert!(
            result.is_ok(),
            "find_symbol_position should succeed: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_find_symbol_position_not_found_is_an_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.ts");
        std::fs::write(&file_path, "function foo() {}").unwrap();
        let result = find_symbol_position(&file_path, "nonexistent");
        assert!(result.is_err(), "a missing symbol has no position");
    }

    #[test]
    fn test_find_symbol_position_file_not_found() {
        let result = find_symbol_position(Path::new("/nonexistent/file.ts"), "foo");
        assert!(result.is_err());
    }

    // === validate_file_path ===

    #[test]
    fn rejects_path_traversal() {
        let path = PathBuf::from("../../../etc/passwd");
        let result = validate_file_path(&path, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }

    #[test]
    fn rejects_nonexistent_file() {
        let path = PathBuf::from("/nonexistent/file.rs");
        let err_msg = validate_file_path(&path, None).unwrap_err().to_string();
        assert!(
            err_msg.contains("not found") || err_msg.contains("inaccessible"),
            "Expected 'not found' or 'inaccessible', got: {}",
            err_msg
        );
    }

    #[test]
    fn accepts_valid_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();
        assert!(validate_file_path(&file_path, None).is_ok());
    }

    #[test]
    fn rejects_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = validate_file_path(temp_dir.path(), None);
        assert!(result.unwrap_err().to_string().contains("not a file"));
    }

    #[test]
    fn workspace_constraint_rejects_outside() {
        let workspace = tempfile::tempdir().unwrap();
        let outside_workspace = tempfile::tempdir().unwrap();
        let file_outside = outside_workspace.path().join("test.rs");
        std::fs::write(&file_outside, "fn main() {}").unwrap();

        let result = validate_file_path(&file_outside, Some(workspace.path()));
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("outside workspace"),
            "Expected 'outside workspace' error"
        );
    }

    #[test]
    fn workspace_not_found() {
        let temp_file = tempfile::tempdir().unwrap();
        let file_path = temp_file.path().join("test.rs");
        std::fs::write(&file_path, "fn main() {}").unwrap();

        let result = validate_file_path(&file_path, Some(Path::new("/nonexistent/workspace")));
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("workspace not found"),
            "Expected 'workspace not found' error"
        );
    }

    // === repo-outline tests ===

    #[test]
    fn repo_outline_lists_files_with_line_counts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("hello.ts");
        std::fs::write(&file, "const x = 1;\nconst y = 2;\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].lines, 2);
        assert_eq!(result.files[0].language, "TypeScript");
    }

    #[test]
    fn repo_outline_skips_build_and_vendor_dirs() {
        let temp_dir = tempfile::tempdir().unwrap();
        for dir in ["src", "target/debug", "node_modules/pkg", ".git"] {
            std::fs::create_dir_all(temp_dir.path().join(dir)).unwrap();
        }
        std::fs::write(temp_dir.path().join("src/a.ts"), "const a = 1;\n").unwrap();
        std::fs::write(temp_dir.path().join("target/debug/b.rs"), "fn b() {}\n").unwrap();
        std::fs::write(temp_dir.path().join("node_modules/pkg/c.js"), "var c;\n").unwrap();
        std::fs::write(temp_dir.path().join(".git/d.json"), "{}\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        let paths: Vec<&str> = result.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, ["src/a.ts"]);
        assert!(!result.truncated);
    }

    #[test]
    fn repo_outline_says_when_max_files_cut_the_list() {
        let temp_dir = tempfile::tempdir().unwrap();
        for i in 0..3 {
            std::fs::write(temp_dir.path().join(format!("f{i}.ts")), "const x = 1;\n").unwrap();
        }
        let outline = |max_files| {
            execute_repo_outline(
                temp_dir.path(),
                temp_dir.path(),
                None,
                None,
                None,
                false,
                max_files,
                false,
            )
            .unwrap()
        };
        assert!(outline(2).truncated);
        assert!(!outline(3).truncated);
    }

    #[test]
    fn repo_outline_respects_max_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        for i in 0..10 {
            let file = temp_dir.path().join(format!("file{}.ts", i));
            std::fs::write(&file, "const x = 1;\n").unwrap();
        }

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            3,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 3);
        assert_eq!(result.total_files, 3);
    }

    #[test]
    fn repo_outline_skips_unknown_extensions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ts_file = temp_dir.path().join("hello.ts");
        std::fs::write(&ts_file, "const x = 1;\n").unwrap();
        let txt_file = temp_dir.path().join("readme.txt");
        std::fs::write(&txt_file, "just a readme\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "hello.ts");
    }

    #[test]
    fn repo_outline_respects_depth() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("top.ts");
        std::fs::write(&file, "const a = 1;\n").unwrap();
        let sub = temp_dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let deep_file = sub.join("deep.ts");
        std::fs::write(&deep_file, "const b = 2;\n").unwrap();

        // depth=1 means only the root directory itself (no subdirs)
        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            Some(1),
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "top.ts");
    }

    #[test]
    fn repo_outline_language_summary() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ts1 = temp_dir.path().join("a.ts");
        std::fs::write(&ts1, "const x = 1;\n").unwrap();
        let ts2 = temp_dir.path().join("b.ts");
        std::fs::write(&ts2, "const y = 2;\nconst z = 3;\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.languages.len(), 1);
        assert_eq!(result.languages[0].language, "TypeScript");
        assert_eq!(result.languages[0].files, 2);
        assert_eq!(result.languages[0].lines, 3);
    }

    #[test]
    fn repo_outline_include_filter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ts = temp_dir.path().join("main.ts");
        std::fs::write(&ts, "const a = 1;\n").unwrap();
        let rs = temp_dir.path().join("lib.rs");
        std::fs::write(&rs, "fn main() {}\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            Some("*.ts"),
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "main.ts");
    }

    #[test]
    fn repo_outline_exclude_filter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ts = temp_dir.path().join("main.ts");
        std::fs::write(&ts, "const a = 1;\n").unwrap();
        let rs = temp_dir.path().join("lib.rs");
        std::fs::write(&rs, "fn main() {}\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            Some("*.rs"),
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "main.ts");
    }

    #[test]
    fn repo_outline_with_symbols() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("hello.ts");
        std::fs::write(&file, "function greet() {}\nclass Foo {}\n").unwrap();

        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            true,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 1);
        let symbols = result.files[0].symbols.as_ref().unwrap();
        assert!(
            symbols.contains(&"greet".to_string()),
            "should contain 'greet', got {:?}",
            symbols
        );
        assert!(
            symbols.contains(&"Foo".to_string()),
            "should contain 'Foo', got {:?}",
            symbols
        );
    }

    #[test]
    fn repo_outline_empty_directory() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = execute_repo_outline(
            temp_dir.path(),
            temp_dir.path(),
            None,
            None,
            None,
            false,
            500,
            false,
        )
        .unwrap();
        assert_eq!(result.files.len(), 0);
        assert_eq!(result.total_files, 0);
        assert!(result.languages.is_empty());
    }

    #[test]
    fn test_repo_outline_subtree_workspace_relative_paths() {
        // relative=false: paths are workspace-relative even when traversal is scoped
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("lib.rs"), "pub fn foo() {}").unwrap();

        let subtree = root.join("sub");
        let result =
            execute_repo_outline(root, &subtree, None, None, None, false, 500, false).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "sub/lib.rs"); // workspace-relative
        assert_eq!(result.root, subtree.to_string_lossy()); // root reflects traversal root
    }

    #[test]
    fn test_repo_outline_subtree_relative_paths() {
        // relative=true: paths are relative to the subtree
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("lib.rs"), "pub fn foo() {}").unwrap();

        let subtree = root.join("sub");
        let result =
            execute_repo_outline(root, &subtree, None, None, None, false, 500, true).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.files[0].path, "lib.rs"); // subtree-relative
    }

    #[test]
    fn test_repo_outline_subtree_excludes_sibling_dirs() {
        // Only files under the subtree are returned; siblings are excluded
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("lib.rs"), "pub fn foo() {}").unwrap();

        let subtree = root.join("sub");
        let result =
            execute_repo_outline(root, &subtree, None, None, None, false, 500, false).unwrap();

        assert!(result.files.iter().all(|f| !f.path.contains("main.rs")));
        assert_eq!(result.files.len(), 1);
    }

    // === symbols-batch tests ===

    #[test]
    fn symbols_batch_by_files_returns_grouped_results() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_a = temp_dir.path().join("a.ts");
        let file_b = temp_dir.path().join("b.ts");
        std::fs::write(&file_a, "function foo() {}").unwrap();
        std::fs::write(&file_b, "class Bar {}").unwrap();

        let result =
            execute_symbols_batch_by_files(&[file_a, file_b], None, temp_dir.path()).unwrap();
        assert_eq!(result.results.len(), 2);
        assert!(
            result.total_symbols >= 2,
            "expected at least 2 symbols, got {}",
            result.total_symbols
        );
    }

    #[test]
    fn symbols_batch_with_kind_filter() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("mixed.ts");
        std::fs::write(&file, "function foo() {}\nclass Bar {}\nconst x = 1;").unwrap();

        let result =
            execute_symbols_batch_by_files(&[file], Some("function"), temp_dir.path()).unwrap();
        for group in &result.results {
            for sym in &group.symbols {
                assert_eq!(
                    sym.kind, "function",
                    "kind filter should only include functions"
                );
            }
        }
    }

    #[test]
    fn symbols_batch_by_files_generates_stable_ids() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("test.ts");
        std::fs::write(&file, "function hello() {}").unwrap();

        let result = execute_symbols_batch_by_files(&[file], None, temp_dir.path()).unwrap();
        assert_eq!(result.results.len(), 1);
        assert!(!result.results[0].symbols.is_empty());
        let sym = &result.results[0].symbols[0];
        let id = sym.id.as_ref().expect("symbol should have a stable ID");
        assert!(
            act_lite_parser::SymbolId::parse(id).is_some(),
            "id '{}' should be parseable as SymbolId",
            id
        );
    }

    #[test]
    fn symbols_batch_by_files_empty_list() {
        let temp_dir = tempfile::tempdir().unwrap();
        let result = execute_symbols_batch_by_files(&[], None, temp_dir.path()).unwrap();
        assert_eq!(result.results.len(), 0);
        assert_eq!(result.total_symbols, 0);
    }

    #[test]
    fn symbols_batch_by_files_nonexistent_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let bad_file = temp_dir.path().join("nonexistent.ts");
        let result = execute_symbols_batch_by_files(&[bad_file], None, temp_dir.path());
        assert!(result.is_err(), "nonexistent file should produce error");
    }

    #[test]
    fn symbols_batch_by_ids_finds_matching_symbols() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("test.ts");
        std::fs::write(&file, "function hello() {}\nclass World {}").unwrap();

        let ids = vec!["test.ts::hello#function".to_string()];
        let result = execute_symbols_batch_by_ids(&ids, temp_dir.path()).unwrap();
        assert!(!result.results.is_empty());
        assert!(result.total_symbols >= 1);
        let sym = &result.results[0].symbols[0];
        assert_eq!(sym.name, "hello");
    }

    #[test]
    fn symbols_batch_by_ids_invalid_id_skipped() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ids = vec!["invalid_no_separator".to_string()];
        let result = execute_symbols_batch_by_ids(&ids, temp_dir.path()).unwrap();
        assert_eq!(result.results.len(), 0);
        assert_eq!(result.total_symbols, 0);
    }

    #[tokio::test]
    async fn symbols_batch_requires_files_or_ids() {
        let temp_dir = tempfile::tempdir().unwrap();
        let args = QueryArgs {
            query_type: QueryType::SymbolsBatch {
                files: None,
                ids: None,
                kinds: None,
            },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("--files") || err_msg.contains("--ids"),
            "should mention --files or --ids: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn symbols_batch_dispatch_with_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("test.ts");
        std::fs::write(&file, "function greet() {}").unwrap();

        let args = QueryArgs {
            query_type: QueryType::SymbolsBatch {
                files: Some("test.ts".to_string()),
                ids: None,
                kinds: Some("function".to_string()),
            },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        assert!(
            result.is_ok(),
            "symbols-batch with --files should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn symbols_batch_dispatch_with_ids() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("test.ts");
        std::fs::write(&file, "function hello() {}\nclass World {}").unwrap();

        let args = QueryArgs {
            query_type: QueryType::SymbolsBatch {
                files: None,
                ids: Some("test.ts::hello#function".to_string()),
                kinds: None,
            },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        assert!(
            result.is_ok(),
            "symbols-batch with --ids should succeed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_repo_outline_query_execute() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file = temp_dir.path().join("test.ts");
        std::fs::write(&file, "const x = 1;").unwrap();

        let args = QueryArgs {
            query_type: QueryType::RepoOutline {
                depth: Some(3),
                include: Some("*.ts".to_string()),
                exclude: Some("*.test.ts".to_string()),
                symbols: true,
                max_files: 50,
            },
        };
        let result = execute(&args, OutputFormat::Json, Some(temp_dir.path()), false).await;
        assert!(
            result.is_ok(),
            "repo-outline with filters should succeed: {:?}",
            result.err()
        );
    }

    // === simple_glob_match tests ===

    #[test]
    fn test_simple_glob_match_extension() {
        assert!(simple_glob_match("*.ts", "foo.ts"));
        assert!(simple_glob_match("*.ts", "src/foo.ts"));
        assert!(!simple_glob_match("*.ts", "foo.rs"));
    }

    #[test]
    fn test_simple_glob_match_double_star() {
        assert!(simple_glob_match("src/**/*.ts", "src/foo/bar.ts"));
        assert!(simple_glob_match("src/**/*.ts", "src/bar.ts"));
    }

    #[test]
    fn test_simple_glob_match_question_mark() {
        assert!(simple_glob_match("fo?.ts", "foo.ts"));
        assert!(!simple_glob_match("fo?.ts", "fooo.ts"));
    }

    #[test]
    fn test_simple_glob_match_exact() {
        assert!(simple_glob_match("main.ts", "main.ts"));
        assert!(!simple_glob_match("main.ts", "service.ts"));
    }

    #[test]
    fn test_glob_match_recursive_no_match_at_end() {
        // Pattern fully consumed but text remains
        assert!(!simple_glob_match("abc", "abcd"));
    }

    #[test]
    fn test_glob_match_star_stops_at_slash() {
        // Single * should not match across /
        assert!(!simple_glob_match("src/*.ts", "src/sub/foo.ts"));
        assert!(simple_glob_match("src/*.ts", "src/foo.ts"));
    }

    // === LSP references attribution helper ===

    #[test]
    fn test_lsp_references_result_empty_answer_is_attributed_lsp() {
        // An LSP `Ok` with zero ranges is a real answer and must carry
        // `source: lsp`, not fall back to the tree-sitter default.
        let result = lsp_references_result(Vec::new());
        let QueryResult::References {
            references,
            skipped_files,
            source,
        } = &result
        else {
            panic!("expected References result");
        };
        assert!(references.is_empty());
        assert!(skipped_files.is_empty());
        assert_eq!(*source, ReferenceSource::Lsp);
    }

    #[test]
    fn test_lsp_references_result_maps_ranges_to_start_locations() {
        let file = PathBuf::from("a.ts");
        let ranges = vec![
            act_lite_core::Range::new(
                Location::new(file.clone(), 3, 5),
                Location::new(file.clone(), 3, 12),
            ),
            act_lite_core::Range::new(Location::new(file.clone(), 9, 1), Location::new(file, 9, 8)),
        ];
        let result = lsp_references_result(ranges);
        let QueryResult::References {
            references, source, ..
        } = &result
        else {
            panic!("expected References result");
        };
        assert_eq!(*source, ReferenceSource::Lsp);
        let positions: Vec<(u32, u32)> = references
            .iter()
            .map(|location| (location.line(), location.column()))
            .collect();
        assert_eq!(positions, [(3, 5), (9, 1)]);
    }

    // === symbols answer parse-state indicator ===

    #[test]
    fn with_parse_state_flags_invalid_ts() {
        let source = act_lite_parser::SourceFile::from_content(
            "test.ts",
            "struct PlainStruct { size: number }\nfunction usePlain(): void {}\n".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        // Precondition: the fixture must genuinely parse with ERROR nodes,
        // or this test proves nothing.
        assert!(
            source.tree().root_node().has_error(),
            "fixture must contain a real parse error, or this test is vacuous"
        );

        let result = act_lite_parser::execute_symbols(&source).unwrap();
        let value = with_parse_state(&result, &source);
        assert_eq!(value["parse_error"], true);
    }

    #[test]
    fn with_parse_state_clean_file_omits_key() {
        let source = act_lite_parser::SourceFile::from_content(
            "test.ts",
            "function usePlain(): void {}\n".to_string(),
            Grammar::TypeScript,
        )
        .unwrap();
        assert!(
            !source.tree().root_node().has_error(),
            "fixture must parse cleanly, or this test is vacuous"
        );

        let result = act_lite_parser::execute_symbols(&source).unwrap();
        let value = with_parse_state(&result, &source);
        assert!(
            value.get("parse_error").is_none(),
            "clean parse must omit the key entirely, not serialize it false: {value:?}"
        );
    }
}
