//! LSP client implementation for JSON-RPC over stdio.

use crate::config::ServerConfig;
use crate::error::{LspError, Result};
use crate::servers::LspServer;
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use async_trait::async_trait;
use lsp_types::{
    ClientCapabilities, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, InitializeResult, ServerCapabilities,
    TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    WorkspaceFolder,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Notification,
    },
    request::{Initialize, Request, Shutdown},
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tracing::{debug, error, trace, warn};

/// A document text's fingerprint, to tell whether the server holds it.
pub(crate) fn text_hash(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// A full-text `didChange` for `uri` at the next document version.
pub(crate) fn full_text_change(uri: Uri, text: &str) -> lsp_types::DidChangeTextDocumentParams {
    use std::sync::atomic::AtomicI32;
    // ponytail: one process-wide counter keeps every document's versions
    // strictly increasing past the `didOpen` version 1, across clients and
    // calls; per-document counters only if a server ever objects to gaps.
    // Taken under the document lock, so wire order follows version order.
    static NEXT_VERSION: AtomicI32 = AtomicI32::new(2);
    lsp_types::DidChangeTextDocumentParams {
        text_document: lsp_types::VersionedTextDocumentIdentifier {
            uri,
            version: NEXT_VERSION.fetch_add(1, Ordering::Relaxed),
        },
        content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: text.to_string(),
        }],
    }
}

/// What a `did_open` actually did.
///
/// The distinction is what makes the post-`didOpen` analysis wait affordable:
/// it is worth paying when the server has just been handed a document to
/// analyse, and is a guaranteed timeout when it has not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentOpen {
    /// The document was not open. The notification went out and the server is
    /// analysing it now, so its readiness signal is still to come.
    Opened,
    /// The document was already open on this client. Nothing was sent, and
    /// nothing new will be published for it.
    AlreadyOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentReady {
    /// Freshly opened, and the server published its diagnostics inside the
    /// readiness budget. Answers from this point are from a full analysis.
    Analyzed,
    /// Freshly opened, and the readiness budget ran out before any
    /// `publishDiagnostics` arrived. The server may answer, but from a partial
    /// index — treat its answers as unknown, never as confirmed-empty.
    AnalysisTimedOut,
    AlreadyOpen,
}

/// Trait for LSP client operations.
///
/// This allows for mock implementations in tests.
#[async_trait]
pub trait LspClientTrait: Send + Sync {
    /// Sends a request to the LSP server and waits for a response.
    async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result>
    where
        R::Params: Serialize + Send,
        R::Result: DeserializeOwned;

    /// Sends a notification to the LSP server (no response expected).
    async fn notify<N: Notification>(&self, params: N::Params) -> Result<()>
    where
        N::Params: Serialize + Send;

    /// Notifies the server that a text document was opened.
    ///
    /// Implementations must be idempotent per document and report which case
    /// they took, so a caller can tell a fresh open from a redundant one.
    async fn did_open(&self, uri: Uri, language_id: String, text: String) -> Result<DocumentOpen>;

    /// Waits until the server has published diagnostics for `uri`.
    ///
    /// This is the readiness signal after a `didOpen`. A server that answers a
    /// query before it has analysed the document answers from what it has
    /// bound so far, which for pyright means the open file and nothing else.
    /// Returns false on timeout; the caller proceeds either way.
    async fn wait_for_diagnostics(&self, _uri: &Uri, _timeout_ms: u64) -> bool {
        true
    }

    /// Records the outcome of the readiness wait paid for `uri`'s first open.
    ///
    /// Written by [`crate::capabilities::open_document`] at every fresh open,
    /// immediately after the wait resolves, recording BOTH outcomes: `true`
    /// means the server published diagnostics inside the budget (analyzed),
    /// `false` means the budget ran out.
    async fn record_readiness(&self, uri: &Uri, analyzed: bool);

    async fn readiness(&self, uri: &Uri) -> Option<bool>;

    /// Sends `text` as the full new text of the open document `uri` (a
    /// `didChange` at the next version), unless the server was last sent
    /// that text; returns whether it sent. The comparison, the send and the
    /// record are one step under the per-client document lock, so concurrent
    /// callers cannot leave the server and the record disagreeing. A
    /// document that is not open is left alone (`false`).
    async fn change_text(&self, uri: &Uri, text: &str) -> Result<bool>;

    /// The capabilities the server advertised in its `initialize` result;
    /// `None` when they are not known, and then every request is sent.
    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        None
    }

    /// Checks if the client is initialized.
    fn is_initialized(&self) -> bool;

    /// Shuts down the LSP server.
    async fn shutdown(&self) -> Result<()>;
}

/// LSP client that communicates with a language server over JSON-RPC.
pub struct LspClient {
    /// Language this client is for (for error messages).
    language: String,

    /// The child process running the LSP server.
    process: Arc<Mutex<Option<Child>>>,

    #[cfg(unix)]
    process_group: Option<i32>,

    /// Channel for sending messages to the write task.
    message_tx: mpsc::UnboundedSender<OutgoingMessage>,

    /// Pending requests awaiting responses.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,

    /// Next request ID.
    next_id: Arc<AtomicU64>,

    /// Whether the client is initialized.
    initialized: Arc<AtomicBool>,

    /// Request timeout.
    timeout_ms: u64,

    /// Server-specific `initializationOptions`, sent with the initialize request.
    initialization_options: serde_json::Value,

    /// Broadcast of every notification the server sends, by (method, params).
    notifications: broadcast::Sender<(String, serde_json::Value)>,

    /// Work-done progress tokens the server has begun and not yet ended.
    /// Non-empty means the server is still building its index.
    active_progress: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Set once the first readiness wait has run; it is a per-connection
    /// concern, not a per-query one.
    settled: Arc<AtomicBool>,

    /// Documents already sent with `didOpen`. Re-opening a document makes the
    /// server discard and redo its analysis of it, so a query that opens what
    /// warming already opened pays for the same work twice and can time out
    /// waiting for an answer the server had moments ago.
    /// Each holds the [`text_hash`] of the text the server was last sent.
    opened: Arc<Mutex<HashMap<String, u64>>>,

    /// Outcome of the readiness wait paid at each document's first open,
    /// keyed by `uri.as_str()` exactly as `opened` is: `true` = the wait saw
    /// `publishDiagnostics` inside the budget, `false` = the budget ran out.
    /// Same lifetime as `opened`, so an already-open document always has its
    /// earlier wait's outcome here.
    readiness: Arc<Mutex<HashMap<String, bool>>>,

    /// The capabilities from the server's `initialize` result.
    server_capabilities: std::sync::OnceLock<ServerCapabilities>,
}

/// Builds the `initialize` request parameters.
///
/// Split out so the parameters can be asserted without a live server process.
fn build_initialize_params(
    workspace_root: &Path,
    root_uri: Uri,
    initialization_options: &serde_json::Value,
) -> InitializeParams {
    InitializeParams {
        process_id: Some(std::process::id()),
        #[allow(deprecated)]
        root_uri: Some(root_uri.clone()),
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: root_uri,
            name: workspace_root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
        }]),
        // Ask for work-done progress. Without it a server never tells us it is
        // still indexing, and a query issued mid-index gets answered from a
        // partial index — rust-analyzer returned zero callers for a symbol with
        // three, silently, on a warm connection.
        // Declaring this makes servers report indexing as work-done progress,
        // which is the only reliable "am I ready" signal. It also obliges us to
        // answer `window/workDoneProgress/create`; see read_loop.
        capabilities: ClientCapabilities {
            window: Some(lsp_types::WindowClientCapabilities {
                work_done_progress: Some(true),
                ..Default::default()
            }),
            // typescript-language-server advertises `callHierarchyProvider`
            // only to a client that declares it (callers and callees need it),
            // and publishes diagnostics, the readiness signal, only to one that
            // declares `publishDiagnostics`.
            text_document: Some(lsp_types::TextDocumentClientCapabilities {
                call_hierarchy: Some(lsp_types::CallHierarchyClientCapabilities {
                    dynamic_registration: Some(false),
                }),
                publish_diagnostics: Some(Default::default()),
                ..Default::default()
            }),
            ..Default::default()
        },
        // Servers read their behaviour out of this. pyright's `diagnosticMode`
        // lives here, and it decides whether a references query searches the
        // workspace or only the documents the client has opened.
        initialization_options: match initialization_options {
            serde_json::Value::Null => None,
            options => Some(options.clone()),
        },
        ..Default::default()
    }
}

/// Internal message type for outgoing messages.
enum OutgoingMessage {
    /// A request that expects a response.
    Request(JsonRpcRequest),
    /// A notification that does not expect a response.
    Notification(JsonRpcNotification),
    /// A pre-serialized message — a reply to a server-to-client request.
    Raw(String),
}

