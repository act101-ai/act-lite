//! Mock LSP client for testing.

use crate::client::{DocumentOpen, LspClientTrait};
use crate::error::{LspError, Result};
use async_trait::async_trait;
use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

/// Mock LSP client for deterministic testing.
///
/// Allows tests to register canned responses for specific requests.
#[derive(Clone)]
pub struct MockLspClient {
    /// Canned responses for requests, keyed by method name.
    responses: Arc<Mutex<HashMap<String, Vec<serde_json::Value>>>>,

    /// Whether the client is initialized.
    initialized: Arc<Mutex<bool>>,

    /// Record of sent requests, for verification.
    sent_requests: Arc<Mutex<Vec<(String, serde_json::Value)>>>,

    /// Record of sent notifications, for verification.
    sent_notifications: Arc<Mutex<Vec<(String, serde_json::Value)>>>,

    /// If true, return empty/null responses instead of errors for missing responses.
    lenient: Arc<Mutex<bool>>,

    /// URIs the caller waited on for diagnostics, for verification.
    diagnostics_waits: Arc<Mutex<Vec<String>>>,

    /// Documents already sent with `didOpen`, mirroring `LspClient::opened`.
    opened: Arc<Mutex<HashSet<String>>>,

    /// Recorded readiness outcomes, mirroring `LspClient::readiness`.
    ///
    /// Written only through `record_readiness` (i.e. by `open_document` at a
    /// fresh open), never pre-seeded: the mock models only states production
    /// can reach, and in production every open goes through `open_document`,
    /// so a URI in `opened` without an entry here is not a reachable state.
    readiness: Arc<Mutex<HashMap<String, bool>>>,

    /// When true, `wait_for_diagnostics` reports a timeout instead of a
    /// settled analysis — the condition a real server produces when it is
    /// still analysing a file after the readiness budget expires.
    analysis_times_out: Arc<Mutex<bool>>,

    empty_answers: Arc<Mutex<bool>>,

    /// The server's `initialize` capabilities, once a test sets them.
    server_capabilities: Arc<OnceLock<lsp_types::ServerCapabilities>>,
}

impl MockLspClient {
    /// Creates a new mock client.
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            initialized: Arc::new(Mutex::new(true)),
            sent_requests: Arc::new(Mutex::new(Vec::new())),
            sent_notifications: Arc::new(Mutex::new(Vec::new())),
            lenient: Arc::new(Mutex::new(false)),
            diagnostics_waits: Arc::new(Mutex::new(Vec::new())),
            opened: Arc::new(Mutex::new(HashSet::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            analysis_times_out: Arc::new(Mutex::new(false)),
            empty_answers: Arc::new(Mutex::new(false)),
            server_capabilities: Arc::new(OnceLock::new()),
        }
    }

    /// Creates a new mock client in lenient mode.
    ///
    /// In lenient mode, requests without registered responses return
    /// empty/null values instead of errors.
    pub fn new_lenient() -> Self {
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            initialized: Arc::new(Mutex::new(true)),
            sent_requests: Arc::new(Mutex::new(Vec::new())),
            sent_notifications: Arc::new(Mutex::new(Vec::new())),
            lenient: Arc::new(Mutex::new(true)),
            diagnostics_waits: Arc::new(Mutex::new(Vec::new())),
            opened: Arc::new(Mutex::new(HashSet::new())),
            readiness: Arc::new(Mutex::new(HashMap::new())),
            analysis_times_out: Arc::new(Mutex::new(false)),
            empty_answers: Arc::new(Mutex::new(false)),
            server_capabilities: Arc::new(OnceLock::new()),
        }
    }

    /// Enables lenient mode.
    pub fn set_lenient(&self, lenient: bool) {
        *self.lenient.lock().unwrap() = lenient;
    }

    pub fn set_empty_answers(&self, enabled: bool) {
        *self.empty_answers.lock().unwrap() = enabled;
    }

    /// Makes `wait_for_diagnostics` report a timeout, so a test can exercise
    /// the "server has not finished analysing this file" branch without
    /// waiting on a real clock.
    pub fn set_analysis_times_out(&self, times_out: bool) {
        *self.analysis_times_out.lock().unwrap() = times_out;
    }

    /// Sets the capabilities the mocked server advertised in `initialize`.
    pub fn set_server_capabilities(&self, capabilities: lsp_types::ServerCapabilities) {
        let _ = self.server_capabilities.set(capabilities);
    }

    /// Registers a canned response for a specific request method.
    ///
    /// Multiple responses can be registered for the same method; they will be
    /// returned in FIFO order.
    pub fn register_response<R: Request>(&self, response: R::Result)
    where
        R::Result: Serialize,
    {
        let value = serde_json::to_value(response).expect("failed to serialize response");
        let mut responses = self.responses.lock().unwrap();
        responses
            .entry(R::METHOD.to_string())
            .or_default()
            .push(value);
    }

    /// Registers multiple canned responses for a specific request method.
    pub fn register_responses<R: Request>(&self, responses_list: Vec<R::Result>)
    where
        R::Result: Serialize,
    {
        for response in responses_list {
            self.register_response::<R>(response);
        }
    }

    /// Gets the list of requests sent to this mock client.
    pub fn sent_requests(&self) -> Vec<(String, serde_json::Value)> {
        self.sent_requests.lock().unwrap().clone()
    }

    /// Gets the list of notifications sent to this mock client.
    /// URIs a caller waited on via `wait_for_diagnostics`.
    pub fn diagnostics_waits(&self) -> Vec<String> {
        self.diagnostics_waits.lock().unwrap().clone()
    }

    pub fn sent_notifications(&self) -> Vec<(String, serde_json::Value)> {
        self.sent_notifications.lock().unwrap().clone()
    }

    /// Clears all recorded requests and notifications.
    pub fn clear_history(&self) {
        self.sent_requests.lock().unwrap().clear();
        self.sent_notifications.lock().unwrap().clear();
    }
}

