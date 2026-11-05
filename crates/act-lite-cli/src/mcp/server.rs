//! The act-lite MCP server: eight navigation tools over a tree-sitter parse,
//! with a language server for `references` and `callers` when one is
//! available.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use act_lite_lsp::{LspPool, LspServer};
use act_lite_parser::{Grammar, QueryResult};
use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use tokio_util::sync::CancellationToken;

use super::catalog::TOOLS;
use super::params::*;
use super::path_error::enrich_not_found_error;
use crate::analyze::clones::ClonesAnalyzer;
use crate::analyze::envelope::AnalyzeError;
use crate::analyze::hotspots::HotspotsAnalyzer;
use crate::analyze::patterns::PatternsAnalyzer;
use crate::analyze::run::{Analyzer, Request, run};
use crate::definition;

/// MCP server exposing act-lite's navigation queries.
#[derive(Clone)]
pub struct ActLiteServer {
    tool_router: ToolRouter<Self>,
    root: PathBuf,
    lsp_pool: Arc<LspPool>,
    enable_lsp: bool,
}

impl ActLiteServer {
    /// A server for the workspace that contains the current directory: the
    /// nearest ancestor holding `.git`, else the current directory.
    pub fn new(enable_lsp: bool) -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let root = cwd
            .ancestors()
            .find(|dir| dir.join(".git").exists())
            .unwrap_or(&cwd)
            .to_path_buf();
        Self::with_root(&root, enable_lsp)
    }

    /// A server for the workspace at `root`. The root is canonicalized when
    /// it exists: language servers reject relative file URIs, and
    /// workspace-relative output needs one spelling of the root.
    pub fn with_root(root: &Path, enable_lsp: bool) -> Self {
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let lsp_pool = if enable_lsp {
            LspPool::new(root.clone())
        } else {
            LspPool::disabled()
        };
        Self {
            tool_router: Self::tool_router(),
            root,
            lsp_pool: Arc::new(lsp_pool),
            enable_lsp,
        }
    }

    /// The workspace root the server answers for.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Stops every language server the session started.
    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.lsp_pool
            .shutdown_all()
            .await
            .map_err(|e| anyhow::anyhow!("failed to stop language servers: {e}"))
    }

    /// Every `#[tool]` route, described from the catalog. A route with no
    /// catalog entry fails construction here.
    fn tool_router() -> ToolRouter<Self> {
        let mut router = Self::uncatalogued_tool_router();
        router += Self::analyzer_tool_router();
        #[cfg(debug_assertions)]
        {
            router += Self::fixture_tool_router();
        }
        for route in router.map.values_mut() {
            let entry = TOOLS
                .iter()
                .find(|e| e.name == route.attr.name)
                .unwrap_or_else(|| {
                    panic!(
                        "#[tool] `{}` has no ToolEntry in mcp/catalog.rs, the one source of its description; add one",
                        route.attr.name
                    )
                });
            route.attr.description = Some(std::borrow::Cow::Borrowed(entry.description));
        }
        router
    }

    /// Runs `analyzer` on the request `params` describe, on a blocking thread
    /// so that the serve loop keeps reading messages. `ct` is the request's
    /// cancellation token. A request error is `invalid_params`, a failure
    /// `internal_error`. The first content block is the result JSON, the
    /// second the hint.
    async fn run_analyzer<A: Analyzer + 'static>(
        &self,
        analyzer: A,
        params: AnalyzeParams,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        self.run_analyzer_with(move |_| Ok(analyzer), params, ct)
            .await
    }

    /// As [`Self::run_analyzer`], with the analyzer built from the checked
    /// request, so that its own parameters are checked after the shared ones.
    async fn run_analyzer_with<A: Analyzer + 'static>(
        &self,
        make: impl FnOnce(&Request) -> Result<A, AnalyzeError> + Send + 'static,
        params: AnalyzeParams,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        let root = self.root.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            let request = params.request(&root)?;
            let analyzer = make(&request)?;
            let envelope = run(&analyzer, &request, &ct)?;
            let json = serde_json::to_string(&envelope).map_err(|e| AnalyzeError::Failed {
                analysis: A::NAME,
                cause: e.to_string(),
            })?;
            let hint = analyze_hint(
                envelope.total,
                envelope.truncated,
                &envelope.excluded,
                &envelope.incomplete,
                A::HINT,
            );
            Ok::<_, AnalyzeError>((json, hint))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("{} failed: {e}", A::NAME), None))?;
        let (json, hint) = outcome.map_err(|e| {
            if e.is_invalid_params() {
                McpError::invalid_params(e.to_string(), None)
            } else {
                McpError::internal_error(e.to_string(), None)
            }
        })?;
        Ok(success_with_hint(json, &hint))
    }

    /// Resolves a tool's `file` argument to an existing single source file in
    /// a supported language, relative paths against the workspace root.
    fn source_path(&self, file: &str) -> Result<(PathBuf, Grammar), McpError> {
        if file.trim().is_empty() {
            return Err(McpError::invalid_params(
                "file is required and must be a path to a single source file, not empty",
                None,
            ));
        }
        let path = PathBuf::from(file);
        let resolved = if path.is_absolute() {
            path
        } else {
            self.root.join(path)
        };
        if !self.is_inside_root(&resolved) {
            return Err(McpError::invalid_params(
                "path must be inside the workspace root",
                None,
            ));
        }
        if !resolved.exists() {
            return Err(McpError::invalid_params(
                enrich_not_found_error(file, &resolved, &self.root),
                None,
            ));
        }
        if resolved.is_dir() {
            return Err(McpError::invalid_params(
                format!(
                    "'{}' is a directory, and a single source file is required. Use `repo_outline` to explore directories.",
                    resolved.display()
                ),
                None,
            ));
        }
        let grammar = Grammar::from_path(&resolved).ok_or_else(|| {
            McpError::invalid_params(
                crate::query::unsupported_language_error(&resolved).to_string(),
                None,
            )
        })?;
        Ok((resolved, grammar))
    }

    /// Whether `path`, with symlinks and `..` segments resolved, lies inside
    /// the workspace root. A missing path is judged by its nearest existing
    /// ancestor, and refused when the missing rest holds a `..` segment.
    fn is_inside_root(&self, path: &Path) -> bool {
        let mut existing = path;
        loop {
            if let Ok(canonical) = existing.canonicalize() {
                let rest = path.strip_prefix(existing).unwrap_or(path);
                return canonical.starts_with(&self.root)
                    && !rest
                        .components()
                        .any(|c| matches!(c, std::path::Component::ParentDir));
            }
            match existing.parent() {
                Some(parent) => existing = parent,
                None => return false,
            }
        }
    }

    fn load(path: &Path, file: &str) -> Result<act_lite_parser::SourceFile, McpError> {
        act_lite_parser::SourceFile::load(path).map_err(|e| {
            McpError::invalid_params(
                format!(
                    "Failed to load file '{}' (resolved: '{}'): {}",
                    file,
                    path.display(),
                    e
                ),
                None,
            )
        })
    }

    /// Resolves `symbol` to its 1-indexed position in `source`, or an error
    /// that points the caller at `symbols`.
    fn symbol_position(
        source: &act_lite_parser::SourceFile,
        symbol: &str,
        file: &str,
    ) -> Result<(u32, u32), McpError> {
        resolve_symbol_position(source, symbol).ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "Symbol '{symbol}' not found in {file}. Use `symbols` to list the file's symbols."
                ),
                None,
            )
        })
    }
}