impl LspClient {
    /// Spawns a new LSP server process and creates a client to communicate with it.
    ///
    /// # Security
    ///
    /// The config is validated before spawning to prevent command injection.
    /// Only commands in the allowed list can be executed.
    pub async fn spawn(language: String, config: ServerConfig) -> Result<Self> {
        // Validate config to prevent command injection
        config.validate()?;

        debug!(
            "Spawning LSP server for {}: {} {:?}",
            language, config.command, config.args
        );

        // Spawn the LSP server process
        let mut command = Command::new(&config.command);
        command
            .args(&config.args)
            .current_dir(
                config
                    .working_directory
                    .as_ref()
                    .unwrap_or(&PathBuf::from(".")),
            )
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        let mut child = crate::watchdog::spawn_with_parent_death_guard(command, std::process::id())
            .map_err(|e| LspError::ServerSpawnFailed {
                language: language.clone(),
                source: e,
            })?;
        #[cfg(not(unix))]
        let mut child = command.spawn().map_err(|e| LspError::ServerSpawnFailed {
            language: language.clone(),
            source: e,
        })?;

        // With the guard's `process_group(0)` the child's pid IS its
        // process-group id.
        #[cfg(unix)]
        let process_group = child.id().map(|id| id as i32);

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LspError::CommunicationError {
                language: language.clone(),
                message: "failed to capture stdin".to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LspError::CommunicationError {
                language: language.clone(),
                message: "failed to capture stdout".to_string(),
            })?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| LspError::CommunicationError {
                language: language.clone(),
                message: "failed to capture stderr".to_string(),
            })?;

        let (message_tx, message_rx) = mpsc::unbounded_channel();
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let (notifications, _) = broadcast::channel(256);

        // Spawn read task to process responses from the server
        let read_pending = Arc::clone(&pending);
        let read_language = language.clone();
        let read_notifications = notifications.clone();
        let read_reply_tx = message_tx.clone();
        let active_progress = Arc::new(Mutex::new(first_build_progress(&language)));
        let read_active_progress = Arc::clone(&active_progress);
        tokio::spawn(async move {
            Self::read_loop(
                stdout,
                read_pending,
                read_language,
                read_notifications,
                read_reply_tx,
                read_active_progress,
            )
            .await;
        });

        // Spawn write task to send messages to the server
        let write_language = language.clone();
        tokio::spawn(async move {
            Self::write_loop(stdin, message_rx, write_language).await;
        });

        // Spawn stderr logging task
        let stderr_language = language.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!("LSP stderr [{}]: {}", stderr_language, line);
            }
        });

        Ok(Self {
            language,
            process: Arc::new(Mutex::new(Some(child))),
            #[cfg(unix)]
            process_group,
            message_tx,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: config.timeout_ms,
            initialization_options: config.initialization_options.clone(),
            notifications,
            active_progress,
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Initializes the LSP client by sending the initialize request.
    pub async fn initialize(&self, workspace_root: PathBuf) -> Result<InitializeResult> {
        debug!("Initializing LSP client for {}", self.language);

        let root_uri = crate::types::path_to_uri(&workspace_root)?;

        let params =
            build_initialize_params(&workspace_root, root_uri, &self.initialization_options);

        let result: InitializeResult = self.request::<Initialize>(params).await?;
        let _ = self.server_capabilities.set(result.capabilities.clone());
        self.initialized.store(true, Ordering::SeqCst);

        // Send initialized notification
        self.notify::<lsp_types::notification::Initialized>(lsp_types::InitializedParams {})
            .await?;

        debug!("LSP client initialized for {}", self.language);
        Ok(result)
    }

    // `did_open` is deliberately NOT an inherent method. It lives only on
    // `LspClientTrait` (see the impl below), which tracks `opened` and returns
    // `DocumentOpen` so callers can tell a real open from a no-op. An inherent
    // copy existed here and did neither; Rust prefers an inherent method over
    // a trait one, so any caller holding a concrete `&LspClient` would have
    // silently got the non-deduping version and re-opened documents the server
    // already had — making the server discard and redo its analysis, which is
    // the cost D2 exists to remove. One definition, no shadowing.

    /// Notifies the server that a text document was changed.
    pub async fn did_change(
        &self,
        uri: Uri,
        version: i32,
        changes: Vec<lsp_types::TextDocumentContentChangeEvent>,
    ) -> Result<()> {
        self.notify::<DidChangeTextDocument>(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier { uri, version },
            content_changes: changes,
        })
        .await
    }

    /// Notifies the server that a text document was closed.
    pub async fn did_close(&self, uri: Uri) -> Result<()> {
        self.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
        })
        .await
    }

    /// Read loop that processes responses from the LSP server.
    async fn read_loop(
        stdout: impl tokio::io::AsyncRead + Unpin,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
        language: String,
        notifications: broadcast::Sender<(String, serde_json::Value)>,
        reply_tx: mpsc::UnboundedSender<OutgoingMessage>,
        active_progress: Arc<Mutex<std::collections::HashSet<String>>>,
    ) {
        let mut reader = BufReader::new(stdout);

        loop {
            // Read headers until we find Content-Length
            let mut content_length: Option<usize> = None;
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => return, // EOF
                    Ok(_) => {}
                    Err(e) => {
                        error!("Failed to read LSP header: {}", e);
                        return;
                    }
                }

                let line = line.trim();
                if line.is_empty() {
                    break; // End of headers
                }

                if let Some(len_str) = line.strip_prefix("Content-Length:")
                    && let Ok(len) = len_str.trim().parse()
                {
                    content_length = Some(len);
                }
            }

            let content_length = match content_length {
                Some(len) => len,
                None => {
                    warn!("No Content-Length header found");
                    continue;
                }
            };

            // Read the JSON content
            let mut buffer = vec![0u8; content_length];
            if let Err(e) = tokio::io::AsyncReadExt::read_exact(&mut reader, &mut buffer).await {
                error!("Failed to read LSP message body: {}", e);
                return;
            }

            let content = match String::from_utf8(buffer) {
                Ok(s) => s,
                Err(e) => {
                    error!("Invalid UTF-8 in LSP message: {}", e);
                    continue;
                }
            };

            trace!("LSP recv [{}]: {}", language, content);

            // Server-to-client requests carry BOTH an id and a method, and a
            // JsonRpcResponse deserializes from one because result and error
            // are optional — so this must be tried first or every server
            // request is silently swallowed as an unmatched response.
            let is_server_request = serde_json::from_str::<serde_json::Value>(&content)
                .map(|v| v.get("id").is_some() && v.get("method").is_some())
                .unwrap_or(false);

            if is_server_request {
                if let Ok(request) = serde_json::from_str::<ServerRequest>(&content) {
                    trace!("Server request {} (id {})", request.method, request.id);
                    // Answer with the id as received: JSON-RPC ids are numbers
                    // or strings (sourcekit-lsp sends "sk-0").
                    let response = if request.method == "window/workDoneProgress/create" {
                        serde_json::json!({"jsonrpc": "2.0", "id": request.id, "result": null})
                    } else {
                        serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": request.id,
                            "error": crate::types::JsonRpcError {
                                code: -32601,
                                message: format!("method not supported by act: {}", request.method),
                                data: None,
                            },
                        })
                    };
                    if let Ok(json) = serde_json::to_string(&response) {
                        let _ = reply_tx.send(OutgoingMessage::Raw(json));
                    }
                }
            } else if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&content) {
                trace!("Received response for ID {}", response.id);
                let mut pending_lock = pending.lock().await;
                if let Some(tx) = pending_lock.remove(&response.id) {
                    let _ = tx.send(response);
                }
            } else if let Ok(notification) = serde_json::from_str::<JsonRpcNotification>(&content) {
                trace!("Received notification {}", notification.method);
                // Track work-done progress here, from the first message on, and
                // before the broadcast: a begin sent before wait_for_indexing
                // subscribes must still count.
                if notification.method == "$/progress"
                    && let Some(token) = notification.params.get("token").map(|t| t.to_string())
                {
                    let kind = notification
                        .params
                        .get("value")
                        .and_then(|v| v.get("kind"))
                        .and_then(|k| k.as_str())
                        .unwrap_or("");
                    let mut active = active_progress.lock().await;
                    match kind {
                        "begin" => {
                            active.insert(token);
                        }
                        "end" => {
                            active.remove(&token);
                        }
                        _ => {}
                    }
                }
                if first_build_end(&language).is_some_and(|(method, name)| {
                    notification.method == method
                        && notification.params.get("name").and_then(|n| n.as_str()) == Some(name)
                }) {
                    active_progress.lock().await.remove(FIRST_BUILD_TOKEN);
                }
                // A send error only means nobody is currently waiting.
                let _ = notifications.send((notification.method, notification.params));
            } else {
                warn!("Failed to parse LSP message: {}", content);
            }
        }
    }

    /// Write loop that sends messages to the LSP server.
    async fn write_loop(
        mut stdin: impl tokio::io::AsyncWrite + Unpin,
        mut message_rx: mpsc::UnboundedReceiver<OutgoingMessage>,
        language: String,
    ) {
        while let Some(msg) = message_rx.recv().await {
            let json = match &msg {
                OutgoingMessage::Request(req) => match serde_json::to_string(req) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Failed to serialize request: {}", e);
                        continue;
                    }
                },
                OutgoingMessage::Notification(notif) => match serde_json::to_string(notif) {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Failed to serialize notification: {}", e);
                        continue;
                    }
                },
                OutgoingMessage::Raw(json) => json.clone(),
            };

            let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
            trace!("LSP send [{}]: {}", language, json);

            if let Err(e) = stdin.write_all(message.as_bytes()).await {
                error!("Failed to write to LSP server: {}", e);
                break;
            }

            if let Err(e) = stdin.flush().await {
                error!("Failed to flush LSP stdin: {}", e);
                break;
            }
        }

        debug!("LSP write loop ended for {}", language);
    }
}