impl Default for MockLspClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LspClientTrait for MockLspClient {
    async fn request<R: Request>(&self, params: R::Params) -> Result<R::Result>
    where
        R::Params: Serialize + Send,
        R::Result: DeserializeOwned,
    {
        // Record the request
        let params_value = serde_json::to_value(params).expect("failed to serialize params");
        self.sent_requests
            .lock()
            .unwrap()
            .push((R::METHOD.to_string(), params_value));

        // Get the next canned response
        let mut responses = self.responses.lock().unwrap();
        let is_lenient = *self.lenient.lock().unwrap();

        let response_list = responses.get_mut(R::METHOD);

        // In lenient mode, return null for missing responses
        if response_list.as_ref().is_none_or(|l| l.is_empty()) {
            if *self.empty_answers.lock().unwrap() {
                // A measured empty: the server answered and found nothing.
                // Falls through to `null` for results that are not collections.
                if let Ok(empty) =
                    serde_json::from_value::<R::Result>(serde_json::Value::Array(Vec::new()))
                {
                    return Ok(empty);
                }
            }
            if is_lenient {
                // Return null, which many LSP results interpret as "no results"
                return serde_json::from_value(serde_json::Value::Null).map_err(|e| {
                    LspError::ResponseError {
                        language: "mock".to_string(),
                        code: -1,
                        message: format!("failed to create default response: {}", e),
                    }
                });
            }
            if response_list.is_none() {
                return Err(LspError::ResponseError {
                    language: "mock".to_string(),
                    code: -1,
                    message: format!("no canned response for method '{}'", R::METHOD),
                });
            } else {
                return Err(LspError::ResponseError {
                    language: "mock".to_string(),
                    code: -1,
                    message: format!("no more canned responses for method '{}'", R::METHOD),
                });
            }
        }

        let response_list = response_list.unwrap();

        let response_value = response_list.remove(0);
        serde_json::from_value(response_value).map_err(|e| LspError::ResponseError {
            language: "mock".to_string(),
            code: -1,
            message: format!("failed to deserialize canned response: {}", e),
        })
    }

    async fn notify<N: Notification>(&self, params: N::Params) -> Result<()>
    where
        N::Params: Serialize + Send,
    {
        // Record the notification
        let params_value = serde_json::to_value(params).expect("failed to serialize params");
        self.sent_notifications
            .lock()
            .unwrap()
            .push((N::METHOD.to_string(), params_value));

        Ok(())
    }

    async fn wait_for_diagnostics(&self, uri: &lsp_types::Uri, _timeout_ms: u64) -> bool {
        // Recorded so tests can assert the readiness wait happened, and so no
        // test pays a real timeout.
        self.diagnostics_waits
            .lock()
            .unwrap()
            .push(uri.as_str().to_string());
        // Answering `true` unconditionally is what left the timed-out branch
        // untested — the same geometry as the `null`-result and query-position
        // defects: a mock that could not express the condition, so nothing
        // exercised the production path that handles it.
        !*self.analysis_times_out.lock().unwrap()
    }

    async fn record_readiness(&self, uri: &lsp_types::Uri, analyzed: bool) {
        self.readiness
            .lock()
            .unwrap()
            .insert(uri.as_str().to_string(), analyzed);
    }