#[tool_router(router = uncatalogued_tool_router)]
impl ActLiteServer {
    #[tool]
    async fn status(&self) -> Result<CallToolResult, McpError> {
        let languages: Vec<&str> = Grammar::all().iter().map(|g| g.name()).collect();
        let mut lsp = serde_json::Map::new();
        for server in LspServer::ALL {
            let ready = self.enable_lsp
                && act_lite_lsp::installer::is_installed(&server.config(self.root.clone()));
            lsp.insert(
                server.binary_name().to_string(),
                serde_json::Value::from(if ready { "ready" } else { "not_installed" }),
            );
        }
        let response = serde_json::json!({
            "name": "act-lite",
            "version": env!("CARGO_PKG_VERSION"),
            "root": self.root.display().to_string(),
            "languages": languages,
            "lsp": lsp,
        });
        Ok(CallToolResult::success(vec![ContentBlock::text(
            response.to_string(),
        )]))
    }

    #[tool]
    async fn skeleton(
        &self,
        params: Parameters<SkeletonParams>,
    ) -> Result<CallToolResult, McpError> {
        let (path, _) = self.source_path(&params.0.file)?;
        let source = Self::load(&path, &params.0.file)?;
        let result = act_lite_parser::execute_skeleton(&source).map_err(|e| {
            McpError::internal_error(format!("Failed to execute skeleton query: {}", e), None)
        })?;
        let json = serde_json::to_string(&compact_skeleton(&result))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(success_with_hint(
            json,
            "Use `definition` to jump to any symbol's source. Use `references` to find its usages.",
        ))
    }