/// A request sent by the server to the client.
#[derive(serde::Deserialize)]
struct ServerRequest {
    id: serde_json::Value,
    method: String,
}

/// JSON-RPC error code for LSP's `ContentModified`.
///
/// The spec says the client should resend the request: the server's state
/// changed underneath it. rust-analyzer returns this for every query issued
/// while it is still loading the workspace.
const LSP_CONTENT_MODIFIED: i32 = -32801;

/// How long to keep resending a request the server is not ready to answer —
/// ContentModified, or a timeout while it is still warming up.
/// Kept under the MCP client's own patience: a request that stalls past that
/// is backgrounded by the client, and the agent gives up on the tool and edits
/// by hand — which is worse than a fast, honest error.
const CONTENT_MODIFIED_BUDGET_MS: u64 = 45_000;

const CONTENT_MODIFIED_BACKOFF_START_MS: u64 = 200;
const CONTENT_MODIFIED_BACKOFF_MAX_MS: u64 = 2_000;

/// How long to let a server settle before the first query, once per client.
///
/// Short on purpose. A generous budget does not buy a better answer, it buys a
/// stall: waiting on progress that never drains made every query — CLI included
/// — hang for the whole budget. Warming is what removes the cold start; this is
/// only a guard against querying a server that has not drawn breath.
const INDEXING_BUDGET_MS: u64 = 10_000;

/// Total wait before the first query of a connection, from didOpen: the
/// settle wait, the diagnostics wait and the wait for indexing begun after
/// didOpen together. sourcekit-lsp builds a cold SwiftPM package before it
/// indexes: 14.6 s for a two-file fixture on an idle machine. Kept under
/// `CONTENT_MODIFIED_BUDGET_MS`, the MCP client's patience, with room left
/// for the query itself.
const FIRST_OPEN_WAIT_BUDGET_MS: u64 = 30_000;

/// The progress token that stands for a server's first build when the server
/// reports it by [`LspServer::build_end_notification`], not by `$/progress`.
/// `$/progress` tokens are stored as JSON text, so this one cannot collide.
const FIRST_BUILD_TOKEN: &str = "first build";

/// The first-build notification of the server whose binary is `language`.
fn first_build_end(language: &str) -> Option<(&'static str, &'static str)> {
    LspServer::ALL
        .into_iter()
        .find(|server| server.binary_name() == language)
        .and_then(LspServer::build_end_notification)
}

/// The work-done progress a client holds from spawn: the server's first
/// build, when the server ends it by a notification and not by `$/progress`.
/// The first-open wait then holds the first query until the build ends.
fn first_build_progress(language: &str) -> std::collections::HashSet<String> {
    first_build_end(language)
        .map(|_| FIRST_BUILD_TOKEN.to_string())
        .into_iter()
        .collect()
}

impl LspClient {
    /// Waits until the server reports no work in flight, or the budget expires.
    /// Returns immediately when the server never began any work.
    async fn wait_for_indexing(&self, budget_ms: u64) -> bool {
        let mut rx = self.notifications.subscribe();
        let deadline = Duration::from_millis(budget_ms);
        let drained = timeout(deadline, async {
            loop {
                if self.active_progress.lock().await.is_empty() {
                    // Give a beat for a begin that has not arrived yet, then
                    // accept an idle server.
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    if self.active_progress.lock().await.is_empty() {
                        return true;
                    }
                }
                // read_loop updates active_progress before it broadcasts, so
                // any notification is a cue to check it again.
                match rx.recv().await {
                    Ok(_) | Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => return false,
                }
            }
        })
        .await;

        match drained {
            Ok(done) => done,
            Err(_) => {
                debug!(
                    "{} still reports work in flight after {}ms; querying anyway",
                    self.language, budget_ms
                );
                false
            }
        }
    }

    /// Sends one request and waits for its response. No retry: `request`
    /// owns the ContentModified policy.
    async fn request_value<R: Request>(&self, params_value: serde_json::Value) -> Result<R::Result>
    where
        R::Result: DeserializeOwned,
    {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, R::METHOD.to_string(), params_value);

        let (response_tx, response_rx) = oneshot::channel();

        // Register pending request
        self.pending.lock().await.insert(id, response_tx);

        // Send request to write loop
        self.message_tx
            .send(OutgoingMessage::Request(request))
            .map_err(|_| LspError::ServerTerminated {
                language: self.language.clone(),
            })?;

        // Wait for response with timeout
        let response = match timeout(Duration::from_millis(self.timeout_ms), response_rx).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                // Channel closed - server terminated
                self.pending.lock().await.remove(&id);
                return Err(LspError::ServerTerminated {
                    language: self.language.clone(),
                });
            }
            Err(_) => {
                // Timeout - clean up pending request to prevent memory leak
                self.pending.lock().await.remove(&id);
                return Err(LspError::Timeout {
                    timeout_ms: self.timeout_ms,
                });
            }
        };

        // Check for error response
        if let Some(error) = response.error {
            return Err(LspError::ResponseError {
                language: self.language.clone(),
                code: error.code,
                message: error.message,
            });
        }

        // Deserialize result.
        //
        // A response carrying no `result` member is the wire form of `null`,
        // which the spec makes a legal ANSWER for every request it declares as
        // `T | null` — `callHierarchy/outgoingCalls` returns null for a
        // declaration that calls nothing, and `textDocument/hover` for a
        // position with nothing to show. Rejecting it outright reported those
        // answers as server failures.
        //
        // So let the target type decide rather than the presence of the member:
        // `Option<T>` reads null as `None`, while a `Vec<T>` or a required
        // struct still fails — and there null IS a protocol error.
        let Some(result) = response.result else {
            return serde_json::from_value(serde_json::Value::Null).map_err(|source| {
                LspError::ResponseError {
                    language: self.language.clone(),
                    code: -1,
                    message: format!(
                        "{} answered {} with no result, but the LSP spec declares a \
                         non-null result for it ({source}); the server is not \
                         answering this request correctly — check its version and logs",
                        self.language,
                        R::METHOD,
                    ),
                }
            });
        };

        serde_json::from_value(result).map_err(Into::into)
    }
}