    async fn readiness(&self, uri: &lsp_types::Uri) -> Option<bool> {
        self.readiness.lock().unwrap().get(uri.as_str()).copied()
    }

    async fn did_open(
        &self,
        uri: lsp_types::Uri,
        language_id: String,
        text: String,
    ) -> Result<DocumentOpen> {
        // Deduplicated exactly as `LspClient` does it. A mock that re-opened a
        // document the real client would have skipped would model a server
        // state production can never reach.
        if !self.opened.lock().unwrap().insert(uri.as_str().to_string()) {
            return Ok(DocumentOpen::AlreadyOpen);
        }
        self.notify::<lsp_types::notification::DidOpenTextDocument>(
            lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri,
                    language_id,
                    version: 1,
                    text,
                },
            },
        )
        .await?;
        Ok(DocumentOpen::Opened)
    }

    fn server_capabilities(&self) -> Option<&lsp_types::ServerCapabilities> {
        self.server_capabilities.get()
    }

    fn is_initialized(&self) -> bool {
        *self.initialized.lock().unwrap()
    }

    async fn shutdown(&self) -> Result<()> {
        *self.initialized.lock().unwrap() = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::request::References;
    use lsp_types::{Location, Position, Range, Uri};

    fn test_uri() -> Uri {
        "file:///test.ts".parse().unwrap()
    }

    #[tokio::test]
    async fn test_mock_client_basic() {
        let mock = MockLspClient::new();
        assert!(mock.is_initialized());
    }

    #[tokio::test]
    async fn test_mock_client_register_response() {
        let mock = MockLspClient::new();

        // Register a canned response
        let expected_locations = vec![Location {
            uri: test_uri(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 10,
                },
            },
        }];

        mock.register_response::<References>(Some(expected_locations.clone()));

        // Make a request
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 5,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        let result: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();
        assert_eq!(result, Some(expected_locations));

        // Verify the request was recorded
        let requests = mock.sent_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "textDocument/references");
    }

    #[tokio::test]
    async fn test_mock_client_shutdown() {
        let mock = MockLspClient::new();
        assert!(mock.is_initialized());

        mock.shutdown().await.unwrap();
        assert!(!mock.is_initialized());
    }

    #[tokio::test]
    async fn test_mock_client_no_response() {
        let mock = MockLspClient::new();

        // Try to make a request without registering a response
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 5,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        let result: Result<Option<Vec<Location>>> = mock.request::<References>(params).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_client_default() {
        let mock = MockLspClient::default();
        assert!(mock.is_initialized());
    }

    #[tokio::test]
    async fn test_mock_client_clone() {
        let mock = MockLspClient::new();
        mock.register_response::<References>(Some(vec![]));

        let cloned = mock.clone();
        assert!(cloned.is_initialized());

        // Both should share the same responses
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // First call consumes the response
        let result: Option<Vec<Location>> =
            mock.request::<References>(params.clone()).await.unwrap();
        assert!(result.is_some());

        // Second call on cloned should fail (response consumed)
        let result2: Result<Option<Vec<Location>>> = cloned.request::<References>(params).await;
        assert!(result2.is_err());
    }

    #[tokio::test]
    async fn test_mock_client_notifications() {
        let mock = MockLspClient::new();

        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: test_uri(),
                language_id: "typescript".to_string(),
                version: 1,
                text: "const x = 1;".to_string(),
            },
        };

        // Send notification
        mock.notify::<lsp_types::notification::DidOpenTextDocument>(params)
            .await
            .unwrap();

        // Verify notification was recorded
        let notifications = mock.sent_notifications();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].0, "textDocument/didOpen");
    }

    #[tokio::test]
    async fn test_mock_client_clear_history() {
        let mock = MockLspClient::new();

        // Send a notification
        mock.notify::<lsp_types::notification::DidOpenTextDocument>(
            lsp_types::DidOpenTextDocumentParams {
                text_document: lsp_types::TextDocumentItem {
                    uri: test_uri(),
                    language_id: "typescript".to_string(),
                    version: 1,
                    text: "".to_string(),
                },
            },
        )
        .await
        .unwrap();

        assert_eq!(mock.sent_notifications().len(), 1);

        // Clear history
        mock.clear_history();

        assert!(mock.sent_notifications().is_empty());
        assert!(mock.sent_requests().is_empty());
    }

    #[tokio::test]
    async fn test_mock_client_register_responses_multiple() {
        let mock = MockLspClient::new();

        // Register multiple responses
        let responses = vec![
            Some(vec![Location {
                uri: test_uri(),
                range: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 5,
                    },
                },
            }]),
            Some(vec![]),
            None,
        ];

        mock.register_responses::<References>(responses);

        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // First call gets first response
        let r1: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();
        assert_eq!(r1.unwrap().len(), 1);

        // Second call gets second response
        let r2: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();
        assert!(r2.unwrap().is_empty());

        // Third call gets third response
        let r3: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();
        assert!(r3.is_none());
    }

    #[tokio::test]
    async fn test_mock_client_exhausted_responses() {
        let mock = MockLspClient::new();

        // Register only one response
        mock.register_response::<References>(Some(vec![]));

        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // First call succeeds
        let _: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();

        // Second call fails (no more responses)
        let result: Result<Option<Vec<Location>>> = mock.request::<References>(params).await;
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("no more canned responses"));
    }

    #[tokio::test]
    async fn test_mock_client_multiple_requests_recorded() {
        let mock = MockLspClient::new();

        // Register enough responses
        mock.register_responses::<References>(vec![Some(vec![]), Some(vec![]), Some(vec![])]);

        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // Make multiple requests
        let _: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();
        let _: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();
        let _: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();

        // All should be recorded
        let requests = mock.sent_requests();
        assert_eq!(requests.len(), 3);
        for (method, _) in &requests {
            assert_eq!(method, "textDocument/references");
        }
    }

    #[tokio::test]
    async fn test_mock_client_lenient_mode() {
        let mock = MockLspClient::new_lenient();

        // In lenient mode, requests without registered responses return null
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // Should return None instead of error in lenient mode
        let result: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mock_client_set_lenient() {
        let mock = MockLspClient::new();

        // Initially not lenient - should fail without response
        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        let result: Result<Option<Vec<Location>>> =
            mock.request::<References>(params.clone()).await;
        assert!(result.is_err());

        // Set lenient mode
        mock.set_lenient(true);

        // Now should succeed with null
        let result: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mock_client_lenient_after_exhausted() {
        let mock = MockLspClient::new();
        mock.set_lenient(true);

        // Register one response
        mock.register_response::<References>(Some(vec![]));

        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 0,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: true,
            },
        };

        // First call gets the registered response
        let r1: Option<Vec<Location>> = mock.request::<References>(params.clone()).await.unwrap();
        assert!(r1.is_some());

        // Second call in lenient mode returns null instead of error
        let r2: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();
        assert!(r2.is_none());
    }

    #[tokio::test]
    async fn test_mock_client_request_params_recorded() {
        let mock = MockLspClient::new();
        mock.register_response::<References>(Some(vec![]));

        let params = lsp_types::ReferenceParams {
            text_document_position: lsp_types::TextDocumentPositionParams {
                text_document: lsp_types::TextDocumentIdentifier { uri: test_uri() },
                position: Position {
                    line: 5,
                    character: 10,
                },
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: lsp_types::ReferenceContext {
                include_declaration: false,
            },
        };

        let _: Option<Vec<Location>> = mock.request::<References>(params).await.unwrap();

        // Verify parameters were recorded
        let requests = mock.sent_requests();
        assert_eq!(requests.len(), 1);
        let (method, params_value) = &requests[0];
        assert_eq!(method, "textDocument/references");

        // Check params contain position info - ReferenceParams flattens textDocumentPosition
        let params_obj = params_value.as_object().unwrap();
        // The textDocumentPosition is flattened, so textDocument and position are at top level
        let position = params_obj.get("position").unwrap();
        assert_eq!(position.get("line").unwrap().as_u64().unwrap(), 5);
        assert_eq!(position.get("character").unwrap().as_u64().unwrap(), 10);
    }

    #[tokio::test]
    async fn test_mock_client_notification_params_recorded() {
        let mock = MockLspClient::new();

        let params = lsp_types::DidOpenTextDocumentParams {
            text_document: lsp_types::TextDocumentItem {
                uri: test_uri(),
                language_id: "typescript".to_string(),
                version: 42,
                text: "const test = 1;".to_string(),
            },
        };

        mock.notify::<lsp_types::notification::DidOpenTextDocument>(params)
            .await
            .unwrap();

        let notifications = mock.sent_notifications();
        assert_eq!(notifications.len(), 1);
        let (method, params_value) = &notifications[0];
        assert_eq!(method, "textDocument/didOpen");

        // Verify params contain the document info
        let text_doc = params_value.get("textDocument").unwrap();
        assert_eq!(text_doc.get("version").unwrap().as_u64().unwrap(), 42);
        assert_eq!(
            text_doc.get("languageId").unwrap().as_str().unwrap(),
            "typescript"
        );
    }
}