    #[tool]
    async fn symbols(&self, params: Parameters<SymbolsParams>) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let (path, _) = self.source_path(&p.file)?;
        let source = Self::load(&path, &p.file)?;
        let result = act_lite_parser::execute_symbols(&source).map_err(|e| {
            McpError::internal_error(format!("Failed to execute symbols query: {}", e), None)
        })?;
        let symbols = if let QueryResult::Symbols { ref symbols } = result {
            symbols.as_slice()
        } else {
            &[]
        };

        // Kind filter: case-insensitive against the Debug form of NodeKind.
        let kind_filtered: Vec<_> = match &p.kind {
            Some(kinds) => symbols
                .iter()
                .filter(|s| {
                    let kind_str = format!("{:?}", s.kind());
                    kinds.iter().any(|k| kind_str.eq_ignore_ascii_case(k))
                })
                .collect(),
            None => symbols.iter().collect(),
        };

        // Pattern filter: case-insensitive regex on the name.
        let filtered: Vec<_> = match &p.pattern {
            Some(pat) => {
                let re = regex::RegexBuilder::new(pat)
                    .case_insensitive(true)
                    .build()
                    .map_err(|e| {
                        McpError::invalid_params(
                            format!("Invalid pattern regex '{}': {}", pat, e),
                            None,
                        )
                    })?;
                kind_filtered
                    .into_iter()
                    .filter(|s| re.is_match(s.name()))
                    .collect()
            }
            None => kind_filtered,
        };

