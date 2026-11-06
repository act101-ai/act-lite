use crate::client::{DocumentOpen, DocumentReady, LspClientTrait};
use crate::error::{LspError, Result};
use crate::types::{lsp_range_to_range, path_to_uri};
use act_lite_core::{Location, Range, Symbol, SymbolKind};
use act_lite_parser::Grammar;
use lsp_types::{
    CallHierarchyIncomingCall, CallHierarchyIncomingCallsParams, CallHierarchyItem,
    CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    CodeActionContext, CodeActionParams, CodeActionResponse, GotoDefinitionResponse, HoverParams,
    ReferenceParams, RenameParams, TextDocumentIdentifier, TextDocumentPositionParams, Uri,
    WorkspaceEdit, WorkspaceSymbolParams,
    request::{
        CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare,
        CodeActionRequest, GotoImplementation, HoverRequest, References, Rename as RenameRequest,
        Request, WorkspaceSymbolRequest,
    },
};
use std::path::{Path, PathBuf};
use tracing::debug;

/// File edits returned by a code action: file path → list of (range, new_text) pairs.
pub type FileEdits = Vec<(PathBuf, Vec<(Range, String)>)>;

/// Result of a references query.
#[derive(Debug, Clone)]
pub struct ReferencesResult {
    pub ranges: Vec<Range>,
}

/// Result of a callers query.
#[derive(Debug, Clone)]
pub struct CallersResult {
    pub callers: Vec<Symbol>,
}

/// Result of a hover/type query.
#[derive(Debug, Clone)]
pub struct TypeInfo {
    pub content: String,
    pub range: Option<Range>,
}

/// Result of a rename preview.
#[derive(Debug, Clone)]
pub struct RenamePreview {
    pub changes: Vec<(PathBuf, Vec<Range>)>,
}

/// Severity level for a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Information,
    Hint,
}

/// A diagnostic message from the LSP server.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The range where the diagnostic applies.
    pub range: Range,
    /// The severity of the diagnostic.
    pub severity: DiagnosticSeverity,
    /// The diagnostic message.
    pub message: String,
    /// Optional source of the diagnostic (e.g., "typescript").
    pub source: Option<String>,
    /// Optional error code.
    pub code: Option<String>,
}

/// Result of querying code actions.
#[derive(Debug, Clone)]
pub struct CodeActionsResult {
    pub actions: Vec<CodeAction>,
}

/// A code action that can be applied.
#[derive(Debug, Clone)]
pub struct CodeAction {
    /// The title of the code action.
    pub title: String,
    /// The kind of code action (e.g., "quickfix", "refactor").
    pub kind: Option<String>,
    /// Whether this is the preferred action.
    pub is_preferred: bool,
    /// The workspace edit to apply: file → list of (range, new_text) pairs.
    pub edit: Option<FileEdits>,
}

/// How long to wait for a server to publish diagnostics after a `didOpen`
/// before querying anyway. Bounds the cost for servers that never publish.
const ANALYSIS_READY_TIMEOUT_MS: u64 = 2_000;

/// The LSP language id `didOpen` announces for `file`, by its grammar.
fn language_id(file: &Path) -> &'static str {
    match Grammar::from_path(file) {
        Some(Grammar::TypeScript | Grammar::Tsx) => "typescript",
        Some(Grammar::JavaScript | Grammar::Jsx) => "javascript",
        Some(Grammar::Rust) => "rust",
        Some(Grammar::Python) => "python",
        Some(Grammar::Go) => "go",
        Some(Grammar::Swift) => "swift",
        Some(Grammar::Php) => "php",
        Some(Grammar::C) => "c",
        Some(Grammar::Cpp) => "cpp",
        Some(Grammar::CSharp) => "csharp",
        Some(Grammar::Dart) => "dart",
        Some(Grammar::Elixir) => "elixir",
        Some(Grammar::Java) => "java",
        Some(Grammar::Kotlin) => "kotlin",
        Some(Grammar::Ruby) => "ruby",
        Some(Grammar::Scala) => "scala",
        Some(Grammar::Sql) => "sql",
        _ => "plaintext",
    }
}

/// Tells the server that `file`'s content is now `text` (full-document
/// sync), after the caller rewrote it on disk.
///
/// `did_open` is idempotent per document, so a document the server already
/// has would otherwise keep the text it was first opened with, and every
/// later query would be answered against that stale text. A document not
/// yet open is opened with `text` through [`open_document`]'s own path —
/// the first-open readiness wait and its recorded outcome — so the open is
/// indistinguishable from one `open_document` made; an open one gets a
/// `didChange` when the server was last sent other text. Returns the document's readiness exactly as `open_document`
/// would report it, which a caller about to query must honour.
pub async fn sync_document(
    client: &impl LspClientTrait,
    file: &Path,
    text: String,
) -> Result<DocumentReady> {
    let (opened, ready) = open_with_text(client, file, text.clone()).await?;
    if opened == DocumentOpen::Opened {
        return Ok(ready);
    }
    client.change_text(&path_to_uri(file)?, &text).await?;
    Ok(ready)
}

/// Opens a document in the LSP server.
///
/// This must be called before querying for references or other operations
/// that require the LSP to know about the document.
pub async fn open_document(client: &impl LspClientTrait, file: &Path) -> Result<DocumentReady> {
    // Read the file content; a FIFO is not waited on.
    let content = act_lite_parser::read_regular_text(file).map_err(|e| LspError::IoError {
        operation: "read file for LSP".to_string(),
        path: file.to_path_buf(),
        source: e,
    })?;
    Ok(open_with_text(client, file, content).await?.1)
}

/// Held by a query from its document sync to its last request, so no other
/// query changes the document's text in between.
type DocumentGuard = tokio::sync::OwnedMutexGuard<()>;

/// The lock that serializes the queries on one document.
async fn lock_document(file: &Path) -> DocumentGuard {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    type Locks = HashMap<PathBuf, Arc<tokio::sync::Mutex<()>>>;
    static LOCKS: OnceLock<Mutex<Locks>> = OnceLock::new();
    let lock = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(file.to_path_buf())
        .or_default()
        .clone();
    lock.lock_owned().await
}

/// Opens `file` with `text` when the caller holds the checked text, else as
/// [`open_document`] does. A document that is already open and was last
/// sent other text gets `text` as a `didChange`, so the query is answered
/// against the text its position was taken from. The returned guard keeps
/// any other query from changing the document until the caller drops it:
/// a query holds it until its last request is answered.
async fn open_for_query(
    client: &impl LspClientTrait,
    file: &Path,
    text: Option<&str>,
) -> Result<(DocumentReady, DocumentGuard)> {
    let guard = lock_document(file).await;
    let Some(text) = text else {
        return Ok((open_document(client, file).await?, guard));
    };
    let (opened, ready) = open_with_text(client, file, text.to_string()).await?;
    if opened == DocumentOpen::AlreadyOpen {
        client.change_text(&path_to_uri(file)?, text).await?;
    }
    Ok((ready, guard))
}

/// [`open_document`]'s body for a caller that already holds the text: the
/// `didOpen`, then the first-open readiness wait and its recorded outcome
/// (or, already open, the recorded outcome of the first open's wait). Also
/// returns whether this call sent the `didOpen`.
async fn open_with_text(
    client: &impl LspClientTrait,
    file: &Path,
    content: String,
) -> Result<(DocumentOpen, DocumentReady)> {
    let uri = path_to_uri(file)?;

    debug!("Opening document {} in LSP", file.display());
    let opened = client
        .did_open(uri.clone(), language_id(file).to_string(), content)
        .await?;

    // The wait below is a per-open cost, and this is called once per query —
    // once per declaration for the graph builder's callee pass. On a document
    // that is already open, `did_open` sent nothing, so the server will publish
    // nothing new and the wait can only run out its full timeout: 2 s of
    // waiting for an event that cannot arrive, 1,772 times over on
    // Textualize/rich. Its first analysis pass finished long ago; skip it —
    // but consult the outcome that wait recorded, so an earlier timeout is
    // still reported as one.
    if opened == DocumentOpen::AlreadyOpen {
        debug!(
            "{} already open; analysis wait already paid",
            file.display()
        );
        return match client.readiness(&uri).await {
            // The first open's wait ran out its budget; the server may still
            // not be ready, so the timeout is the honest answer.
            Some(false) => Ok((opened, DocumentReady::AnalysisTimedOut)),
            Some(true) | None => Ok((opened, DocumentReady::AlreadyOpen)),
        };
    }

    // Give the server the chance to finish its first analysis pass before any
    // query goes out. pyright answers `textDocument/references` immediately
    // after a didOpen with the open file's hits only, and publishes its
    // diagnostics afterwards; asking before that signal loses every call site
    // in every other file.
    //
    // The bool it returns is the whole point of the wait and must not be
    // dropped: false means the budget ran out with no `publishDiagnostics`, so
    // anything the server says next comes from a partial index. A caller that
    // persists answers has to be able to refuse to cache that.
    let matched = client
        .wait_for_diagnostics(&uri, ANALYSIS_READY_TIMEOUT_MS)
        .await;
    // Record the outcome so the already-open arm above can carry it — an
    // analyzed open is as much a fact as a timed-out one.
    client.record_readiness(&uri, matched).await;
    if matched {
        Ok((opened, DocumentReady::Analyzed))
    } else {
        debug!(
            "{} analysis not ready within {ANALYSIS_READY_TIMEOUT_MS} ms; answers \
             about it are from a partial index",
            file.display()
        );
        Ok((opened, DocumentReady::AnalysisTimedOut))
    }
}