#[async_trait]
impl LspClientTrait for LspClient {
    fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.get()
    }

    async fn wait_for_diagnostics(&self, uri: &Uri, timeout_ms: u64) -> bool {
        // Subscribe BEFORE waiting on anything: a broadcast receiver only sees
        // what is sent after it subscribes, and the diagnostics for this
        // document can land while the indexing wait below is still running.
        let mut rx = self.notifications.subscribe();

        // A server that is still indexing answers from a partial index, which
        // looks like a real answer: rust-analyzer reported zero callers for a
        // symbol with three. Wait for its work-done progress to drain first,
        // on a budget of its own — a cold large workspace takes far longer to
        // index than to publish diagnostics for one file.
        let first_open = !self.settled.swap(true, Ordering::SeqCst);
        let first_open_deadline =
            tokio::time::Instant::now() + Duration::from_millis(FIRST_OPEN_WAIT_BUDGET_MS);
        let left = || {
            first_open_deadline
                .saturating_duration_since(tokio::time::Instant::now())
                .as_millis() as u64
        };
        if first_open {
            self.wait_for_indexing(INDEXING_BUDGET_MS.min(left())).await;
        }
        let wanted = uri.as_str();
        let deadline = Duration::from_millis(if first_open {
            timeout_ms.min(left())
        } else {
            timeout_ms
        });

        let matched = timeout(deadline, async {
            loop {
                match rx.recv().await {
                    Ok((method, params)) => {
                        if method == "textDocument/publishDiagnostics"
                            && params.get("uri").and_then(|u| u.as_str()) == Some(wanted)
                        {
                            return true;
                        }
                    }
                    // Lagged past our slot: something was published, so treat
                    // the wait as satisfied rather than stalling for the full
                    // timeout.
                    Err(broadcast::error::RecvError::Lagged(_)) => return true,
                    Err(broadcast::error::RecvError::Closed) => return false,
                }
            }
        })
        .await;

        // sourcekit-lsp begins indexing about a second after the first didOpen,
        // after the settle wait above saw an idle server, and publishes no
        // diagnostics until it is done; a query sent before then gets `[]`.
        // Give the same settle beat as wait_for_indexing for a begin that has
        // not arrived yet.
        let drained = first_open && {
            let beat = Duration::from_millis(150).min(Duration::from_millis(left()));
            if self.active_progress.lock().await.is_empty() {
                tokio::time::sleep(beat).await;
            }
            !self.active_progress.lock().await.is_empty() && self.wait_for_indexing(left()).await
        };

        match matched {
            Ok(found) => found || drained,
            Err(_) if drained => true,
            Err(_) => {
                debug!(
                    "No diagnostics for {} within {}ms; querying anyway",
                    uri.as_str(),
                    timeout_ms
                );
                false
            }
        }
    }

    async fn record_readiness(&self, uri: &Uri, analyzed: bool) {
        self.readiness
            .lock()
            .await
            .insert(uri.as_str().to_string(), analyzed);
    }

    async fn readiness(&self, uri: &Uri) -> Option<bool> {
        self.readiness.lock().await.get(uri.as_str()).copied()
    }

    async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result>
    where
        R::Params: Serialize + Send,
        R::Result: DeserializeOwned,
    {
        let params_value = serde_json::to_value(params)?;
        let mut backoff = Duration::from_millis(CONTENT_MODIFIED_BACKOFF_START_MS);
        let mut spent = Duration::ZERO;
        let budget = Duration::from_millis(CONTENT_MODIFIED_BUDGET_MS);

        loop {
            let attempt = self.request_value::<R>(params_value.clone()).await;
            match attempt {
                // A timeout on a cold server is not a failure, it is "not yet".
                // pyright took longer than the per-request cap to answer
                // `references` over a freshly opened workspace; the agent got a
                // hard error and hand-edited fifty sites instead.
                Err(LspError::Timeout { .. }) if spent + backoff < budget => {
                    debug!(
                        "{} timed out on {}; resending in {:?}",
                        self.language,
                        R::METHOD,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                    spent += backoff;
                    backoff =
                        (backoff * 2).min(Duration::from_millis(CONTENT_MODIFIED_BACKOFF_MAX_MS));
                }
                Err(LspError::ResponseError {
                    code: LSP_CONTENT_MODIFIED,
                    ..
                }) if spent + backoff < budget => {
                    debug!(
                        "{} is still loading (ContentModified); resending {} in {:?}",
                        self.language,
                        R::METHOD,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                    spent += backoff;
                    backoff =
                        (backoff * 2).min(Duration::from_millis(CONTENT_MODIFIED_BACKOFF_MAX_MS));
                }
                other => return other,
            }
        }
    }

    async fn notify<N: Notification>(&self, params: N::Params) -> Result<()>
    where
        N::Params: Serialize + Send,
    {
        let params_value = serde_json::to_value(params)?;
        let notification = JsonRpcNotification::new(N::METHOD.to_string(), params_value);

        trace!("LSP notify [{}]: {}", self.language, N::METHOD);

        // Send notification through the unified message channel
        self.message_tx
            .send(OutgoingMessage::Notification(notification))
            .map_err(|_| LspError::ServerTerminated {
                language: self.language.clone(),
            })?;

        Ok(())
    }

    async fn did_open(&self, uri: Uri, language_id: String, text: String) -> Result<DocumentOpen> {
        // The lock is held across the send, so a concurrent `change_text`
        // cannot reach the server before this `didOpen`.
        let mut opened = self.opened.lock().await;
        if opened.contains_key(uri.as_str()) {
            trace!("{} already open in {}", uri.as_str(), self.language);
            return Ok(DocumentOpen::AlreadyOpen);
        }
        let key = uri.as_str().to_string();
        let hash = text_hash(&text);
        self.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id,
                version: 1,
                text,
            },
        })
        .await?;
        opened.insert(key, hash);
        Ok(DocumentOpen::Opened)
    }

    async fn change_text(&self, uri: &Uri, text: &str) -> Result<bool> {
        let hash = text_hash(text);
        let mut opened = self.opened.lock().await;
        match opened.get(uri.as_str()) {
            None => return Ok(false),
            Some(sent) if *sent == hash => return Ok(false),
            Some(_) => {}
        }
        self.notify::<lsp_types::notification::DidChangeTextDocument>(full_text_change(
            uri.clone(),
            text,
        ))
        .await?;
        opened.insert(uri.as_str().to_string(), hash);
        Ok(true)
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    async fn shutdown(&self) -> Result<()> {
        debug!("Shutting down LSP client for {}", self.language);

        if let Err(e) = self.request::<Shutdown>(()).await {
            debug!(
                "{}: graceful LSP shutdown request failed ({e}); killing",
                self.language
            );
        } else if let Err(e) = self.notify::<lsp_types::notification::Exit>(()).await {
            debug!(
                "{}: LSP exit notification failed ({e}); killing",
                self.language
            );
        }

        // Signal the whole process group, then reap the direct child. The group
        // is what catches the workers the server forked.
        #[cfg(unix)]
        if let Some(pgid) = self.process_group {
            signal_process_group(pgid, libc::SIGTERM);
            // Brief grace period for a clean exit before escalating.
            tokio::time::sleep(Duration::from_millis(200)).await;
            signal_process_group(pgid, libc::SIGKILL);
        }

        // `kill` also waits, which reaps the child rather than leaving a zombie.
        if let Some(mut child) = self.process.lock().await.take() {
            let _ = child.kill().await;
        }

        Ok(())
    }
}

/// Sends `sig` to every process in group `pgid`.
///
/// Guarded against `pgid <= 0`: `killpg(0, …)` would signal act's OWN process
/// group, and negative ids are not groups.
#[cfg(unix)]
fn signal_process_group(pgid: i32, sig: i32) {
    if pgid <= 0 {
        return;
    }
    // SAFETY: `killpg` is a plain syscall; an invalid pgid yields ESRCH, which
    // is ignored. No memory is touched.
    unsafe {
        libc::killpg(pgid, sig);
    }
}