        let total = filtered.len();
        let limit = p.limit as usize;
        let items: Vec<_> = filtered[..total.min(limit)]
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name(),
                    "kind": format!("{:?}", s.kind()),
                    "line": s.range().start().line(),
                    "end_line": s.range().end().line(),
                })
            })
            .collect();
        let compact = symbols_success_json(items, source.tree().root_node().has_error());
        let mut json = serde_json::to_string(&compact)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        if total > limit {
            json.push_str(&format!(
                "\nNote: Returning {} of {} symbols. Use `pattern` (name regex) or `kind` (e.g. [\"function\",\"class\"]) to filter.",
                limit, total
            ));
        }
        Ok(success_with_hint(
            json,
            "Use `references` to find all usages of a symbol. Use `definition` to jump to one.",
        ))
    }

    #[tool]
    async fn symbols_batch(
        &self,
        params: Parameters<SymbolsBatchParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let json = if let Some(files) = &p.files {
            let mut path_errors: Vec<serde_json::Value> = Vec::new();
            let file_paths: Vec<PathBuf> = files
                .iter()
                .filter_map(|f| {
                    let path = self.root.join(f);
                    let inside = self.is_inside_root(&path);
                    if inside && path.is_file() && Grammar::from_path(&path).is_some() {
                        Some(path)
                    } else {
                        let msg = if !inside {
                            "path must be inside the workspace root".to_string()
                        } else if path.is_file() {
                            crate::query::unsupported_language_error(&path).to_string()
                        } else if path.is_dir() {
                            format!("'{f}' is a directory, and a single source file is required")
                        } else {
                            enrich_not_found_error(f, &path, &self.root)
                        };
                        path_errors.push(serde_json::json!({ "file": f, "error": msg }));
                        None
                    }
                })
                .collect();
            let kinds = p.kinds.as_ref().map(|k| k.join(","));
            let result = crate::query::execute_symbols_batch_by_files(
                &file_paths,
                kinds.as_deref(),
                &self.root,
            )
            .map_err(|e| McpError::invalid_params(format!("{e:#}"), None))?;
            let mut value = serde_json::to_value(&result)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            // Name filter, as single-file `symbols` takes.
            if let Some(pattern) = &p.pattern {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| McpError::invalid_params(format!("invalid pattern: {e}"), None))?;
                if let Some(groups) = value.get_mut("results").and_then(|f| f.as_array_mut()) {
                    for group in groups.iter_mut() {
                        if let Some(syms) = group.get_mut("symbols").and_then(|s| s.as_array_mut())
                        {
                            syms.retain(|sym| {
                                sym.get("name")
                                    .and_then(|n| n.as_str())
                                    .is_some_and(|n| re.is_match(n))
                            });
                        }
                    }
                    let listed: usize = groups
                        .iter()
                        .filter_map(|g| g.get("symbols").and_then(|s| s.as_array()))
                        .map(Vec::len)
                        .sum();
                    value["total_symbols"] = listed.into();
                }
            }
            if !path_errors.is_empty()
                && let Some(obj) = value.as_object_mut()
            {
                obj.insert(
                    "path_errors".to_string(),
                    serde_json::Value::Array(path_errors),
                );
            }
            serde_json::to_string(&value)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else if let Some(ids) = &p.ids {
            let outside = ids.iter().any(|id| {
                act_lite_parser::SymbolId::parse(id)
                    .is_some_and(|parsed| !self.is_inside_root(&self.root.join(parsed.file())))
            });
            if outside {
                return Err(McpError::invalid_params(
                    "path must be inside the workspace root",
                    None,
                ));
            }
            let result = crate::query::execute_symbols_batch_by_ids(ids, &self.root)
                .map_err(|e| McpError::invalid_params(format!("{e:#}"), None))?;
            serde_json::to_string(&result)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?
        } else {
            return Err(McpError::invalid_params(
                "Either 'files' or 'ids' must be provided",
                None,
            ));
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
    }

    #[tool]
    async fn repo_outline(
        &self,
        params: Parameters<RepoOutlineParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let traversal_root = match &p.path {
            Some(rel) => {
                let candidate = self.root.join(rel);
                // Canonicalize both to guard against symlinks and `..` segments.
                let canonical_ws = std::fs::canonicalize(&self.root).map_err(|e| {
                    McpError::internal_error(
                        format!(
                            "workspace root {} is not readable: {e}",
                            self.root.display()
                        ),
                        None,
                    )
                })?;
                let canonical_candidate = std::fs::canonicalize(&candidate).map_err(|_| {
                    McpError::invalid_params(
                        enrich_not_found_error(rel, &candidate, &self.root),
                        None,
                    )
                })?;
                if !canonical_candidate.starts_with(&canonical_ws) {
                    return Err(McpError::invalid_params(
                        "path must be inside the workspace root",
                        None,
                    ));
                }
                if !canonical_candidate.is_dir() {
                    return Err(McpError::invalid_params(
                        format!("path is not a directory: {}", rel),
                        None,
                    ));
                }
                canonical_candidate
            }
            None => self.root.clone(),
        };
        let result = crate::query::execute_repo_outline(
            &self.root,
            &traversal_root,
            p.depth,
            p.include.as_deref(),
            p.exclude.as_deref(),
            p.symbols,
            p.max_files.unwrap_or(500),
            p.relative,
        )
        .map_err(|e| McpError::internal_error(format!("{e:#}"), None))?;
        let json = serde_json::to_string(&result)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(success_with_hint(
            json,
            "Use `skeleton <file>` to see any file's API, or `symbols <file>` for its definitions.",
        ))
    }

    #[tool]
    async fn references(
        &self,
        params: Parameters<ReferencesParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let (path, grammar) = self.source_path(&p.file)?;
        let source = Self::load(&path, &p.file)?;
        // Validate the symbol before branching on the language server, so the
        // caller gets the same "not found" error either way.
        let position = Self::symbol_position(&source, &p.symbol, &p.file)?;
        let location = act_lite_core::Location::new(path.clone(), position.0, position.1);

        let mut lsp_error = None;
        if let Some(client) = self.lsp_pool.get_query_client(grammar).await {
            match act_lite_lsp::query_references(client.as_ref(), &path, &location).await {
                Ok(refs) => {
                    let response = serde_json::json!({
                        "symbol": p.symbol,
                        "file": p.file,
                        "resolved_by": lsp_resolved_by(grammar),
                        "references": refs.ranges.iter().map(|range| serde_json::json!({
                            "file": range.start().file().display().to_string(),
                            "line": range.start().line(),
                        })).collect::<Vec<_>>(),
                    });
                    let json = serde_json::to_string(&response)
                        .map_err(|e| McpError::internal_error(e.to_string(), None))?;
                    return Ok(success_with_hint(
                        json,
                        "Use `callers` to see only call sites.",
                    ));
                }
                Err(e) => lsp_error = Some(e.to_string()),
            }
        }

        // No language server answer: fall back to the name-level tree-sitter
        // baseline over same-grammar files, and say so.
        let walk = act_lite_parser::walk_reference_siblings(
            &self.root,
            &path,
            grammar,
            act_lite_parser::MAX_REFERENCE_WALK_FILES,
            act_lite_parser::MAX_REFERENCE_WALK_FILE_BYTES,
        )
        .map_err(|e| {
            McpError::internal_error(
                format!("failed to walk the workspace for references: {e}"),
                None,
            )
        })?;
        let result = act_lite_parser::execute_references_for_symbol_across(
            &source,
            &walk.siblings,
            &p.symbol,
            position.0,
            position.1,
        )
        .map_err(|e| {
            McpError::internal_error(
                format!("failed to find references for '{}': {e}", p.symbol),
                None,
            )
        })?;
        let references = match result {
            QueryResult::References { references, .. } => references,
            _ => Vec::new(),
        };
        let why = match &lsp_error {
            Some(e) => format!("the {} server failed: {e}", lsp_server_name(grammar)),
            None if !self.enable_lsp => "language servers are disabled".to_string(),
            None => format!("no {} server", lsp_server_name(grammar)),
        };
        let mut response = serde_json::json!({
            "symbol": p.symbol,
            "file": p.file,
            "resolved_by": format!("tree-sitter workspace baseline (name-level; {why})"),
            "references": references.iter().map(|r| serde_json::json!({
                "file": r.file().display().to_string(),
                "line": r.line(),
            })).collect::<Vec<_>>(),
        });
        if !walk.skipped.is_empty()
            && let Some(obj) = response.as_object_mut()
        {
            obj.insert(
                "skipped_files".to_string(),
                serde_json::to_value(&walk.skipped)
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?,
            );
        }
        let json = serde_json::to_string(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        let note = "Name-level baseline — a same-name symbol in an unrelated scope can appear.";
        let install = if self.enable_lsp {
            crate::query::lsp_install_hint_once(grammar, &self.lsp_pool)
        } else {
            None
        };
        Ok(match install {
            Some(install) => success_with_hint(json, &format!("{note} {install}")),
            None => success_with_hint(json, note),
        })
    }

    #[tool]
    async fn callers(&self, params: Parameters<CallersParams>) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let (path, grammar) = self.source_path(&p.file)?;
        let source = Self::load(&path, &p.file)?;
        let position = Self::symbol_position(&source, &p.symbol, &p.file)?;
        let location = act_lite_core::Location::new(path.clone(), position.0, position.1);

        if !self.enable_lsp {
            return Err(McpError::invalid_request(
                "callers needs a language server, and language servers are disabled (--disable-lsp or ACT_LITE_DISABLE_LSP)",
                None,
            ));
        }
        let client = self
            .lsp_pool
            .get_query_client(grammar)
            .await
            .ok_or_else(|| {
                McpError::invalid_request(
                    crate::query::lsp_unavailable_error(grammar, &self.root, "callers").to_string(),
                    None,
                )
            })?;
        let callers = act_lite_lsp::query_callers(client.as_ref(), &path, &location)
            .await
            .map_err(|e| match e {
                // Nothing was sent: the server does not offer the operation,
                // a request the tool cannot serve, as when none is available.
                act_lite_lsp::LspError::NotAdvertised { .. } => {
                    McpError::invalid_request(e.to_string(), None)
                }
                e => McpError::internal_error(
                    format!(
                        "the {} server failed to answer callers of '{}': {e}",
                        lsp_server_name(grammar),
                        p.symbol
                    ),
                    None,
                ),
            })?;
        let response = serde_json::json!({
            "symbol": p.symbol,
            "resolved_by": lsp_resolved_by(grammar),
            "callers": callers.callers.iter().map(|sym| serde_json::json!({
                "name": sym.name(),
                "kind": format!("{:?}", sym.kind()),
                "file": sym.file().display().to_string(),
                "line": sym.range().start().line(),
            })).collect::<Vec<_>>(),
        });
        let json = serde_json::to_string(&response)
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![ContentBlock::text(json)]))
    }

    #[tool]
    async fn definition(
        &self,
        params: Parameters<DefinitionParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = &params.0;
        let (path, _grammar) = self.source_path(&p.file)?;
        let source = Self::load(&path, &p.file)?;

        let (symbol_name, best) =
            definition::resolve_definition(&source, &self.root, &p.symbol, p.line, p.column)
                .map_err(|e| match e {
                    definition::DefinitionError::NoSymbolAtPosition => McpError::invalid_params(
                        definition::no_symbol_text(Path::new(&p.file), p.line, p.column),
                        None,
                    ),
                    definition::DefinitionError::NotFound { symbol } => McpError::invalid_params(
                        definition::not_found_text(&symbol, Path::new(&p.file), p.line, p.column),
                        None,
                    ),
                })?;

        let json = serde_json::to_string(&definition::definition_json(&symbol_name, &best))
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(success_with_hint(
            json,
            "Use `references` to find all usages of this symbol. Use `callers` to inspect call sites.",
        ))
    }
}