/// Queries the LSP server for all references to a symbol at the given location.
pub async fn query_references(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
    text: Option<&str>,
) -> Result<ReferencesResult> {
    debug!(
        "Querying references at {}:{}:{}",
        file.display(),
        position.line(),
        position.column()
    );

    // Ensure the document is open in the LSP server
    let (_, _document) = open_for_query(client, file, text).await?;

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: lsp_position,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
        context: lsp_types::ReferenceContext {
            include_declaration: true,
        },
    };

    let response: Option<Vec<lsp_types::Location>> = client.request::<References>(params).await?;

    let Some(response) = response else {
        return Err(LspError::NullResult {
            method: References::METHOD,
        });
    };

    let ranges = response
        .into_iter()
        .filter_map(|loc| {
            let path = url_to_path(&loc.uri).ok()?;
            Some(crate::types::lsp_range_to_range(&loc.range, path))
        })
        .collect();

    Ok(ReferencesResult { ranges })
}

fn require_advertised(
    client: &impl LspClientTrait,
    operation: &'static str,
    capability: &'static str,
    advertised: impl Fn(&lsp_types::ServerCapabilities) -> bool,
) -> Result<()> {
    match client.server_capabilities() {
        Some(capabilities) if !advertised(capabilities) => Err(LspError::NotAdvertised {
            operation,
            capability,
        }),
        _ => Ok(()),
    }
}

fn advertises_call_hierarchy(capabilities: &lsp_types::ServerCapabilities) -> bool {
    !matches!(
        capabilities.call_hierarchy_provider,
        None | Some(lsp_types::CallHierarchyServerCapability::Simple(false))
    )
}

/// Queries the LSP server for all callers of a function at the given location.
pub async fn query_callers(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
    text: Option<&str>,
) -> Result<CallersResult> {
    debug!(
        "Querying callers at {}:{}:{}",
        file.display(),
        position.line(),
        position.column()
    );

    require_advertised(
        client,
        "callers",
        "callHierarchyProvider",
        advertises_call_hierarchy,
    )?;

    // Ensure the document is open in the LSP server
    let (_, _document) = open_for_query(client, file, text).await?;

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    // First, prepare the call hierarchy
    let prepare_params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: lsp_position,
        },
        work_done_progress_params: Default::default(),
    };

    let items: Option<Vec<CallHierarchyItem>> = client
        .request::<CallHierarchyPrepare>(prepare_params)
        .await?;

    let Some(items) = items else {
        return Err(LspError::NullResult {
            method: CallHierarchyPrepare::METHOD,
        });
    };

    let mut all_callers = Vec::new();

    // For each item, get incoming calls
    for item in items {
        let incoming_params = CallHierarchyIncomingCallsParams {
            item: item.clone(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let incoming: Option<Vec<CallHierarchyIncomingCall>> = client
            .request::<CallHierarchyIncomingCalls>(incoming_params)
            .await?;

        let Some(calls) = incoming else {
            return Err(LspError::NullResult {
                method: CallHierarchyIncomingCalls::METHOD,
            });
        };
        for call in calls {
            let path = match url_to_path(&call.from.uri) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let range = lsp_range_to_range(&call.from.selection_range, path.clone());

            all_callers.push(Symbol::new(
                call.from.name.clone(),
                lsp_symbol_kind_to_act(&call.from.kind),
                range,
            ));
        }
    }

    Ok(CallersResult {
        callers: all_callers,
    })
}

/// Queries the LSP server for type information at the given location.
pub async fn query_type(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
) -> Result<TypeInfo> {
    debug!(
        "Querying type at {}:{}:{}",
        file.display(),
        position.line(),
        position.column()
    );

    // Ensure the document is open in the LSP server
    open_document(client, file).await?;

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: lsp_position,
        },
        work_done_progress_params: Default::default(),
    };

    let response: Option<lsp_types::Hover> = client.request::<HoverRequest>(params).await?;

    let Some(hover) = response else {
        return Err(LspError::NullResult {
            method: HoverRequest::METHOD,
        });
    };

    let content = match hover.contents {
        lsp_types::HoverContents::Scalar(markup) => markup_to_string(markup),
        lsp_types::HoverContents::Array(items) => items
            .into_iter()
            .map(markup_to_string)
            .collect::<Vec<_>>()
            .join("\n\n"),
        lsp_types::HoverContents::Markup(markup) => markup.value,
    };

    let range = hover
        .range
        .map(|r| lsp_range_to_range(&r, file.to_path_buf()));

    Ok(TypeInfo { content, range })
}

/// Validates that a rename is possible at the given location.
pub async fn validate_rename(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
    new_name: &str,
) -> Result<RenamePreview> {
    debug!(
        "Validating rename at {}:{}:{} to '{}'",
        file.display(),
        position.line(),
        position.column(),
        new_name
    );

    // Ensure the document is open in the LSP server
    open_document(client, file).await?;

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: lsp_position,
        },
        new_name: new_name.to_string(),
        work_done_progress_params: Default::default(),
    };

    let response: Option<WorkspaceEdit> = client.request::<RenameRequest>(params).await?;

    match response {
        Some(edit) => {
            let mut changes = Vec::new();

            if let Some(document_changes) = edit.changes {
                for (uri, edits) in document_changes {
                    let path = url_to_path(&uri)?;
                    let ranges: Vec<Range> = edits
                        .iter()
                        .map(|e| lsp_range_to_range(&e.range, path.clone()))
                        .collect();
                    changes.push((path, ranges));
                }
            }

            Ok(RenamePreview { changes })
        }
        None => Err(LspError::ResponseError {
            language: "unknown".to_string(),
            code: -1,
            message: "rename not supported at this location".to_string(),
        }),
    }
}