impl Drop for LspClient {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pgid) = self.process_group {
            signal_process_group(pgid, libc::SIGKILL);
        }

        // Still take the handle so the child is reaped where a runtime exists.
        let process = Arc::clone(&self.process);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Some(mut child) = process.lock().await.take() {
                    let _ = child.kill().await;
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::JsonRpcError;
    use std::str::FromStr;

    fn create_test_client() -> (LspClient, mpsc::UnboundedReceiver<OutgoingMessage>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };
        (client, rx)
    }

    #[tokio::test]
    async fn request_resends_while_the_server_reports_content_modified() {
        let (client, mut rx) = create_test_client();
        let pending = Arc::clone(&client.pending);

        // Fake server: ContentModified twice, then a result.
        let server = tokio::spawn(async move {
            let mut answered = 0;
            while let Some(msg) = rx.recv().await {
                let OutgoingMessage::Request(req) = msg else {
                    continue;
                };
                let response = if answered < 2 {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: LSP_CONTENT_MODIFIED,
                            message: "content modified".to_string(),
                            data: None,
                        }),
                    }
                } else {
                    JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: req.id,
                        result: Some(serde_json::json!(null)),
                        error: None,
                    }
                };
                answered += 1;
                if let Some(tx) = pending.lock().await.remove(&req.id) {
                    let _ = tx.send(response);
                }
                if answered > 2 {
                    break;
                }
            }
            answered
        });

        let result: Result<Option<Vec<lsp_types::Location>>> = client
            .request::<lsp_types::request::References>(lsp_types::ReferenceParams {
                text_document_position: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: Uri::from_str("file:///tmp/main.rs").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: lsp_types::ReferenceContext {
                    include_declaration: true,
                },
            })
            .await;

        assert!(result.is_ok(), "retry should outlast the loading window");
        assert_eq!(
            server.await.unwrap(),
            3,
            "two ContentModified answers should produce two resends"
        );
    }

    /// Builds the request whose result type the LSP spec declares nullable.
    fn outgoing_calls_params() -> lsp_types::CallHierarchyOutgoingCallsParams {
        let zero = lsp_types::Position {
            line: 0,
            character: 0,
        };
        lsp_types::CallHierarchyOutgoingCallsParams {
            item: lsp_types::CallHierarchyItem {
                name: "render".to_string(),
                kind: lsp_types::SymbolKind::FUNCTION,
                tags: None,
                detail: None,
                uri: Uri::from_str("file:///tmp/rich/console.py").unwrap(),
                range: lsp_types::Range {
                    start: zero,
                    end: zero,
                },
                selection_range: lsp_types::Range {
                    start: zero,
                    end: zero,
                },
                data: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        }
    }

    /// Answers every request with the wire form `{"result": null}`.
    ///
    /// Built by parsing the bytes a server sends, not by constructing the
    /// struct: `result` is `Option<Value>`, so serde reads a JSON `null` as
    /// `None`, and that — not `Some(Value::Null)` — is the shape the handling
    /// under test has to get right.
    fn answer_with_null(
        mut rx: mpsc::UnboundedReceiver<OutgoingMessage>,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let OutgoingMessage::Request(req) = msg else {
                    continue;
                };
                let wire = format!(r#"{{"jsonrpc":"2.0","id":{},"result":null}}"#, req.id);
                let response: JsonRpcResponse = serde_json::from_str(&wire).unwrap();
                assert!(
                    response.result.is_none(),
                    "a wire `result: null` must parse to None, or this test proves nothing"
                );
                if let Some(tx) = pending.lock().await.remove(&req.id) {
                    let _ = tx.send(response);
                }
            }
        })
    }

    /// A `null` result is an answer, not a failure.
    ///
    /// `callHierarchy/outgoingCalls` is declared `CallHierarchyOutgoingCall[] |
    /// null`, so `null` means "this declaration calls nothing". pyright answers
    /// that way for at least one declaration in every file of Textualize/rich;
    /// treating it as an error marked all 154 files incomplete and left the
    /// persistent callee index holding zero cached lists.
    ///
    /// The discrimination has to come from the target type, not from a blanket
    /// "missing result is fine": a request whose result type is a required
    /// struct must still fail, because there `null` really is a protocol error.
    #[tokio::test]
    async fn a_null_result_is_an_answer_for_a_nullable_result_type() {
        let (client, rx) = create_test_client();
        let server = answer_with_null(rx, Arc::clone(&client.pending));

        let nullable: Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>> = client
            .request::<lsp_types::request::CallHierarchyOutgoingCalls>(outgoing_calls_params())
            .await;

        assert_eq!(
            nullable.expect("a null outgoingCalls result means 'no outgoing calls'"),
            None,
            "null must deserialise into the nullable result type"
        );

        let required: Result<lsp_types::InitializeResult> = client
            .request::<lsp_types::request::Initialize>(InitializeParams::default())
            .await;

        assert!(
            required.is_err(),
            "null is not a legal `initialize` result and must stay an error"
        );

        server.abort();
    }

    /// The mock must not model success where the real client fails, or the
    /// reverse: every test that stands in a mock for a server is only worth the
    /// fidelity of that substitution.
    ///
    /// This is the exact pairing that hid the null-result defect — the mock
    /// deserialised `null` into the result type while the real client rejected
    /// it before looking at the type at all.
    #[tokio::test]
    async fn mock_and_client_agree_on_a_null_result() {
        let (client, rx) = create_test_client();
        let server = answer_with_null(rx, Arc::clone(&client.pending));
        // A lenient mock with no canned response answers null, same as above.
        let mock = crate::mock::MockLspClient::new_lenient();

        let real: Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>> = client
            .request::<lsp_types::request::CallHierarchyOutgoingCalls>(outgoing_calls_params())
            .await;
        let mocked: Result<Option<Vec<lsp_types::CallHierarchyOutgoingCall>>> = mock
            .request::<lsp_types::request::CallHierarchyOutgoingCalls>(outgoing_calls_params())
            .await;
        assert_eq!(
            real.is_ok(),
            mocked.is_ok(),
            "mock and client disagree on a null outgoingCalls result: \
             client {real:?}, mock {mocked:?}"
        );

        let real: Result<lsp_types::InitializeResult> = client
            .request::<lsp_types::request::Initialize>(InitializeParams::default())
            .await;
        let mocked: Result<lsp_types::InitializeResult> = mock
            .request::<lsp_types::request::Initialize>(InitializeParams::default())
            .await;
        assert_eq!(
            real.is_ok(),
            mocked.is_ok(),
            "mock and client disagree on a null initialize result: \
             client {real:?}, mock {mocked:?}"
        );

        server.abort();
    }

    /// The readiness wait must resolve on the matching publishDiagnostics, and
    /// give up on its own if the server never sends one.
    #[tokio::test]
    async fn wait_for_diagnostics_resolves_and_times_out() {
        let (client, _rx) = create_test_client();
        let uri: Uri = Uri::from_str("file:///tmp/main.py").unwrap();
        let other: Uri = Uri::from_str("file:///tmp/other.py").unwrap();

        // Nothing published: bounded wait, then false.
        assert!(!client.wait_for_diagnostics(&uri, 50).await);

        // A publish for a different document does not satisfy the wait.
        let publisher = client.notifications.clone();
        let uri_str = uri.as_str().to_string();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = publisher.send((
                "textDocument/publishDiagnostics".to_string(),
                serde_json::json!({"uri": "file:///tmp/unrelated.py", "diagnostics": []}),
            ));
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = publisher.send((
                "textDocument/publishDiagnostics".to_string(),
                serde_json::json!({"uri": uri_str, "diagnostics": []}),
            ));
        });
        assert!(client.wait_for_diagnostics(&uri, 2_000).await);
        assert!(!client.wait_for_diagnostics(&other, 50).await);
    }

    #[test]
    fn initialize_params_carry_the_configured_initialization_options() {
        let root = PathBuf::from("/tmp/workspace");
        let uri = crate::types::path_to_uri(&root).unwrap();
        let config = crate::config::ServerConfig::python(root.clone());

        let params = build_initialize_params(&root, uri.clone(), &config.initialization_options);

        assert_eq!(
            params.initialization_options,
            Some(config.initialization_options.clone()),
            "configured initializationOptions must reach the server"
        );
        assert_eq!(
            params.initialization_options.as_ref().unwrap()["python"]["analysis"]["diagnosticMode"],
            serde_json::json!("workspace"),
            "pyright must be told to analyse the workspace, not open files only"
        );

        // A server with no options must not send an empty object.
        let none = build_initialize_params(&root, uri, &serde_json::Value::Null);
        assert_eq!(none.initialization_options, None);
    }

    /// typescript-language-server advertises `callHierarchyProvider` only to a
    /// client that declares `textDocument.callHierarchy`, and publishes
    /// diagnostics (the readiness signal) only to one that declares
    /// `textDocument.publishDiagnostics`.
    #[test]
    fn initialize_params_ask_for_call_hierarchy_and_diagnostics() {
        let root = PathBuf::from("/tmp/workspace");
        let uri = crate::types::path_to_uri(&root).unwrap();

        let params = build_initialize_params(&root, uri, &serde_json::Value::Null);

        let value = serde_json::to_value(&params).unwrap();
        assert!(
            value["capabilities"]["textDocument"]["callHierarchy"].is_object(),
            "initialize must declare textDocument.callHierarchy: {}",
            value["capabilities"]
        );
        assert!(
            value["capabilities"]["textDocument"]["publishDiagnostics"].is_object(),
            "initialize must declare textDocument.publishDiagnostics: {}",
            value["capabilities"]
        );
    }

    #[tokio::test]
    async fn test_client_not_initialized() {
        let (client, _rx) = create_test_client();
        assert!(!client.is_initialized());
    }

    #[tokio::test]
    async fn test_client_initialized_flag() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        assert!(client.is_initialized());
    }

    #[tokio::test]
    async fn test_next_id_increments() {
        let (client, _rx) = create_test_client();

        let id1 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id2 = client.next_id.fetch_add(1, Ordering::SeqCst);
        let id3 = client.next_id.fetch_add(1, Ordering::SeqCst);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    #[tokio::test]
    async fn test_notify_with_terminated_server() {
        let (tx, rx) = mpsc::unbounded_channel();
        // Drop receiver to simulate terminated server
        drop(rx);

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        let result = client
            .notify::<lsp_types::notification::DidOpenTextDocument>(
                lsp_types::DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                        language_id: "typescript".to_string(),
                        version: 1,
                        text: "".to_string(),
                    },
                },
            )
            .await;

        assert!(matches!(result, Err(LspError::ServerTerminated { .. })));
    }

    #[tokio::test]
    async fn test_outgoing_message_enum() {
        // Test that OutgoingMessage can hold different types
        let request = JsonRpcRequest::new(1, "test/method".to_string(), serde_json::json!({}));
        let msg_request = OutgoingMessage::Request(request);

        let notification =
            JsonRpcNotification::new("test/notification".to_string(), serde_json::json!({}));
        let msg_notification = OutgoingMessage::Notification(notification);

        // Just verify they can be created and matched
        match msg_request {
            OutgoingMessage::Request(r) => assert_eq!(r.method, "test/method"),
            OutgoingMessage::Notification(_) => panic!("expected request"),
            OutgoingMessage::Raw(_) => panic!("unexpected raw reply"),
        }

        match msg_notification {
            OutgoingMessage::Request(_) => panic!("expected notification"),
            OutgoingMessage::Notification(n) => assert_eq!(n.method, "test/notification"),
            OutgoingMessage::Raw(_) => panic!("unexpected raw reply"),
        }
    }

    #[tokio::test]
    async fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new(
            42,
            "textDocument/hover".to_string(),
            serde_json::json!({
                "textDocument": { "uri": "file:///test.ts" },
                "position": { "line": 0, "character": 5 }
            }),
        );

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"method\":\"textDocument/hover\""));
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
    }

    #[tokio::test]
    async fn test_json_rpc_notification_serialization() {
        let notification = JsonRpcNotification::new(
            "textDocument/didOpen".to_string(),
            serde_json::json!({
                "textDocument": {
                    "uri": "file:///test.ts",
                    "languageId": "typescript",
                    "version": 1,
                    "text": "const x = 1;"
                }
            }),
        );

        let json = serde_json::to_string(&notification).unwrap();
        assert!(json.contains("\"method\":\"textDocument/didOpen\""));
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        // Notifications don't have an id field
        assert!(!json.contains("\"id\""));
    }

    #[tokio::test]
    async fn test_pending_requests_cleanup_on_timeout() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let pending = Arc::new(Mutex::new(HashMap::new()));

        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::clone(&pending),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: 10, // Very short timeout
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Make a request that will timeout
        let result = client
            .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await;

        // Should timeout
        assert!(matches!(result, Err(LspError::Timeout { .. })));

        // Pending map should be cleaned up
        let pending_lock = pending.lock().await;
        assert!(pending_lock.is_empty());
    }

    #[tokio::test]
    async fn test_request_with_terminated_server() {
        let (tx, rx) = mpsc::unbounded_channel();
        // Drop receiver to simulate terminated server
        drop(rx);

        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        let result = client
            .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await;

        assert!(matches!(result, Err(LspError::ServerTerminated { .. })));
    }

    #[tokio::test]
    async fn test_multiple_concurrent_next_ids() {
        let (client, _rx) = create_test_client();

        // Simulate concurrent ID generation
        let mut ids = Vec::new();
        for _ in 0..10 {
            ids.push(client.next_id.fetch_add(1, Ordering::SeqCst));
        }

        // All IDs should be unique and sequential
        for (i, id) in ids.iter().enumerate().take(10) {
            assert_eq!(*id, (i + 1) as u64);
        }
    }

    #[tokio::test]
    async fn test_client_language_field() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: 5000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Verify the language is stored correctly
        assert_eq!(client.language, "typescript");
    }

    #[tokio::test]
    async fn test_client_timeout_field() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(false)),
            timeout_ms: 30000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        assert_eq!(client.timeout_ms, 30000);
    }

    #[tokio::test]
    async fn test_initialized_flag_toggling() {
        let (client, _rx) = create_test_client();

        assert!(!client.is_initialized());

        client.initialized.store(true, Ordering::SeqCst);
        assert!(client.is_initialized());

        client.initialized.store(false, Ordering::SeqCst);
        assert!(!client.is_initialized());
    }

    #[tokio::test]
    async fn test_pending_map_operations() {
        let (_client, _rx) = create_test_client();

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Test insert
        let (tx, _rx) = oneshot::channel();
        pending.lock().await.insert(1, tx);
        assert_eq!(pending.lock().await.len(), 1);

        // Test remove
        let _ = pending.lock().await.remove(&1);
        assert!(pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn test_outgoing_message_request_creation() {
        let request = JsonRpcRequest::new(
            1,
            "textDocument/definition".to_string(),
            serde_json::json!({"uri": "file:///test.ts"}),
        );
        let msg = OutgoingMessage::Request(request);

        match msg {
            OutgoingMessage::Request(r) => {
                assert_eq!(r.id, 1);
                assert_eq!(r.method, "textDocument/definition");
            }
            _ => panic!("Expected Request"),
        }
    }

    #[tokio::test]
    async fn test_outgoing_message_notification_creation() {
        let notification = JsonRpcNotification::new(
            "textDocument/didSave".to_string(),
            serde_json::json!({"uri": "file:///test.ts"}),
        );
        let msg = OutgoingMessage::Notification(notification);

        match msg {
            OutgoingMessage::Notification(n) => {
                assert_eq!(n.method, "textDocument/didSave");
            }
            _ => panic!("Expected Notification"),
        }
    }

    #[tokio::test]
    async fn test_read_loop_handles_eof() {
        let empty_input: &[u8] = b"";
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Should return immediately on EOF
        LspClient::read_loop(
            empty_input,
            pending,
            "test".to_string(),
            broadcast::channel(16).0,
            mpsc::unbounded_channel().0,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;
        // Test passes if it returns without hanging
    }

    #[tokio::test]
    async fn test_read_loop_handles_valid_response() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"result": "ok"})),
            error: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().await.insert(1, tx);

        // Use a cursor to provide the message as input
        let input = std::io::Cursor::new(message.into_bytes());

        // Spawn read loop
        let pending_clone = Arc::clone(&pending);
        let handle = tokio::spawn(async move {
            LspClient::read_loop(
                input,
                pending_clone,
                "test".to_string(),
                broadcast::channel(16).0,
                mpsc::unbounded_channel().0,
                Arc::new(Mutex::new(std::collections::HashSet::new())),
            )
            .await;
        });

        // Wait for response with timeout
        let result = timeout(Duration::from_millis(100), rx).await;

        // Should receive the response
        if let Ok(Ok(resp)) = result {
            assert_eq!(resp.id, 1);
            assert!(resp.result.is_some());
        }

        // Clean up
        handle.abort();
    }

    #[tokio::test]
    async fn read_loop_answers_a_server_request_with_a_string_id() {
        // sourcekit-lsp sends `window/workDoneProgress/create` with a string
        // id; it starts indexing progress only once that request is answered.
        let body = r#"{"jsonrpc":"2.0","id":"sk-0","method":"window/workDoneProgress/create","params":{"token":"t"}}"#;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let (reply_tx, mut reply_rx) = mpsc::unbounded_channel();

        LspClient::read_loop(
            std::io::Cursor::new(message.into_bytes()),
            Arc::new(Mutex::new(HashMap::new())),
            "test".to_string(),
            broadcast::channel(16).0,
            reply_tx,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;

        let reply = match reply_rx.try_recv() {
            Ok(OutgoingMessage::Raw(json)) => json,
            _ => panic!("server request with a string id was not answered"),
        };
        let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["id"], serde_json::json!("sk-0"));
        assert_eq!(reply["result"], serde_json::Value::Null);
        assert!(reply.get("error").is_none());
    }

    #[tokio::test]
    async fn indexing_progress_begun_before_the_wait_still_holds_it() {
        // sourcekit-lsp begins its indexing progress about a second after
        // didOpen, before the first query reaches wait_for_indexing.
        let body = r#"{"jsonrpc":"2.0","method":"$/progress","params":{"token":"indexing.1","value":{"kind":"begin","title":"Indexing"}}}"#;
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
        let (client, _rx) = create_test_client();

        LspClient::read_loop(
            std::io::Cursor::new(message.into_bytes()),
            Arc::new(Mutex::new(HashMap::new())),
            "test".to_string(),
            client.notifications.clone(),
            mpsc::unbounded_channel().0,
            Arc::clone(&client.active_progress),
        )
        .await;

        assert!(
            !client.wait_for_indexing(300).await,
            "wait_for_indexing reported idle while indexing progress was active"
        );
    }

    #[tokio::test]
    async fn first_open_waits_for_indexing_that_begins_after_did_open() {
        // sourcekit-lsp begins indexing about a second after didOpen, after
        // the settle wait has seen an idle server.
        let (client, _rx) = create_test_client();
        let uri = Uri::from_str("file:///tmp/Lib.swift").unwrap();
        let progress = Arc::clone(&client.active_progress);
        let notifications = client.notifications.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            progress.lock().await.insert("indexing.1".to_string());
            let _ = notifications.send(("$/progress".to_string(), serde_json::json!({})));
            tokio::time::sleep(Duration::from_millis(500)).await;
            progress.lock().await.remove("indexing.1");
            let _ = notifications.send(("$/progress".to_string(), serde_json::json!({})));
        });

        client.wait_for_diagnostics(&uri, 500).await;

        assert!(
            client.active_progress.lock().await.is_empty(),
            "first open returned while indexing progress was still active"
        );
    }

    /// Runs a references query on a fresh test client and returns how long
    /// after its didOpen the request went out. `script` gets the client's
    /// progress set and notification sender once the didOpen is seen.
    async fn first_query_delay<F, Fut>(script: F) -> Duration
    where
        F: FnOnce(
                Arc<Mutex<std::collections::HashSet<String>>>,
                broadcast::Sender<(String, serde_json::Value)>,
            ) -> Fut
            + Send
            + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Lib.swift");
        std::fs::write(&file, "func compute() {}\n").unwrap();
        let (client, mut rx) = create_test_client();
        let client = Arc::new(client);
        let progress = Arc::clone(&client.active_progress);
        let notifications = client.notifications.clone();

        let querying = Arc::clone(&client);
        let query = tokio::spawn(async move {
            let position = act_lite_core::Location::new(file.clone(), 1, 6);
            let _ = crate::capabilities::query_references(&*querying, &file, &position, None).await;
        });

        let opened = match rx.recv().await {
            Some(OutgoingMessage::Notification(n)) if n.method == "textDocument/didOpen" => {
                tokio::time::Instant::now()
            }
            _ => panic!("expected didOpen first"),
        };
        tokio::spawn(script(progress, notifications));
        match rx.recv().await {
            Some(OutgoingMessage::Request(r)) if r.method == "textDocument/references" => {}
            _ => panic!("expected the references request"),
        }
        let delay = opened.elapsed();
        query.abort();
        delay
    }

    #[tokio::test(start_paused = true)]
    async fn first_query_goes_out_within_the_budget_when_progress_never_ends() {
        let delay = first_query_delay(|progress, _| async move {
            progress.lock().await.insert("indexing.1".to_string());
        })
        .await;

        assert!(
            delay <= Duration::from_millis(FIRST_OPEN_WAIT_BUDGET_MS),
            "first query went out {delay:?} after didOpen, over the {FIRST_OPEN_WAIT_BUDGET_MS} ms budget"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn first_query_waits_for_progress_begun_just_after_the_diagnostics_wait() {
        // Settle beat (150 ms) + diagnostics wait (2 s) end at 2.15 s; the
        // begin lands 50 ms later and the progress ends at 5 s.
        let delay = first_query_delay(|progress, notifications| async move {
            tokio::time::sleep(Duration::from_millis(2_200)).await;
            progress.lock().await.insert("indexing.1".to_string());
            let _ = notifications.send(("$/progress".to_string(), serde_json::json!({})));
            tokio::time::sleep(Duration::from_millis(2_800)).await;
            progress.lock().await.remove("indexing.1");
            let _ = notifications.send(("$/progress".to_string(), serde_json::json!({})));
        })
        .await;

        assert!(
            delay >= Duration::from_millis(5_000),
            "first query went out {delay:?} after didOpen, before the indexing progress ended at 5 s"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn elixir_ls_first_query_waits_for_the_build_end() {
        // elixir-ls 0.30 publishes diagnostics 0.35 s after didOpen, builds
        // the project 3 s after `initialized`, reports no work-done progress,
        // and ends the build with `telemetry/event` `{"name":"build"}`.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("lib.ex");
        std::fs::write(&file, "defmodule Lib do\n  def compute(x), do: x\nend\n").unwrap();
        let uri = crate::types::path_to_uri(&file).unwrap();
        let (mut client, mut rx) = create_test_client();
        client.language = "elixir-ls".to_string();
        client.active_progress = Arc::new(Mutex::new(first_build_progress("elixir-ls")));
        let client = Arc::new(client);

        let (mut server, stdout) = tokio::io::duplex(4096);
        tokio::spawn(LspClient::read_loop(
            stdout,
            Arc::clone(&client.pending),
            client.language.clone(),
            client.notifications.clone(),
            mpsc::unbounded_channel().0,
            Arc::clone(&client.active_progress),
        ));

        let querying = Arc::clone(&client);
        let query_file = file.clone();
        let query = tokio::spawn(async move {
            let position = act_lite_core::Location::new(query_file.clone(), 2, 7);
            let _ = crate::capabilities::query_references(&*querying, &query_file, &position, None)
                .await;
        });

        let opened = match rx.recv().await {
            Some(OutgoingMessage::Notification(n)) if n.method == "textDocument/didOpen" => {
                tokio::time::Instant::now()
            }
            _ => panic!("expected didOpen first"),
        };
        let send = |method: &str, params: serde_json::Value| {
            let body = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": params})
                .to_string();
            format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
        };
        let diagnostics = send(
            "textDocument/publishDiagnostics",
            serde_json::json!({"uri": uri.as_str(), "diagnostics": []}),
        );
        let build = send(
            "telemetry/event",
            serde_json::json!({"name": "build", "measurements": {}, "properties": {}}),
        );
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(350)).await;
            server.write_all(diagnostics.as_bytes()).await.unwrap();
            tokio::time::sleep(Duration::from_millis(3_150)).await;
            server.write_all(build.as_bytes()).await.unwrap();
            // Keep the pipe open: EOF would end read_loop.
            tokio::time::sleep(Duration::from_secs(3_600)).await;
        });

        match rx.recv().await {
            Some(OutgoingMessage::Request(r)) if r.method == "textDocument/references" => {}
            _ => panic!("expected the references request"),
        }
        let delay = opened.elapsed();
        query.abort();

        assert!(
            delay >= Duration::from_millis(3_500)
                && delay <= Duration::from_millis(FIRST_OPEN_WAIT_BUDGET_MS),
            "first query went out {delay:?} after didOpen; the build ended at 3.5 s"
        );
    }

    #[tokio::test]
    async fn test_read_loop_ignores_notification() {
        let notification = JsonRpcNotification::new(
            "textDocument/publishDiagnostics".to_string(),
            serde_json::json!({}),
        );
        let json = serde_json::to_string(&notification).unwrap();
        let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let input = std::io::Cursor::new(message.into_bytes());

        // Should handle notification gracefully and return on EOF
        LspClient::read_loop(
            input,
            pending,
            "test".to_string(),
            broadcast::channel(16).0,
            mpsc::unbounded_channel().0,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;
    }

    #[tokio::test]
    async fn test_read_loop_handles_missing_content_length() {
        // Message without Content-Length header followed by EOF
        let message = "Invalid-Header: value\r\n\r\n";

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let input = std::io::Cursor::new(message.as_bytes().to_vec());

        // Should continue and eventually return on EOF
        LspClient::read_loop(
            input,
            pending,
            "test".to_string(),
            broadcast::channel(16).0,
            mpsc::unbounded_channel().0,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;
    }

    #[tokio::test]
    async fn test_read_loop_handles_invalid_json() {
        // Valid header and UTF-8, but invalid JSON
        let body = "not valid json";
        let message = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let input = std::io::Cursor::new(message.into_bytes());

        // Should handle invalid JSON gracefully
        LspClient::read_loop(
            input,
            pending,
            "test".to_string(),
            broadcast::channel(16).0,
            mpsc::unbounded_channel().0,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;
    }

    #[tokio::test]
    async fn test_write_loop_sends_request() {
        use tokio::io::AsyncReadExt;

        let (tx, rx) = mpsc::unbounded_channel();
        let (output, mut output_rx) = tokio::io::duplex(1024);

        let handle = tokio::spawn(async move {
            LspClient::write_loop(output, rx, "test".to_string()).await;
        });

        // Send a request
        let request = JsonRpcRequest::new(1, "test/method".to_string(), serde_json::json!({}));
        tx.send(OutgoingMessage::Request(request)).unwrap();

        // Drop sender to close the channel
        drop(tx);

        // Wait for write loop to finish
        let _ = timeout(Duration::from_millis(100), handle).await;

        // Read what was written
        let mut buf = vec![0u8; 1024];
        let n = timeout(Duration::from_millis(100), output_rx.read(&mut buf)).await;

        if let Ok(Ok(n)) = n {
            let output_str = String::from_utf8_lossy(&buf[..n]);
            assert!(output_str.contains("Content-Length:"));
            assert!(output_str.contains("test/method"));
        }
    }

    #[tokio::test]
    async fn test_write_loop_sends_notification() {
        use tokio::io::AsyncReadExt;

        let (tx, rx) = mpsc::unbounded_channel();
        let (output, mut output_rx) = tokio::io::duplex(1024);

        let handle = tokio::spawn(async move {
            LspClient::write_loop(output, rx, "test".to_string()).await;
        });

        // Send a notification
        let notification = JsonRpcNotification::new(
            "textDocument/didOpen".to_string(),
            serde_json::json!({"uri": "file:///test.ts"}),
        );
        tx.send(OutgoingMessage::Notification(notification))
            .unwrap();

        // Drop sender to close the channel
        drop(tx);

        // Wait for write loop to finish
        let _ = timeout(Duration::from_millis(100), handle).await;

        // Read what was written
        let mut buf = vec![0u8; 1024];
        let n = timeout(Duration::from_millis(100), output_rx.read(&mut buf)).await;

        if let Ok(Ok(n)) = n {
            let output_str = String::from_utf8_lossy(&buf[..n]);
            assert!(output_str.contains("textDocument/didOpen"));
        }
    }

    #[tokio::test]
    async fn test_write_loop_handles_closed_receiver() {
        let (tx, rx) = mpsc::unbounded_channel::<OutgoingMessage>();
        let (output, _) = tokio::io::duplex(1024);

        // Drop the sender immediately to close the channel
        drop(tx);

        // Write loop should return when channel is closed
        let result = timeout(
            Duration::from_millis(100),
            LspClient::write_loop(output, rx, "test".to_string()),
        )
        .await;

        assert!(result.is_ok()); // Should complete without hanging
    }

    #[tokio::test]
    async fn test_request_with_error_response() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::clone(&pending),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 5000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a task to respond with an error
        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            if let Some(OutgoingMessage::Request(req)) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
                let error_response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: Some(crate::types::JsonRpcError {
                        code: -32600,
                        message: "Invalid Request".to_string(),
                        data: None,
                    }),
                };
                let mut pending_lock = pending_clone.lock().await;
                if let Some(sender) = pending_lock.remove(&req.id) {
                    let _ = sender.send(error_response);
                }
            }
        });

        // Make a request
        let result = client
            .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await;

        // Should get a ResponseError
        assert!(matches!(result, Err(LspError::ResponseError { .. })));
    }

    /// `textDocument/hover` is declared `Hover | null`, and null — "nothing to
    /// show at this position" — is its most common answer. This test used to
    /// assert that answer was a `ResponseError`, which pinned the defect rather
    /// than a requirement.
    #[tokio::test]
    async fn test_request_with_null_result() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::clone(&pending),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 5000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a task to respond with a null result
        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            if let Some(OutgoingMessage::Request(req)) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
                // Create a response with no result and no error
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: None,
                    error: None,
                };
                let mut pending_lock = pending_clone.lock().await;
                if let Some(sender) = pending_lock.remove(&req.id) {
                    let _ = sender.send(response);
                }
            }
        });

        // Make a request that expects Option<Hover>
        let result = client
            .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await;

        // A null hover is "no hover here", not a failure.
        assert_eq!(
            result.expect("a null hover result must not be an error"),
            None
        );
    }

    #[tokio::test]
    async fn test_request_with_valid_response() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::clone(&pending),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 5000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a task to respond with a valid hover result
        let pending_clone = Arc::clone(&pending);
        tokio::spawn(async move {
            if let Some(OutgoingMessage::Request(req)) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(10)).await;
                // Create a valid hover response
                let hover = lsp_types::Hover {
                    contents: lsp_types::HoverContents::Scalar(lsp_types::MarkedString::String(
                        "string".to_string(),
                    )),
                    range: None,
                };
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(serde_json::to_value(Some(hover)).unwrap()),
                    error: None,
                };
                let mut pending_lock = pending_clone.lock().await;
                if let Some(sender) = pending_lock.remove(&req.id) {
                    let _ = sender.send(response);
                }
            }
        });

        // Make a request
        let result: Result<Option<lsp_types::Hover>> = client
            .request::<lsp_types::request::HoverRequest>(lsp_types::HoverParams {
                text_document_position_params: lsp_types::TextDocumentPositionParams {
                    text_document: lsp_types::TextDocumentIdentifier {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                    },
                    position: lsp_types::Position {
                        line: 0,
                        character: 0,
                    },
                },
                work_done_progress_params: Default::default(),
            })
            .await;

        // Should get a valid response
        assert!(result.is_ok());
        let hover = result.unwrap();
        assert!(hover.is_some());
    }

    #[tokio::test]
    async fn test_notify_succeeds() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let client = LspClient {
            language: "test".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a consumer
        tokio::spawn(async move {
            while let Some(_msg) = rx.recv().await {
                // Consume messages
            }
        });

        let result = client
            .notify::<lsp_types::notification::DidOpenTextDocument>(
                lsp_types::DidOpenTextDocumentParams {
                    text_document: lsp_types::TextDocumentItem {
                        uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                        language_id: "typescript".to_string(),
                        version: 1,
                        text: "const x = 1;".to_string(),
                    },
                },
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_did_open_notification() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a consumer
        let consumer = tokio::spawn(async move {
            if let Some(OutgoingMessage::Notification(notif)) = rx.recv().await {
                assert_eq!(notif.method, "textDocument/didOpen");
                return true;
            }
            false
        });

        let uri = lsp_types::Uri::from_str("file:///test.ts").unwrap();
        let result = client
            .did_open(
                uri.clone(),
                "typescript".to_string(),
                "const x = 1;".to_string(),
            )
            .await;

        // There is one `did_open`, and it is the trait's: it reports what it
        // actually did. When an inherent `did_open` shadowed it, this call
        // resolved to that one and this test proved nothing about the method
        // production uses.
        assert_eq!(result.unwrap(), DocumentOpen::Opened);
        let received = consumer.await.unwrap();
        assert!(received);

        // A second open of the same URI sends nothing and says so.
        let again = client
            .did_open(uri, "typescript".to_string(), "const x = 1;".to_string())
            .await;
        assert_eq!(again.unwrap(), DocumentOpen::AlreadyOpen);
    }

    #[tokio::test]
    async fn test_did_change_notification() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a consumer
        let consumer = tokio::spawn(async move {
            if let Some(OutgoingMessage::Notification(notif)) = rx.recv().await {
                assert_eq!(notif.method, "textDocument/didChange");
                return true;
            }
            false
        });

        let uri = lsp_types::Uri::from_str("file:///test.ts").unwrap();
        let changes = vec![lsp_types::TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "const y = 2;".to_string(),
        }];
        let result = client.did_change(uri, 2, changes).await;

        assert!(result.is_ok());
        let received = consumer.await.unwrap();
        assert!(received);
    }

    #[tokio::test]
    async fn test_did_close_notification() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let client = LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 1000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        };

        // Spawn a consumer
        let consumer = tokio::spawn(async move {
            if let Some(OutgoingMessage::Notification(notif)) = rx.recv().await {
                assert_eq!(notif.method, "textDocument/didClose");
                return true;
            }
            false
        });

        let uri = lsp_types::Uri::from_str("file:///test.ts").unwrap();
        let result = client.did_close(uri).await;

        assert!(result.is_ok());
        let received = consumer.await.unwrap();
        assert!(received);
    }

    #[tokio::test]
    async fn test_spawn_with_invalid_command() {
        let config = crate::config::ServerConfig::new("rm".to_string(), vec!["-rf".to_string()]);
        let result = LspClient::spawn("test".to_string(), config).await;
        assert!(matches!(result, Err(LspError::InvalidCommand { .. })));
    }

    #[tokio::test]
    async fn test_read_loop_response_without_pending() {
        // Response for an ID that's not in pending map
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 999,
            result: Some(serde_json::json!({"result": "ok"})),
            error: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        let message = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let input = std::io::Cursor::new(message.into_bytes());

        // Should handle gracefully (log and continue)
        LspClient::read_loop(
            input,
            pending,
            "test".to_string(),
            broadcast::channel(16).0,
            mpsc::unbounded_channel().0,
            Arc::new(Mutex::new(std::collections::HashSet::new())),
        )
        .await;
    }

    #[tokio::test]
    async fn test_concurrent_requests() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let client = Arc::new(LspClient {
            language: "typescript".to_string(),
            process: Arc::new(Mutex::new(None)),
            #[cfg(unix)]
            process_group: None,
            message_tx: tx,
            pending: Arc::clone(&pending),
            next_id: Arc::new(AtomicU64::new(1)),
            initialized: Arc::new(AtomicBool::new(true)),
            timeout_ms: 5000,
            initialization_options: serde_json::Value::Null,
            notifications: broadcast::channel(16).0,
            active_progress: Arc::new(Mutex::new(std::collections::HashSet::new())),
            opened: Arc::new(Mutex::new(HashMap::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            server_capabilities: std::sync::OnceLock::new(),
            settled: Arc::new(AtomicBool::new(false)),
        });

        // Spawn a responder
        let pending_clone = Arc::clone(&pending);
        let responder = tokio::spawn(async move {
            while let Some(OutgoingMessage::Request(req)) = rx.recv().await {
                tokio::time::sleep(Duration::from_millis(5)).await;
                let response = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Some(serde_json::to_value::<Option<lsp_types::Hover>>(None).unwrap()),
                    error: None,
                };
                let mut pending_lock = pending_clone.lock().await;
                if let Some(sender) = pending_lock.remove(&req.id) {
                    let _ = sender.send(response);
                }
            }
        });

        // Make multiple concurrent requests
        let client1 = Arc::clone(&client);
        let client2 = Arc::clone(&client);

        let params = lsp_types::HoverParams {
            text_document_position_params: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier {
                    uri: lsp_types::Uri::from_str("file:///test.ts").unwrap(),
                },
                position: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
        };

        let (r1, r2) = tokio::join!(
            client1.request::<lsp_types::request::HoverRequest>(params.clone()),
            client2.request::<lsp_types::request::HoverRequest>(params),
        );

        assert!(r1.is_ok());
        assert!(r2.is_ok());

        responder.abort();
    }

    #[tokio::test]
    async fn test_read_loop_multiple_responses() {
        // Create multiple responses
        let response1 = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"id": 1})),
            error: None,
        };
        let response2 = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 2,
            result: Some(serde_json::json!({"id": 2})),
            error: None,
        };

        let json1 = serde_json::to_string(&response1).unwrap();
        let json2 = serde_json::to_string(&response2).unwrap();

        let message = format!(
            "Content-Length: {}\r\n\r\n{}Content-Length: {}\r\n\r\n{}",
            json1.len(),
            json1,
            json2.len(),
            json2
        );

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx1, rx1) = oneshot::channel();
        let (tx2, rx2) = oneshot::channel();
        pending.lock().await.insert(1, tx1);
        pending.lock().await.insert(2, tx2);

        let input = std::io::Cursor::new(message.into_bytes());

        let pending_clone = Arc::clone(&pending);
        let handle = tokio::spawn(async move {
            LspClient::read_loop(
                input,
                pending_clone,
                "test".to_string(),
                broadcast::channel(16).0,
                mpsc::unbounded_channel().0,
                Arc::new(Mutex::new(std::collections::HashSet::new())),
            )
            .await;
        });

        // Wait for responses
        let result1 = timeout(Duration::from_millis(100), rx1).await;
        let result2 = timeout(Duration::from_millis(100), rx2).await;

        if let Ok(Ok(resp)) = result1 {
            assert_eq!(resp.id, 1);
        }
        if let Ok(Ok(resp)) = result2 {
            assert_eq!(resp.id, 2);
        }

        handle.abort();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shutdown_kills_the_whole_process_tree_even_without_a_handshake() {
        let temp = tempfile::tempdir().unwrap();
        let pidfile = temp.path().join("pids");

        // Stands in for tsserver: forks a worker, records both pids, and never
        // speaks a word of LSP.
        let script = temp.path().join("typescript-language-server");
        crate::executable_script::write_executable(
            &script,
            &format!(
                "#!/bin/sh\nsleep 300 &\nprintf '%s %s' \"$$\" \"$!\" > {}\nwait\n",
                pidfile.display()
            ),
        );

        let config = ServerConfig::new(script.to_string_lossy().into_owned(), vec![])
            .with_working_directory(temp.path().to_path_buf())
            .with_timeout(250);

        let client = LspClient::spawn("typescript-language-server".to_string(), config)
            .await
            .expect("fake server must spawn");

        // Wait for the script to record both pids.
        let mut pids = String::new();
        for _ in 0..100 {
            if let Ok(s) = std::fs::read_to_string(&pidfile)
                && s.split_whitespace().count() == 2
            {
                pids = s;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let parsed: Vec<i32> = pids
            .split_whitespace()
            .map(|p| p.parse().unwrap())
            .collect();
        assert_eq!(parsed.len(), 2, "fake server never recorded its pids");
        let (server_pid, worker_pid) = (parsed[0], parsed[1]);

        let alive = |pid: i32| unsafe { libc::kill(pid, 0) } == 0;
        assert!(
            alive(server_pid) && alive(worker_pid),
            "both must start alive"
        );

        // The handshake will fail — the script does not speak LSP. The kill
        // must happen regardless.
        let _ = client.shutdown().await;

        for _ in 0..100 {
            if !alive(server_pid) && !alive(worker_pid) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(
            !alive(server_pid),
            "the language server itself must be killed"
        );
        assert!(
            !alive(worker_pid),
            "the worker it forked must be killed too — this is the orphan that accumulated"
        );
    }
}