/// The code-health analyzers' tools.
#[tool_router(router = analyzer_tool_router)]
impl ActLiteServer {
    #[tool]
    async fn analyze_patterns(
        &self,
        params: Parameters<PatternsParams>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        self.run_analyzer(PatternsAnalyzer, params.0.shared, ct)
            .await
    }

    #[tool]
    async fn analyze_hotspots(
        &self,
        params: Parameters<HotspotsParams>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        self.run_analyzer(HotspotsAnalyzer, params.0.shared, ct)
            .await
    }

    #[tool]
    async fn analyze_clones(
        &self,
        params: Parameters<ClonesParams>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        let ClonesParams { shared, focus } = params.0;
        self.run_analyzer_with(
            move |request| ClonesAnalyzer::new(request, &focus),
            shared,
            ct,
        )
        .await
    }
}

/// The test analyzer's tool, served only by builds with debug assertions.
#[cfg(debug_assertions)]
#[tool_router(router = fixture_tool_router)]
impl ActLiteServer {
    #[tool]
    async fn analyze_fixture(
        &self,
        params: Parameters<FixtureParams>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        self.run_analyzer(
            crate::analyze::fixture::FixtureAnalyzer,
            params.0.shared,
            ct,
        )
        .await
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ActLiteServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "act-lite",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(instructions())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// The server instructions, built from the parser, the language-server table
/// and the tool catalog so that none of them can drift from what is served.
fn instructions() -> String {
    let grammars = Grammar::all();
    let languages = grammars
        .iter()
        .map(|g| {
            let mark = if LspServer::for_grammar(*g).is_some() {
                "*"
            } else {
                ""
            };
            format!("{}{mark}", g.name())
        })
        .collect::<Vec<_>>()
        .join(", ");
    let tools = TOOLS
        .iter()
        .map(|e| format!("- {}: {}", e.name, tool_summary(e.description)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "act-lite does AST-aware code navigation across {count} languages. \
All tools accept absolute paths or paths relative to the workspace root.\n\
\n\
Languages (* marks a language with a language server):\n\
{languages}\n\
\n\
A marked language has a language server; `status` shows whether it is installed. \
When the server is installed and language servers are enabled, `references` and \
`callers` resolve through it. Otherwise, or when the server fails, `references` falls \
back to a name-level tree-sitter search and says so in `resolved_by`, and `callers` \
returns an error.\n\
\n\
Tools:\n\
{tools}",
        count = grammars.len()
    )
}

/// One line for a tool: the description before ` Params:`, cut at its first
/// sentence end (`. `), without a trailing `.`.
fn tool_summary(description: &str) -> &str {
    let before_params = description.split(" Params:").next().unwrap_or(description);
    let sentence = before_params.split(". ").next().unwrap_or(before_params);
    sentence.trim_end_matches('.')
}

/// The LSP server binary for a grammar, for messages.
fn lsp_server_name(grammar: Grammar) -> &'static str {
    LspServer::for_grammar(grammar).map_or("language", LspServer::binary_name)
}

/// Provenance marker for an LSP-backed answer: `lsp:<binary>`.
fn lsp_resolved_by(grammar: Grammar) -> String {
    format!("lsp:{}", lsp_server_name(grammar))
}

/// Resolves a bare symbol name to the position of its declaration: the
/// parser first, then a whole-identifier text scan for a symbol only used in
/// this file.
fn resolve_symbol_position(
    source: &act_lite_parser::SourceFile,
    symbol: &str,
) -> Option<(u32, u32)> {
    if let Ok(QueryResult::Symbols { symbols }) = act_lite_parser::execute_symbols(source)
        && let Some(found) = symbols.iter().find(|s| s.name() == symbol)
    {
        let start = found.range().start();
        return Some((start.line(), start.column()));
    }
    find_symbol_position(source.content(), symbol)
}

/// Finds the first whole-identifier occurrence of `symbol` (1-indexed line
/// and character column).
fn find_symbol_position(content: &str, symbol: &str) -> Option<(u32, u32)> {
    if symbol.is_empty() {
        return None;
    }
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    for (line_idx, line) in content.lines().enumerate() {
        let mut from = 0;
        while let Some(hit) = line[from..].find(symbol) {
            let start = from + hit;
            let end = start + symbol.len();
            let before_ok = line[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_ident(c));
            let after_ok = line[end..].chars().next().is_none_or(|c| !is_ident(c));
            if before_ok && after_ok {
                let col = line[..start].chars().count() as u32 + 1;
                return Some((line_idx as u32 + 1, col));
            }
            from = end;
        }
    }
    None
}

/// The `symbols` payload, with a top-level `"parse_error": true` when the
/// file's tree carries ERROR nodes; the key is absent on a clean parse.
fn symbols_success_json(items: Vec<serde_json::Value>, has_error: bool) -> serde_json::Value {
    let mut value = serde_json::json!({ "symbols": items });
    if has_error && let serde_json::Value::Object(ref mut map) = value {
        map.insert("parse_error".to_string(), serde_json::Value::Bool(true));
    }
    value
}