/// Queries the LSP server for code actions at the given range.
///
/// Code actions include quick fixes, refactorings, and other automated transformations.
pub async fn query_code_actions(
    client: &impl LspClientTrait,
    file: &Path,
    range: &Range,
    diagnostics: &[Diagnostic],
) -> Result<CodeActionsResult> {
    debug!(
        "Querying code actions at {}:{}:{}-{}:{}",
        file.display(),
        range.start().line(),
        range.start().column(),
        range.end().line(),
        range.end().column(),
    );

    // Ensure the document is open in the LSP server
    open_document(client, file).await?;

    let uri = path_to_uri(file)?;
    let lsp_range = crate::types::range_to_lsp_range(range);

    // Convert our diagnostics to LSP diagnostics
    let lsp_diagnostics: Vec<lsp_types::Diagnostic> = diagnostics
        .iter()
        .map(|d| lsp_types::Diagnostic {
            range: crate::types::range_to_lsp_range(&d.range),
            severity: Some(match d.severity {
                DiagnosticSeverity::Error => lsp_types::DiagnosticSeverity::ERROR,
                DiagnosticSeverity::Warning => lsp_types::DiagnosticSeverity::WARNING,
                DiagnosticSeverity::Information => lsp_types::DiagnosticSeverity::INFORMATION,
                DiagnosticSeverity::Hint => lsp_types::DiagnosticSeverity::HINT,
            }),
            message: d.message.clone(),
            source: d.source.clone(),
            code: d.code.clone().map(lsp_types::NumberOrString::String),
            ..Default::default()
        })
        .collect();

    let params = CodeActionParams {
        text_document: TextDocumentIdentifier { uri },
        range: lsp_range,
        context: CodeActionContext {
            diagnostics: lsp_diagnostics,
            only: None,
            trigger_kind: Some(lsp_types::CodeActionTriggerKind::INVOKED),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let response: Option<CodeActionResponse> = client.request::<CodeActionRequest>(params).await?;

    let Some(response) = response else {
        return Err(LspError::NullResult {
            method: CodeActionRequest::METHOD,
        });
    };

    let actions = response
        .into_iter()
        .map(|action_or_command| {
            match action_or_command {
                lsp_types::CodeActionOrCommand::CodeAction(action) => {
                    // Convert workspace edit if present
                    let edit = action.edit.and_then(|we| {
                        we.changes.map(|changes| {
                            changes
                                .into_iter()
                                .filter_map(|(uri, edits)| {
                                    let path = url_to_path(&uri).ok()?;
                                    let text_edits: Vec<(Range, String)> = edits
                                        .iter()
                                        .map(|e| {
                                            (
                                                lsp_range_to_range(&e.range, path.clone()),
                                                e.new_text.clone(),
                                            )
                                        })
                                        .collect();
                                    Some((path, text_edits))
                                })
                                .collect()
                        })
                    });

                    CodeAction {
                        title: action.title,
                        kind: action.kind.map(|k| k.as_str().to_string()),
                        is_preferred: action.is_preferred.unwrap_or(false),
                        edit,
                    }
                }
                lsp_types::CodeActionOrCommand::Command(cmd) => {
                    // Commands need to be executed separately - return as action without edit
                    CodeAction {
                        title: cmd.title,
                        kind: None,
                        is_preferred: false,
                        edit: None,
                    }
                }
            }
        })
        .collect();

    Ok(CodeActionsResult { actions })
}

/// Result of a callees query.
#[derive(Debug, Clone)]
pub struct CalleesResult {
    pub callees: Vec<Symbol>,
}

/// Result of an implementations query.
#[derive(Debug, Clone)]
pub struct ImplementationsResult {
    pub locations: Vec<Range>,
}

/// Result of a diagnostics query.
#[derive(Debug, Clone)]
pub struct DiagnosticsResult {
    pub diagnostics: Vec<Diagnostic>,
}

/// Result of a workspace symbol search.
#[derive(Debug, Clone)]
pub struct WorkspaceSymbolsResult {
    pub symbols: Vec<Symbol>,
}

/// Queries the LSP server for all callees of a function at the given location.
pub async fn query_callees(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
    text: Option<&str>,
) -> Result<CalleesResult> {
    debug!(
        "Querying callees at {}:{}:{}",
        file.display(),
        position.line(),
        position.column()
    );

    require_advertised(
        client,
        "callees",
        "callHierarchyProvider",
        advertises_call_hierarchy,
    )?;

    // Ensure the document is open in the LSP server.
    //
    // A timed-out first open is refused rather than answered, and a `null`
    // from `prepareCallHierarchy` is likewise refused below as no-answer via
    // `LspError::NullResult` — both for the same reason: an unanswered or
    // partially-answered query, recorded as a confirmed-empty callee list, is
    // served from the graph builder's content-hash-keyed cache as a measured
    // zero until the file changes, and nothing short of deleting the index
    // would ever revisit it. An error here leaves the file's record
    // incomplete, and the next build retries.
    let (ready, _document) = open_for_query(client, file, text).await?;
    if ready == DocumentReady::AnalysisTimedOut {
        return Err(LspError::AnalysisNotReady {
            path: file.display().to_string(),
            language: "the language server".to_string(),
            timeout_ms: ANALYSIS_READY_TIMEOUT_MS,
        });
    }

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    // First, prepare the call hierarchy
    let prepare_params = CallHierarchyPrepareParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: lsp_position,
        },
        work_done_progress_params: Default::default(),
    };

    let items: Option<Vec<CallHierarchyItem>> = client
        .request::<CallHierarchyPrepare>(prepare_params)
        .await?;

    let Some(items) = items else {
        return Err(LspError::NullResult {
            method: CallHierarchyPrepare::METHOD,
        });
    };

    let mut all_callees = Vec::new();

    // For each item, get outgoing calls
    for item in items {
        let outgoing_params = CallHierarchyOutgoingCallsParams {
            item: item.clone(),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let outgoing: Option<Vec<CallHierarchyOutgoingCall>> = client
            .request::<CallHierarchyOutgoingCalls>(outgoing_params)
            .await?;

        let Some(calls) = outgoing else {
            return Err(LspError::NullResult {
                method: CallHierarchyOutgoingCalls::METHOD,
            });
        };
        for call in calls {
            let path = match url_to_path(&call.to.uri) {
                Ok(p) => p,
                Err(_) => continue,
            };

            let range = lsp_range_to_range(&call.to.selection_range, path.clone());

            all_callees.push(Symbol::new(
                call.to.name.clone(),
                lsp_symbol_kind_to_act(&call.to.kind),
                range,
            ));
        }
    }

    Ok(CalleesResult {
        callees: all_callees,
    })
}

/// Queries the LSP server for implementations of an interface/trait at the given location.
pub async fn query_implementations(
    client: &impl LspClientTrait,
    file: &Path,
    position: &Location,
    text: Option<&str>,
) -> Result<ImplementationsResult> {
    debug!(
        "Querying implementations at {}:{}:{}",
        file.display(),
        position.line(),
        position.column()
    );

    require_advertised(client, "implementations", "implementationProvider", |c| {
        !matches!(
            c.implementation_provider,
            None | Some(lsp_types::ImplementationProviderCapability::Simple(false))
        )
    })?;

    let (_, _document) = open_for_query(client, file, text).await?;

    let uri = path_to_uri(file)?;
    let lsp_position = crate::types::location_to_position(position);

    let params = lsp_types::GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: lsp_position,
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let response: Option<GotoDefinitionResponse> =
        client.request::<GotoImplementation>(params).await?;

    let Some(response) = response else {
        return Err(LspError::NullResult {
            method: GotoImplementation::METHOD,
        });
    };

    let locations = match response {
        GotoDefinitionResponse::Scalar(loc) => {
            let path = url_to_path(&loc.uri)?;
            vec![lsp_range_to_range(&loc.range, path)]
        }
        GotoDefinitionResponse::Array(locs) => locs
            .into_iter()
            .filter_map(|loc| {
                let path = url_to_path(&loc.uri).ok()?;
                Some(lsp_range_to_range(&loc.range, path))
            })
            .collect(),
        GotoDefinitionResponse::Link(links) => links
            .into_iter()
            .filter_map(|link| {
                let path = url_to_path(&link.target_uri).ok()?;
                Some(lsp_range_to_range(&link.target_selection_range, path))
            })
            .collect(),
    };

    Ok(ImplementationsResult { locations })
}

pub async fn query_diagnostics(
    client: &impl LspClientTrait,
    file: &Path,
) -> Result<DiagnosticsResult> {
    use lsp_types::request::DocumentDiagnosticRequest;

    debug!("Querying diagnostics for {}", file.display());

    open_document(client, file).await?;

    let uri = path_to_uri(file)?;
    let file_path = file.to_path_buf();

    let params = lsp_types::DocumentDiagnosticParams {
        text_document: TextDocumentIdentifier { uri },
        identifier: None,
        previous_result_id: None,
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    // `DocumentDiagnosticRequest::Result` is `DocumentDiagnosticReportResult`,
    // not `Option<_>`, so a wire `null` cannot be told apart from a real
    // answer through the ordinary request path. This locally scoped
    // `Request` — same METHOD/Params, `Result =
    // Option<DocumentDiagnosticReportResult>` — routes through the same
    // deserialization `request` uses for every genuinely-nullable request
    // (e.g. `WorkspaceSymbolRequest`), so a `null` comes back as `Ok(None)`.
    struct NullableDiagnosticRequest;
    impl Request for NullableDiagnosticRequest {
        type Params = lsp_types::DocumentDiagnosticParams;
        type Result = Option<lsp_types::DocumentDiagnosticReportResult>;
        const METHOD: &'static str = DocumentDiagnosticRequest::METHOD;
    }

    let response = match client.request::<NullableDiagnosticRequest>(params).await {
        Ok(None) => {
            return Err(LspError::NullResult {
                method: DocumentDiagnosticRequest::METHOD,
            });
        }
        Ok(Some(report)) => report,
        Err(LspError::ResponseError { code: -32601, .. }) => {
            return Ok(DiagnosticsResult {
                diagnostics: vec![],
            });
        }
        Err(e) => return Err(e),
    };

    let diagnostics = match response {
        lsp_types::DocumentDiagnosticReportResult::Report(report) => match report {
            lsp_types::DocumentDiagnosticReport::Full(full) => full
                .full_document_diagnostic_report
                .items
                .iter()
                .map(|d| lsp_diagnostic_to_diagnostic(d, file_path.clone()))
                .collect(),
            lsp_types::DocumentDiagnosticReport::Unchanged(_) => vec![],
        },
        lsp_types::DocumentDiagnosticReportResult::Partial(_) => vec![],
    };

    Ok(DiagnosticsResult { diagnostics })
}

/// Queries the LSP server for workspace symbols matching a query string.
pub async fn query_workspace_symbols(
    client: &impl LspClientTrait,
    query: &str,
) -> Result<WorkspaceSymbolsResult> {
    debug!("Querying workspace symbols for '{}'", query);

    let params = WorkspaceSymbolParams {
        query: query.to_string(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    let response: Option<lsp_types::WorkspaceSymbolResponse> =
        client.request::<WorkspaceSymbolRequest>(params).await?;

    let Some(response) = response else {
        return Err(LspError::NullResult {
            method: WorkspaceSymbolRequest::METHOD,
        });
    };

    let symbols = match response {
        lsp_types::WorkspaceSymbolResponse::Flat(infos) => infos
            .into_iter()
            .filter_map(|sym| {
                let path = url_to_path(&sym.location.uri).ok()?;
                let range = lsp_range_to_range(&sym.location.range, path);
                Some(Symbol::new(
                    sym.name,
                    lsp_symbol_kind_to_act(&sym.kind),
                    range,
                ))
            })
            .collect(),
        lsp_types::WorkspaceSymbolResponse::Nested(syms) => syms
            .into_iter()
            .filter_map(|sym| {
                let loc = sym.location.clone();
                match loc {
                    lsp_types::OneOf::Left(location) => {
                        let path = url_to_path(&location.uri).ok()?;
                        let range = lsp_range_to_range(&location.range, path);
                        Some(Symbol::new(
                            sym.name,
                            lsp_symbol_kind_to_act(&sym.kind),
                            range,
                        ))
                    }
                    lsp_types::OneOf::Right(_) => None,
                }
            })
            .collect(),
    };

    Ok(WorkspaceSymbolsResult { symbols })
}

/// Converts an LSP diagnostic severity to our severity type.
pub fn lsp_severity_to_diagnostic_severity(
    severity: Option<lsp_types::DiagnosticSeverity>,
) -> DiagnosticSeverity {
    match severity {
        Some(lsp_types::DiagnosticSeverity::ERROR) => DiagnosticSeverity::Error,
        Some(lsp_types::DiagnosticSeverity::WARNING) => DiagnosticSeverity::Warning,
        Some(lsp_types::DiagnosticSeverity::INFORMATION) => DiagnosticSeverity::Information,
        Some(lsp_types::DiagnosticSeverity::HINT) => DiagnosticSeverity::Hint,
        _ => DiagnosticSeverity::Information, // Default to Information for unknown
    }
}

/// Converts an LSP diagnostic to our diagnostic type.
pub fn lsp_diagnostic_to_diagnostic(lsp_diag: &lsp_types::Diagnostic, file: PathBuf) -> Diagnostic {
    let range = lsp_range_to_range(&lsp_diag.range, file);
    let code = match &lsp_diag.code {
        Some(lsp_types::NumberOrString::Number(n)) => Some(n.to_string()),
        Some(lsp_types::NumberOrString::String(s)) => Some(s.clone()),
        None => None,
    };

    Diagnostic {
        range,
        severity: lsp_severity_to_diagnostic_severity(lsp_diag.severity),
        message: lsp_diag.message.clone(),
        source: lsp_diag.source.clone(),
        code,
    }
}

/// Converts an LSP URI to a file path.
fn url_to_path(uri: &Uri) -> Result<PathBuf> {
    crate::types::uri_to_path(uri)
}

/// Converts LSP SymbolKind to act SymbolKind.
fn lsp_symbol_kind_to_act(kind: &lsp_types::SymbolKind) -> SymbolKind {
    match *kind {
        lsp_types::SymbolKind::FUNCTION | lsp_types::SymbolKind::METHOD => SymbolKind::Function,
        lsp_types::SymbolKind::CLASS => SymbolKind::Class,
        lsp_types::SymbolKind::INTERFACE => SymbolKind::Interface,
        lsp_types::SymbolKind::ENUM => SymbolKind::Enum,
        lsp_types::SymbolKind::VARIABLE | lsp_types::SymbolKind::CONSTANT => SymbolKind::Variable,
        lsp_types::SymbolKind::STRUCT => SymbolKind::Struct,
        lsp_types::SymbolKind::TYPE_PARAMETER => SymbolKind::TypeParameter,
        _ => SymbolKind::Unknown,
    }
}

/// Converts LSP markup content to plain string.
fn markup_to_string(content: lsp_types::MarkedString) -> String {
    match content {
        lsp_types::MarkedString::String(s) => s,
        lsp_types::MarkedString::LanguageString(ls) => ls.value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `sourcekit-lsp` creates no language service for a document opened as
    /// `plaintext`; a Swift file needs its own id.
    #[test]
    fn language_id_names_swift() {
        assert_eq!(language_id(Path::new("Lib.swift")), "swift");
    }

    /// intelephense ignores a document opened as `plaintext`; a PHP file
    /// needs its own id.
    #[test]
    fn language_id_names_php() {
        assert_eq!(language_id(Path::new("lib.php")), "php");
    }

    /// Every grammar backed by a configured server opens with a language id,
    /// never `plaintext`, for each extension routed to it.
    #[test]
    fn language_id_names_every_server_backed_grammar() {
        use crate::servers::LspServer;
        for grammar in Grammar::all() {
            if LspServer::for_grammar(grammar).is_none() {
                continue;
            }
            for ext in grammar.language_support().extensions() {
                let file = PathBuf::from(format!("f.{ext}"));
                if Grammar::from_path(&file) != Some(grammar) {
                    continue;
                }
                assert_ne!(
                    language_id(&file),
                    "plaintext",
                    "{grammar:?}: .{ext} opens as plaintext"
                );
            }
        }
    }

    #[test]
    fn test_lsp_symbol_kind_conversion() {
        // Table-driven test covering all LSP symbol kind mappings
        let cases = [
            // Kinds that map to specific act types
            (lsp_types::SymbolKind::FUNCTION, SymbolKind::Function),
            (lsp_types::SymbolKind::METHOD, SymbolKind::Function),
            (lsp_types::SymbolKind::CLASS, SymbolKind::Class),
            (lsp_types::SymbolKind::INTERFACE, SymbolKind::Interface),
            (lsp_types::SymbolKind::ENUM, SymbolKind::Enum),
            (lsp_types::SymbolKind::VARIABLE, SymbolKind::Variable),
            (lsp_types::SymbolKind::CONSTANT, SymbolKind::Variable),
            (lsp_types::SymbolKind::STRUCT, SymbolKind::Struct),
            (
                lsp_types::SymbolKind::TYPE_PARAMETER,
                SymbolKind::TypeParameter,
            ),
            // Kinds that map to Unknown
            (lsp_types::SymbolKind::FILE, SymbolKind::Unknown),
            (lsp_types::SymbolKind::MODULE, SymbolKind::Unknown),
            (lsp_types::SymbolKind::NAMESPACE, SymbolKind::Unknown),
            (lsp_types::SymbolKind::PACKAGE, SymbolKind::Unknown),
            (lsp_types::SymbolKind::PROPERTY, SymbolKind::Unknown),
            (lsp_types::SymbolKind::FIELD, SymbolKind::Unknown),
            (lsp_types::SymbolKind::CONSTRUCTOR, SymbolKind::Unknown),
            (lsp_types::SymbolKind::ENUM_MEMBER, SymbolKind::Unknown),
            (lsp_types::SymbolKind::STRING, SymbolKind::Unknown),
            (lsp_types::SymbolKind::NUMBER, SymbolKind::Unknown),
            (lsp_types::SymbolKind::BOOLEAN, SymbolKind::Unknown),
            (lsp_types::SymbolKind::ARRAY, SymbolKind::Unknown),
            (lsp_types::SymbolKind::OBJECT, SymbolKind::Unknown),
            (lsp_types::SymbolKind::KEY, SymbolKind::Unknown),
            (lsp_types::SymbolKind::NULL, SymbolKind::Unknown),
            (lsp_types::SymbolKind::EVENT, SymbolKind::Unknown),
            (lsp_types::SymbolKind::OPERATOR, SymbolKind::Unknown),
        ];

        for (lsp_kind, expected) in cases {
            assert_eq!(
                lsp_symbol_kind_to_act(&lsp_kind),
                expected,
                "SymbolKind::{:?} should map to {:?}",
                lsp_kind,
                expected
            );
        }
    }

    #[test]
    fn test_diagnostic_severity_conversion() {
        assert_eq!(
            lsp_severity_to_diagnostic_severity(Some(lsp_types::DiagnosticSeverity::ERROR)),
            DiagnosticSeverity::Error
        );
        assert_eq!(
            lsp_severity_to_diagnostic_severity(Some(lsp_types::DiagnosticSeverity::WARNING)),
            DiagnosticSeverity::Warning
        );
        assert_eq!(
            lsp_severity_to_diagnostic_severity(Some(lsp_types::DiagnosticSeverity::INFORMATION)),
            DiagnosticSeverity::Information
        );
        assert_eq!(
            lsp_severity_to_diagnostic_severity(Some(lsp_types::DiagnosticSeverity::HINT)),
            DiagnosticSeverity::Hint
        );
        // Unknown/None defaults to Information
        assert_eq!(
            lsp_severity_to_diagnostic_severity(None),
            DiagnosticSeverity::Information
        );
    }

    #[test]
    fn test_lsp_diagnostic_to_diagnostic() {
        let lsp_diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 4,
                    character: 9,
                },
                end: lsp_types::Position {
                    line: 4,
                    character: 15,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "Type error".to_string(),
            source: Some("typescript".to_string()),
            code: Some(lsp_types::NumberOrString::Number(2322)),
            ..Default::default()
        };

        let diag = lsp_diagnostic_to_diagnostic(&lsp_diag, PathBuf::from("test.ts"));

        assert_eq!(diag.severity, DiagnosticSeverity::Error);
        assert_eq!(diag.message, "Type error");
        assert_eq!(diag.source, Some("typescript".to_string()));
        assert_eq!(diag.code, Some("2322".to_string()));
        // Range conversion: LSP 0-indexed -> act-lite-core 1-indexed
        assert_eq!(diag.range.start().line(), 5);
        assert_eq!(diag.range.start().column(), 10);
    }

    #[test]
    fn test_lsp_diagnostic_with_string_code() {
        let lsp_diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 10,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            message: "Warning".to_string(),
            source: None,
            code: Some(lsp_types::NumberOrString::String("TS2345".to_string())),
            ..Default::default()
        };

        let diag = lsp_diagnostic_to_diagnostic(&lsp_diag, PathBuf::from("test.ts"));
        assert_eq!(diag.code, Some("TS2345".to_string()));
        assert_eq!(diag.severity, DiagnosticSeverity::Warning);
        assert_eq!(diag.source, None);
    }

    #[test]
    fn test_lsp_diagnostic_with_no_code() {
        let lsp_diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 5,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::HINT),
            message: "Hint message".to_string(),
            source: Some("eslint".to_string()),
            code: None,
            ..Default::default()
        };

        let diag = lsp_diagnostic_to_diagnostic(&lsp_diag, PathBuf::from("src/main.ts"));
        assert_eq!(diag.code, None);
        assert_eq!(diag.severity, DiagnosticSeverity::Hint);
        assert_eq!(diag.message, "Hint message");
    }

    #[test]
    fn test_markup_to_string_plain() {
        let markup = lsp_types::MarkedString::String("plain text".to_string());
        assert_eq!(markup_to_string(markup), "plain text");
    }

    #[test]
    fn test_markup_to_string_language() {
        let markup = lsp_types::MarkedString::LanguageString(lsp_types::LanguageString {
            language: "typescript".to_string(),
            value: "const x: number = 5".to_string(),
        });
        assert_eq!(markup_to_string(markup), "const x: number = 5");
    }

    #[test]
    fn test_url_to_path_valid() {
        let uri: lsp_types::Uri = "file:///home/user/test.ts".parse().unwrap();
        let result = url_to_path(&uri);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from("/home/user/test.ts"));
    }

    #[test]
    fn test_url_to_path_invalid_scheme() {
        let uri: lsp_types::Uri = "https://example.com/test.ts".parse().unwrap();
        let result = url_to_path(&uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_diagnostic_code_as_number() {
        // LSP diagnostic with numeric code converted to string
        let lsp_diag = lsp_types::Diagnostic {
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 10,
                },
            },
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "Type error".to_string(),
            source: Some("tsc".to_string()),
            code: Some(lsp_types::NumberOrString::Number(7027)),
            ..Default::default()
        };

        let diag = lsp_diagnostic_to_diagnostic(&lsp_diag, PathBuf::from("test.ts"));
        assert_eq!(diag.code, Some("7027".to_string()));
    }

    // Tests for async capability functions using MockLspClient

    use crate::mock::MockLspClient;
    use lsp_types::request::References;
    use tempfile::tempdir;

    /// `sync_document` opens an unopened document with the new text, and
    /// sends an already-open one a full-text `didChange` at a version above
    /// the `didOpen` version — never a second, deduplicated-away `didOpen`.
    #[tokio::test]
    async fn sync_document_opens_then_changes_with_full_text() {
        let mock = MockLspClient::new_lenient();
        let file = std::path::Path::new("/ws/main.ts");

        sync_document(&mock, file, "v1".to_string()).await.unwrap();
        sync_document(&mock, file, "v2".to_string()).await.unwrap();
        sync_document(&mock, file, "v3".to_string()).await.unwrap();

        let sent = mock.sent_notifications();
        let methods: Vec<&str> = sent.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(
            methods,
            [
                "textDocument/didOpen",
                "textDocument/didChange",
                "textDocument/didChange"
            ]
        );
        assert_eq!(sent[0].1["textDocument"]["text"], "v1");
        let (v2, v3) = (
            sent[1].1["textDocument"]["version"].as_i64().unwrap(),
            sent[2].1["textDocument"]["version"].as_i64().unwrap(),
        );
        assert!(
            1 < v2 && v2 < v3,
            "versions must strictly increase: {v2}, {v3}"
        );
        assert_eq!(sent[1].1["contentChanges"][0]["text"], "v2");
        assert!(sent[1].1["contentChanges"][0].get("range").is_none());
        assert_eq!(sent[2].1["contentChanges"][0]["text"], "v3");
    }

    #[tokio::test]
    async fn test_query_references_returns_results() {
        let mock = MockLspClient::new_lenient();

        // Register mock response for references query
        let expected_refs = vec![
            lsp_types::Location {
                uri: "file:///test/src/main.ts".parse().unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 4,
                        character: 9,
                    },
                    end: lsp_types::Position {
                        line: 4,
                        character: 12,
                    },
                },
            },
            lsp_types::Location {
                uri: "file:///test/src/main.ts".parse().unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 10,
                        character: 5,
                    },
                    end: lsp_types::Position {
                        line: 10,
                        character: 8,
                    },
                },
            },
        ];
        mock.register_response::<References>(Some(expected_refs));

        // Create a temp file so open_document can read it
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const foo = 1;\nconsole.log(foo);").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_references(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        let refs = result.unwrap();
        assert_eq!(refs.ranges.len(), 2);
    }

    /// A query given the caller's checked text sends that text and does not
    /// read the path again: a FIFO put at the path does not block it.
    #[cfg(unix)]
    #[tokio::test]
    async fn query_with_checked_text_does_not_read_the_path() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<References>(Some(vec![]));
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        let c = std::ffi::CString::new(file_path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let location = Location::new(file_path.clone(), 1, 7);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            query_references(&mock, &file_path, &location, Some("const x = 1;")),
        )
        .await
        .expect("the query must not wait on the path");
        assert!(result.is_ok());
        let opened = mock
            .sent_notifications()
            .into_iter()
            .find(|(method, _)| method == "textDocument/didOpen")
            .expect("didOpen sent");
        assert_eq!(opened.1["textDocument"]["text"], "const x = 1;");
    }

    /// A query on an already-open document with checked text that differs
    /// from what the server holds sends that text (`didChange`, full text)
    /// before the request; the same text again sends nothing more.
    #[tokio::test]
    async fn query_on_an_open_document_sends_changed_text_first() {
        let mock = MockLspClient::new_lenient();
        for _ in 0..3 {
            mock.register_response::<References>(Some(vec![]));
        }
        let file = std::path::Path::new("/ws/changed.ts");
        let location = Location::new(file.to_path_buf(), 1, 7);
        for text in ["const a = 1;", "const b = 2;", "const b = 2;"] {
            query_references(&mock, file, &location, Some(text))
                .await
                .unwrap();
        }
        let sent = mock.sent_notifications();
        let methods: Vec<&str> = sent.iter().map(|(m, _)| m.as_str()).collect();
        assert_eq!(methods, ["textDocument/didOpen", "textDocument/didChange"]);
        assert_eq!(sent[0].1["textDocument"]["text"], "const a = 1;");
        assert_eq!(sent[1].1["contentChanges"][0]["text"], "const b = 2;");
        assert!(sent[1].1["textDocument"]["version"].as_i64().unwrap() > 1);
        assert_eq!(
            mock.sent_requests().len(),
            3,
            "each query sends its request"
        );
    }

    /// Concurrent queries on one open document with different texts leave the
    /// server's text and the client's record in agreement: the last text sent
    /// is the one the client records, so asking for it again sends nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_queries_leave_the_sent_text_and_the_record_in_agreement() {
        for round in 0..50 {
            let mock = std::sync::Arc::new(MockLspClient::new_lenient());
            let file = std::path::PathBuf::from(format!("/ws/race{round}.ts"));
            open_for_query(mock.as_ref(), &file, Some("t0"))
                .await
                .unwrap();
            let tasks: Vec<_> = (1..=8)
                .map(|i| {
                    let (mock, file) = (mock.clone(), file.clone());
                    tokio::spawn(async move {
                        open_for_query(mock.as_ref(), &file, Some(&format!("t{i}")))
                            .await
                            .unwrap();
                    })
                })
                .collect();
            for task in tasks {
                task.await.unwrap();
            }
            let sent = mock.sent_notifications();
            let (method, last) = sent.last().unwrap();
            assert_eq!(method, "textDocument/didChange");
            let text = last["contentChanges"][0]["text"]
                .as_str()
                .unwrap()
                .to_string();
            let versions: Vec<i64> = sent[1..]
                .iter()
                .map(|(_, p)| p["textDocument"]["version"].as_i64().unwrap())
                .collect();
            assert!(versions.windows(2).all(|w| w[0] < w[1]), "{versions:?}");
            let uri = path_to_uri(&file).unwrap();
            assert!(
                !mock.change_text(&uri, &text).await.unwrap(),
                "round {round}: the server holds {text}, the record disagrees"
            );
        }
    }

    /// While one query holds a document, another query with other text waits:
    /// its `didChange` is not sent until the first query is done, so the
    /// first query's requests see the text its position was taken from.
    #[tokio::test]
    async fn a_query_waits_for_the_query_that_holds_the_document() {
        let mock = std::sync::Arc::new(MockLspClient::new_lenient());
        mock.register_response::<References>(Some(vec![]));
        let file = std::path::PathBuf::from("/ws/held.ts");
        let (_, held) = open_for_query(mock.as_ref(), &file, Some("text A"))
            .await
            .unwrap();
        let second = {
            let (mock, file) = (mock.clone(), file.clone());
            tokio::spawn(async move {
                let location = Location::new(file.clone(), 1, 1);
                query_references(mock.as_ref(), &file, &location, Some("text B"))
                    .await
                    .unwrap();
            })
        };
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let methods = |mock: &MockLspClient| -> Vec<String> {
            mock.sent_notifications()
                .into_iter()
                .map(|(m, _)| m)
                .collect()
        };
        assert_eq!(methods(&mock), ["textDocument/didOpen"], "B waits for A");
        assert!(mock.sent_requests().is_empty(), "B sends no request yet");
        drop(held);
        second.await.unwrap();
        assert_eq!(
            methods(&mock),
            ["textDocument/didOpen", "textDocument/didChange"]
        );
    }

    /// Without the caller's text, a FIFO at the path is refused, not waited on.
    #[cfg(unix)]
    #[tokio::test]
    async fn open_document_refuses_a_fifo_without_waiting() {
        let mock = MockLspClient::new_lenient();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        let c = std::ffi::CString::new(file_path.as_os_str().as_encoded_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            open_document(&mock, &file_path),
        )
        .await
        .expect("open_document must not wait on a FIFO");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_query_references_empty_result() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<References>(Some(vec![]));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_references(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().ranges.is_empty());
    }

    #[tokio::test]
    async fn test_query_type_returns_hover_info() {
        use lsp_types::request::HoverRequest;

        let mock = MockLspClient::new_lenient();

        let hover = lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                "const foo: number".to_string(),
            )),
            range: Some(lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 6,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 9,
                },
            }),
        };
        mock.register_response::<HoverRequest>(Some(hover));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const foo = 42;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_type(&mock, &file_path, &location).await;

        assert!(result.is_ok());
        let type_info = result.unwrap();
        assert_eq!(type_info.content, "const foo: number");
        assert!(type_info.range.is_some());
    }

    #[tokio::test]
    async fn query_type_null_result_is_no_answer_not_empty() {
        use lsp_types::request::HoverRequest;

        let mock = MockLspClient::new_lenient();
        mock.register_response::<HoverRequest>(None::<lsp_types::Hover>);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_type(&mock, &file_path, &location).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/hover");
            }
            other => panic!("a wire null must be no-answer, not a measured empty: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_type_non_null_empty_hover_stays_ok() {
        use lsp_types::request::HoverRequest;

        let mock = MockLspClient::new_lenient();
        // A non-null hover whose content is empty is the server answering
        // "I looked; there is nothing to show" — a measured empty, not a
        // refusal. It must stay Ok.
        mock.register_response::<HoverRequest>(Some(lsp_types::Hover {
            contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                String::new(),
            )),
            range: None,
        }));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "// comment").unwrap();

        let location = Location::new(file_path.clone(), 1, 1);
        let result = query_type(&mock, &file_path, &location).await;

        match result {
            Ok(type_info) => {
                assert!(type_info.content.is_empty());
                assert!(type_info.range.is_none());
            }
            other => panic!(
                "a non-null empty hover is a measured empty answer, not no-answer: {other:?}"
            ),
        }
    }

    #[tokio::test]
    async fn test_validate_rename_success() {
        use lsp_types::request::Rename;

        let mock = MockLspClient::new_lenient();

        #[allow(clippy::mutable_key_type)]
        let mut changes = std::collections::HashMap::new();
        changes.insert(
            "file:///test/src/main.ts".parse().unwrap(),
            vec![lsp_types::TextEdit {
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 6,
                    },
                    end: lsp_types::Position {
                        line: 0,
                        character: 9,
                    },
                },
                new_text: "bar".to_string(),
            }],
        );
        let edit = lsp_types::WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        };
        mock.register_response::<Rename>(Some(edit));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const foo = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = validate_rename(&mock, &file_path, &location, "bar").await;

        assert!(result.is_ok());
        let preview = result.unwrap();
        assert!(!preview.changes.is_empty());
    }

    #[tokio::test]
    async fn test_validate_rename_not_supported() {
        use lsp_types::request::Rename;

        let mock = MockLspClient::new_lenient();
        mock.register_response::<Rename>(None::<lsp_types::WorkspaceEdit>);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "// comment").unwrap();

        let location = Location::new(file_path.clone(), 1, 1);
        let result = validate_rename(&mock, &file_path, &location, "newname").await;

        assert!(result.is_err());
    }

    // Tests for query_callees

    #[tokio::test]
    async fn test_query_callees_returns_results() {
        use lsp_types::request::{CallHierarchyOutgoingCalls, CallHierarchyPrepare};

        let mock = MockLspClient::new_lenient();

        // Register prepare response
        let item = CallHierarchyItem {
            name: "myFunction".to_string(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: "file:///test/src/main.ts".parse().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 9,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 19,
                },
            },
            data: None,
        };
        mock.register_response::<CallHierarchyPrepare>(Some(vec![item.clone()]));

        // Register outgoing calls response
        let outgoing = vec![CallHierarchyOutgoingCall {
            to: CallHierarchyItem {
                name: "calledFn".to_string(),
                kind: lsp_types::SymbolKind::FUNCTION,
                tags: None,
                detail: None,
                uri: "file:///test/src/utils.ts".parse().unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 10,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 15,
                        character: 1,
                    },
                },
                selection_range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 10,
                        character: 9,
                    },
                    end: lsp_types::Position {
                        line: 10,
                        character: 17,
                    },
                },
                data: None,
            },
            from_ranges: vec![lsp_types::Range {
                start: lsp_types::Position {
                    line: 2,
                    character: 4,
                },
                end: lsp_types::Position {
                    line: 2,
                    character: 12,
                },
            }],
        }];
        mock.register_response::<CallHierarchyOutgoingCalls>(Some(outgoing));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "function myFunction() { calledFn(); }").unwrap();

        let location = Location::new(file_path.clone(), 1, 10);
        let result = query_callees(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        let callees = result.unwrap();
        assert_eq!(callees.callees.len(), 1);
        assert_eq!(callees.callees[0].name(), "calledFn");
    }

    #[tokio::test]
    async fn test_query_callees_empty_list_is_a_measured_zero() {
        use lsp_types::request::CallHierarchyPrepare;

        let mock = MockLspClient::new_lenient();
        mock.register_response::<CallHierarchyPrepare>(Some(vec![]));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_callees(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().callees.is_empty());
    }

    // Tests for query_implementations

    #[tokio::test]
    async fn test_query_implementations_scalar() {
        let mock = MockLspClient::new_lenient();

        let impl_loc = lsp_types::Location {
            uri: "file:///test/src/impl.ts".parse().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 5,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 20,
                    character: 1,
                },
            },
        };
        mock.register_response::<GotoImplementation>(Some(GotoDefinitionResponse::Scalar(
            impl_loc,
        )));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("interface.ts");
        std::fs::write(&file_path, "interface Foo { bar(): void; }").unwrap();

        let location = Location::new(file_path.clone(), 1, 11);
        let result = query_implementations(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        let impls = result.unwrap();
        assert_eq!(impls.locations.len(), 1);
    }

    #[tokio::test]
    async fn test_query_implementations_array() {
        let mock = MockLspClient::new_lenient();

        let locs = vec![
            lsp_types::Location {
                uri: "file:///test/src/a.ts".parse().unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 10,
                        character: 1,
                    },
                },
            },
            lsp_types::Location {
                uri: "file:///test/src/b.ts".parse().unwrap(),
                range: lsp_types::Range {
                    start: lsp_types::Position {
                        line: 3,
                        character: 0,
                    },
                    end: lsp_types::Position {
                        line: 15,
                        character: 1,
                    },
                },
            },
        ];
        mock.register_response::<GotoImplementation>(Some(GotoDefinitionResponse::Array(locs)));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("trait.ts");
        std::fs::write(&file_path, "interface Foo {}").unwrap();

        let location = Location::new(file_path.clone(), 1, 11);
        let result = query_implementations(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        let impls = result.unwrap();
        assert_eq!(impls.locations.len(), 2);
    }

    #[tokio::test]
    async fn query_implementations_null_result_is_no_answer_not_empty() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<GotoImplementation>(None::<GotoDefinitionResponse>);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_implementations(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/implementation");
            }
            other => panic!("a wire null must be no-answer, not a measured zero: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_implementations_non_null_empty_stays_ok() {
        let mock = MockLspClient::new_lenient();
        // A non-null empty list is the server answering "I looked; there are
        // no implementations here" — a measured zero, not a refusal. It must
        // stay Ok.
        mock.register_response::<GotoImplementation>(Some(GotoDefinitionResponse::Array(vec![])));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_implementations(&mock, &file_path, &location, None).await;

        assert!(result.is_ok());
        assert!(result.unwrap().locations.is_empty());
    }

    use lsp_types::notification::Notification;
    use lsp_types::request::DocumentDiagnosticRequest;
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    struct ErrorInjectingClient {
        inner: MockLspClient,
        method: &'static str,
        code: i32,
        message: String,
    }

    #[async_trait::async_trait]
    impl LspClientTrait for ErrorInjectingClient {
        async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result>
        where
            R::Params: Serialize + Send,
            R::Result: DeserializeOwned,
        {
            if R::METHOD == self.method {
                return Err(LspError::ResponseError {
                    language: "mock".to_string(),
                    code: self.code,
                    message: self.message.clone(),
                });
            }
            self.inner.request::<R>(params).await
        }

        async fn notify<N: Notification>(&self, params: N::Params) -> Result<()>
        where
            N::Params: Serialize + Send,
        {
            self.inner.notify::<N>(params).await
        }

        async fn did_open(
            &self,
            uri: Uri,
            language_id: String,
            text: String,
        ) -> Result<DocumentOpen> {
            self.inner.did_open(uri, language_id, text).await
        }

        async fn wait_for_diagnostics(&self, uri: &Uri, timeout_ms: u64) -> bool {
            self.inner.wait_for_diagnostics(uri, timeout_ms).await
        }

        async fn record_readiness(&self, uri: &Uri, analyzed: bool) {
            self.inner.record_readiness(uri, analyzed).await
        }

        async fn readiness(&self, uri: &Uri) -> Option<bool> {
            self.inner.readiness(uri).await
        }

        async fn change_text(&self, uri: &Uri, text: &str) -> Result<bool> {
            self.inner.change_text(uri, text).await
        }

        fn is_initialized(&self) -> bool {
            self.inner.is_initialized()
        }

        async fn shutdown(&self) -> Result<()> {
            self.inner.shutdown().await
        }
    }

    #[tokio::test]
    async fn query_diagnostics_null_result_is_no_answer_not_empty() {
        // `DocumentDiagnosticRequest::Result` (`DocumentDiagnosticReportResult`)
        // is not `Option<_>`, so `MockLspClient::register_response` cannot
        // carry a literal wire `null` through the type-checked path. This
        // locally scoped `Request` — same METHOD, `Result =
        // Option<serde_json::Value>` — exists only to plant that `null`
        // under "textDocument/diagnostic".
        struct RawDiagnosticNull;
        impl Request for RawDiagnosticNull {
            type Params = lsp_types::DocumentDiagnosticParams;
            type Result = Option<serde_json::Value>;
            const METHOD: &'static str = "textDocument/diagnostic";
        }

        let mock = MockLspClient::new_lenient();
        mock.register_response::<RawDiagnosticNull>(None);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let result = query_diagnostics(&mock, &file_path).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/diagnostic");
            }
            other => panic!("a wire null must be no-answer, not a measured zero: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_diagnostics_method_not_found_stays_ok_empty() {
        // -32601 (MethodNotFound) is the server's honest "pull diagnostics
        // is not supported here" — an explicitly-supported empty answer, not
        // a refusal.
        let client = ErrorInjectingClient {
            inner: MockLspClient::new_lenient(),
            method: "textDocument/diagnostic",
            code: -32601,
            message: "method not found".to_string(),
        };

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let result = query_diagnostics(&client, &file_path).await;

        assert!(result.is_ok());
        assert!(result.unwrap().diagnostics.is_empty());
    }

    #[tokio::test]
    async fn query_diagnostics_non_null_empty_report_stays_ok() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<DocumentDiagnosticRequest>(
            lsp_types::DocumentDiagnosticReportResult::Report(
                lsp_types::DocumentDiagnosticReport::Full(
                    lsp_types::RelatedFullDocumentDiagnosticReport {
                        related_documents: None,
                        full_document_diagnostic_report: lsp_types::FullDocumentDiagnosticReport {
                            result_id: None,
                            items: vec![],
                        },
                    },
                ),
            ),
        );

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let result = query_diagnostics(&mock, &file_path).await;

        assert!(result.is_ok());
        assert!(result.unwrap().diagnostics.is_empty());
    }

    #[tokio::test]
    async fn query_diagnostics_other_response_error_is_propagated() {
        // Any error other than -32601 is unknown, not "not supported" — it
        // must propagate, not collapse into a measured zero.
        let client = ErrorInjectingClient {
            inner: MockLspClient::new_lenient(),
            method: "textDocument/diagnostic",
            code: -32603, // InternalError
            message: "server exploded".to_string(),
        };

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let result = query_diagnostics(&client, &file_path).await;

        match result {
            Err(LspError::ResponseError { code, .. }) => {
                assert_eq!(code, -32603);
            }
            other => panic!(
                "a non-MethodNotFound error must propagate, not collapse to a measured \
                 zero: {other:?}"
            ),
        }
    }

    // Tests for query_workspace_symbols

    #[tokio::test]
    async fn test_query_workspace_symbols_returns_results() {
        let mock = MockLspClient::new_lenient();

        #[allow(deprecated)]
        let symbols = lsp_types::WorkspaceSymbolResponse::Flat(vec![
            lsp_types::SymbolInformation {
                name: "UserService".to_string(),
                kind: lsp_types::SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                location: lsp_types::Location {
                    uri: "file:///test/src/user.ts".parse().unwrap(),
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: 0,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: 50,
                            character: 1,
                        },
                    },
                },
                container_name: None,
            },
            lsp_types::SymbolInformation {
                name: "UserController".to_string(),
                kind: lsp_types::SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                location: lsp_types::Location {
                    uri: "file:///test/src/controller.ts".parse().unwrap(),
                    range: lsp_types::Range {
                        start: lsp_types::Position {
                            line: 5,
                            character: 0,
                        },
                        end: lsp_types::Position {
                            line: 30,
                            character: 1,
                        },
                    },
                },
                container_name: None,
            },
        ]);
        mock.register_response::<WorkspaceSymbolRequest>(Some(symbols));

        let result = query_workspace_symbols(&mock, "User").await;

        assert!(result.is_ok());
        let syms = result.unwrap();
        assert_eq!(syms.symbols.len(), 2);
        assert_eq!(syms.symbols[0].name(), "UserService");
        assert_eq!(syms.symbols[1].name(), "UserController");
    }

    #[tokio::test]
    async fn query_workspace_symbols_null_result_is_no_answer_not_empty() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<WorkspaceSymbolRequest>(
            None::<lsp_types::WorkspaceSymbolResponse>,
        );

        let result = query_workspace_symbols(&mock, "NonexistentSymbol").await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "workspace/symbol");
            }
            other => panic!("a wire null must be no-answer, not a measured zero: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_workspace_symbols_non_null_empty_stays_ok() {
        let mock = MockLspClient::new_lenient();
        // A non-null empty list is the server answering "I looked; there are
        // no matching symbols" — a measured zero, not a refusal. It must
        // stay Ok.
        mock.register_response::<WorkspaceSymbolRequest>(Some(
            lsp_types::WorkspaceSymbolResponse::Flat(vec![]),
        ));

        let result = query_workspace_symbols(&mock, "NonexistentSymbol").await;

        assert!(result.is_ok());
        assert!(result.unwrap().symbols.is_empty());
    }

    /// Opening a document must be followed by waiting for the server to say it
    /// has analysed it.
    ///
    /// pyright answers a references query issued straight after `didOpen` with
    /// the open file's hits and nothing else, then publishes its diagnostics.
    /// Measured against psf/requests: 0ms settle → 1 reference, 250ms → 4. Every
    /// caller of query_references inherits that, including rename, delete,
    /// inline and combine, which take their edit sites from it.
    #[tokio::test]
    async fn opening_a_document_waits_for_the_analysis_signal() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "const x = 1;\n").unwrap();

        let mock = MockLspClient::new_lenient();
        open_document(&mock, &file).await.unwrap();

        let expected = path_to_uri(&file).unwrap();
        assert_eq!(
            mock.diagnostics_waits(),
            vec![expected.as_str().to_string()],
            "open_document must wait for the document's diagnostics before returning"
        );
    }

    /// The analysis wait is paid once per document per session, not once per
    /// query.
    ///
    /// `query_callees` runs once per declaration, and every call went through
    /// `open_document`, which awaited `wait_for_diagnostics` unconditionally.
    /// The document was already open by then, so `did_open` sent nothing, so no
    /// diagnostics could arrive and the wait was a guaranteed 2 s timeout. At
    /// 1,772 declarations a cold `verify_test_impact` on Textualize/rich took
    /// 3,276.84 s, 98.9% of it inside that wait.
    #[tokio::test]
    async fn a_second_query_on_an_open_document_does_not_re_pay_the_analysis_wait() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "function render() { return 1; }\n").unwrap();
        let position = Location::new(file.clone(), 1, 10);

        let mock = MockLspClient::new_lenient();
        let _ = query_callees(&mock, &file, &position, None).await;
        let _ = query_callees(&mock, &file, &position, None).await;
        let _ = query_callers(&mock, &file, &position, None).await;

        let opens = mock
            .sent_notifications()
            .into_iter()
            .filter(|(method, _)| method == "textDocument/didOpen")
            .count();
        assert_eq!(
            opens, 1,
            "a document already open must not be re-opened; re-opening makes the \
             server discard and redo its analysis"
        );
        assert_eq!(
            mock.diagnostics_waits().len(),
            1,
            "the analysis wait must be paid once per document, not once per query"
        );
    }

    #[tokio::test]
    async fn already_open_after_timed_out_first_open_reports_analysis_timed_out() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "function render() { return 1; }\n").unwrap();

        let mock = MockLspClient::new();
        mock.set_analysis_times_out(true);

        let first = open_document(&mock, &file).await.unwrap();
        assert_eq!(
            first,
            DocumentReady::AnalysisTimedOut,
            "setup fact: the first open's analysis wait timed out"
        );

        let second = open_document(&mock, &file).await.unwrap();
        assert_eq!(
            second,
            DocumentReady::AnalysisTimedOut,
            "re-opening a document whose first analysis wait timed out must \
             still surface the timeout, not claim readiness as AlreadyOpen"
        );
    }

    #[tokio::test]
    async fn already_open_after_analyzed_first_open_stays_already_open() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "function render() { return 1; }\n").unwrap();

        let mock = MockLspClient::new();

        let first = open_document(&mock, &file).await.unwrap();
        assert_eq!(
            first,
            DocumentReady::Analyzed,
            "setup fact: the first open's analysis wait succeeded"
        );

        let second = open_document(&mock, &file).await.unwrap();
        assert_eq!(
            second,
            DocumentReady::AlreadyOpen,
            "an analysed document must keep reporting AlreadyOpen so the \
             analysis wait is never re-paid"
        );
    }

    #[tokio::test]
    async fn document_scoped_queries_open_the_document_first() {
        fn fixture() -> (tempfile::TempDir, PathBuf) {
            let dir = tempdir().unwrap();
            let path = dir.path().join("main.ts");
            std::fs::write(&path, "function myFunction() { return 1; }\n").unwrap();
            (dir, path)
        }

        fn assert_opened(mock: &MockLspClient, file: &Path, capability: &str) {
            let expected = path_to_uri(file).unwrap();
            let opened = mock
                .sent_notifications()
                .into_iter()
                .any(|(method, params)| {
                    method == "textDocument/didOpen"
                        && params["textDocument"]["uri"] == serde_json::json!(expected.as_str())
                });
            assert!(
                opened,
                "{capability} queried the server without opening the document first"
            );
        }

        // The query outcome is irrelevant here: the lenient mock has no canned
        // responses. What is asserted is the notification sent before the request.
        let (_dir, file) = fixture();
        let position = Location::new(file.clone(), 1, 10);

        let mock = MockLspClient::new_lenient();
        let _ = query_callers(&mock, &file, &position, None).await;
        assert_opened(&mock, &file, "query_callers");

        let mock = MockLspClient::new_lenient();
        let _ = query_callees(&mock, &file, &position, None).await;
        assert_opened(&mock, &file, "query_callees");

        let mock = MockLspClient::new_lenient();
        let _ = query_type(&mock, &file, &position).await;
        assert_opened(&mock, &file, "query_type");

        let mock = MockLspClient::new_lenient();
        let _ = validate_rename(&mock, &file, &position, "newName").await;
        assert_opened(&mock, &file, "validate_rename");

        let mock = MockLspClient::new_lenient();
        let range = Range::new(position.clone(), Location::new(file.clone(), 1, 20));
        let _ = query_code_actions(&mock, &file, &range, &[]).await;
        assert_opened(&mock, &file, "query_code_actions");

        // The three that already did this, so a regression is caught too.
        let mock = MockLspClient::new_lenient();
        let _ = query_references(&mock, &file, &position, None).await;
        assert_opened(&mock, &file, "query_references");

        let mock = MockLspClient::new_lenient();
        let _ = query_implementations(&mock, &file, &position, None).await;
        assert_opened(&mock, &file, "query_implementations");

        let mock = MockLspClient::new_lenient();
        let _ = query_diagnostics(&mock, &file).await;
        assert_opened(&mock, &file, "query_diagnostics");
    }

    #[tokio::test]
    async fn query_references_null_result_is_no_answer_not_empty() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<References>(None);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_references(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/references");
            }
            other => panic!("a wire null must be no-answer, not a measured zero: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_code_actions_null_result_is_no_answer_not_empty() {
        let mock = MockLspClient::new_lenient();
        mock.register_response::<CodeActionRequest>(None);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let range = Range::new(
            Location::new(file_path.clone(), 1, 1),
            Location::new(file_path.clone(), 1, 10),
        );
        let result = query_code_actions(&mock, &file_path, &range, &[]).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/codeAction");
            }
            other => panic!("a wire null must be no-answer, not a measured zero: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_code_actions_non_null_empty_stays_ok() {
        let mock = MockLspClient::new_lenient();
        // A non-null empty list is the server answering "I looked; there are
        // no code actions here" — a measured zero, not a refusal. It must
        // stay Ok.
        mock.register_response::<CodeActionRequest>(Some(vec![]));

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let range = Range::new(
            Location::new(file_path.clone(), 1, 1),
            Location::new(file_path.clone(), 1, 10),
        );
        let result = query_code_actions(&mock, &file_path, &range, &[]).await;

        match result {
            Ok(code_actions) => {
                assert!(code_actions.actions.is_empty());
            }
            other => {
                panic!("a non-null empty action list is a measured zero, not no-answer: {other:?}")
            }
        }
    }

    #[tokio::test]
    async fn an_operation_the_server_does_not_advertise_is_refused_unsent() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("main.ts");
        std::fs::write(&file, "function f() {}").unwrap();
        let position = Location::new(file.clone(), 1, 10);

        for operation in ["callers", "callees", "implementations"] {
            let mock = MockLspClient::new_lenient();
            mock.set_server_capabilities(lsp_types::ServerCapabilities::default());
            let result = match operation {
                "callers" => query_callers(&mock, &file, &position, None)
                    .await
                    .map(|_| ()),
                "callees" => query_callees(&mock, &file, &position, None)
                    .await
                    .map(|_| ()),
                _ => query_implementations(&mock, &file, &position, None)
                    .await
                    .map(|_| ()),
            };
            match result {
                Err(e @ LspError::NotAdvertised { .. }) => {
                    let message = e.to_string();
                    assert!(
                        message.contains(operation) && message.contains("does not support"),
                        "{operation}: {message}"
                    );
                }
                other => panic!("{operation}: not advertised, must be refused: {other:?}"),
            }
            assert!(
                mock.sent_requests().is_empty() && mock.sent_notifications().is_empty(),
                "{operation}: nothing may reach the server: {:?} {:?}",
                mock.sent_requests(),
                mock.sent_notifications()
            );
        }
    }

    #[tokio::test]
    async fn an_advertised_operation_is_sent() {
        let temp_dir = tempdir().unwrap();
        let file = temp_dir.path().join("main.ts");
        std::fs::write(&file, "function f() {}").unwrap();
        let position = Location::new(file.clone(), 1, 10);

        let mock = MockLspClient::new_lenient();
        mock.set_server_capabilities(lsp_types::ServerCapabilities {
            call_hierarchy_provider: Some(lsp_types::CallHierarchyServerCapability::Simple(true)),
            ..Default::default()
        });
        let _ = query_callers(&mock, &file, &position, None).await;

        assert!(
            mock.sent_requests()
                .iter()
                .any(|(method, _)| method == "textDocument/prepareCallHierarchy"),
            "{:?}",
            mock.sent_requests()
        );
    }

    #[tokio::test]
    async fn query_callers_null_prepare_is_no_answer() {
        use lsp_types::request::CallHierarchyPrepare;

        let mock = MockLspClient::new_lenient();
        mock.register_response::<CallHierarchyPrepare>(None::<Vec<CallHierarchyItem>>);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "function f() {}").unwrap();

        let location = Location::new(file_path.clone(), 1, 10);
        let result = query_callers(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/prepareCallHierarchy");
            }
            other => panic!("a wire null prepare must be no-answer, not zero callers: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_callers_null_incoming_calls_is_no_answer() {
        use lsp_types::request::{CallHierarchyIncomingCalls, CallHierarchyPrepare};

        let mock = MockLspClient::new_lenient();
        let item = CallHierarchyItem {
            name: "myFunction".to_string(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: "file:///test/src/main.ts".parse().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 9,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 19,
                },
            },
            data: None,
        };
        mock.register_response::<CallHierarchyPrepare>(Some(vec![item]));
        mock.register_response::<CallHierarchyIncomingCalls>(None);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "function myFunction() {}").unwrap();

        let location = Location::new(file_path.clone(), 1, 10);
        let result = query_callers(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "callHierarchy/incomingCalls");
            }
            other => panic!(
                "a per-item wire null must not record a partial caller set as complete: {other:?}"
            ),
        }
    }

    #[tokio::test]
    async fn query_callees_null_prepare_is_no_answer() {
        use lsp_types::request::CallHierarchyPrepare;

        let mock = MockLspClient::new_lenient();
        mock.register_response::<CallHierarchyPrepare>(None::<Vec<CallHierarchyItem>>);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "const x = 1;").unwrap();

        let location = Location::new(file_path.clone(), 1, 7);
        let result = query_callees(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "textDocument/prepareCallHierarchy");
            }
            other => panic!("a wire null prepare must be no-answer, not zero callees: {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_callees_null_outgoing_calls_is_no_answer() {
        use lsp_types::request::{CallHierarchyOutgoingCalls, CallHierarchyPrepare};

        let mock = MockLspClient::new_lenient();
        let item = CallHierarchyItem {
            name: "myFunction".to_string(),
            kind: lsp_types::SymbolKind::FUNCTION,
            tags: None,
            detail: None,
            uri: "file:///test/src/main.ts".parse().unwrap(),
            range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: 5,
                    character: 1,
                },
            },
            selection_range: lsp_types::Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 9,
                },
                end: lsp_types::Position {
                    line: 0,
                    character: 19,
                },
            },
            data: None,
        };
        mock.register_response::<CallHierarchyPrepare>(Some(vec![item]));
        mock.register_response::<CallHierarchyOutgoingCalls>(None);

        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("main.ts");
        std::fs::write(&file_path, "function myFunction() {}").unwrap();

        let location = Location::new(file_path.clone(), 1, 10);
        let result = query_callees(&mock, &file_path, &location, None).await;

        match result {
            Err(LspError::NullResult { method }) => {
                assert_eq!(method, "callHierarchy/outgoingCalls");
            }
            other => panic!(
                "a per-item wire null must not record a partial callee set as complete: {other:?}"
            ),
        }
    }

    #[tokio::test]
    async fn query_callees_refuses_after_a_timed_out_sync_document_open() {
        use lsp_types::request::CallHierarchyPrepare;

        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "function render() { return 1; }\n").unwrap();
        let position = Location::new(file.clone(), 1, 10);

        let mock = MockLspClient::new();
        mock.set_analysis_times_out(true);
        // Canned so the query WOULD succeed if it were issued.
        mock.register_response::<CallHierarchyPrepare>(Some(vec![]));

        sync_document(
            &mock,
            &file,
            "function render() { return 2; }\n".to_string(),
        )
        .await
        .unwrap();

        match query_callees(&mock, &file, &position, None).await {
            Err(LspError::AnalysisNotReady { .. }) => {}
            other => panic!(
                "a document whose sync_document open timed out must refuse a \
                 callee query as not-ready, not answer it: {other:?}"
            ),
        }
    }

    #[tokio::test]
    async fn query_callees_refuses_when_earlier_open_timed_out() {
        use lsp_types::request::CallHierarchyPrepare;

        let dir = tempdir().unwrap();
        let file = dir.path().join("main.ts");
        std::fs::write(&file, "function render() { return 1; }\n").unwrap();
        let position = Location::new(file.clone(), 1, 10);

        let mock = MockLspClient::new();
        mock.set_analysis_times_out(true);
        // Canned so the query WOULD succeed if it were issued.
        mock.register_response::<CallHierarchyPrepare>(Some(vec![]));

        let first = open_document(&mock, &file).await.unwrap();
        assert_eq!(
            first,
            DocumentReady::AnalysisTimedOut,
            "setup fact: the first open's analysis wait timed out"
        );

        let result = query_callees(&mock, &file, &position, None).await;
        match result {
            Err(LspError::AnalysisNotReady { .. }) => {}
            other => panic!(
                "a query on a document whose earlier open timed out must be \
                 refused as not-ready, not answered: {other:?}"
            ),
        }

        assert!(
            !mock
                .sent_requests()
                .iter()
                .any(|(method, _)| method == CallHierarchyPrepare::METHOD),
            "the refusal must happen before any prepareCallHierarchy request \
             reaches the server"
        );
    }
}
