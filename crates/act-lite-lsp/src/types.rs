//! Type definitions for LSP communication.

use lsp_types::{Position, Range, Uri};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// JSON-RPC request message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: serde_json::Value,
}

/// JSON-RPC response message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC notification message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
}

impl JsonRpcRequest {
    /// Creates a new JSON-RPC request.
    pub fn new(id: u64, method: String, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            method,
            params,
        }
    }
}

impl JsonRpcNotification {
    /// Creates a new JSON-RPC notification.
    pub fn new(method: String, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method,
            params,
        }
    }
}

/// Percent-encodes one path component set for use in a `file:` URI.
///
/// Unreserved characters (RFC 3986 §2.3) pass through, as do the path
/// separator and `:` (a legal `pchar`, and required by Windows drive letters).
/// Everything else — spaces, `@`, `#`, `+`, and any non-ASCII byte — is
/// escaped. Without this, a path containing a space produces no URI at all and
/// the query fails before it is sent.
fn percent_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for byte in path.bytes() {
        match byte {
            // RFC 3986 pchar: unreserved / sub-delims / ":" / "@", plus "/" as
            // the separator. Encoding more than this is not merely redundant —
            // rust-analyzer does not decode a percent-escaped "@", so a fixture
            // directory named `tokio-rs-tokio@be689a35` fell outside its
            // workspace and every call-hierarchy query came back empty.
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b'/'
            | b':'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'=' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Reverses [`percent_encode_path`], including escapes the server chose itself.
fn percent_decode_path(path: &str) -> crate::Result<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            match hex {
                Some(byte) => {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                None => return Err(crate::error::LspError::InvalidUri(path.to_string())),
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| crate::error::LspError::InvalidUri(path.to_string()))
}

/// Converts a file path to an LSP URI.
///
/// A relative path is made absolute against the current directory, with `..`
/// folded lexically. The path is not canonicalized: it need not exist, and
/// symlinks stay unresolved. An absolute path is used as given.
pub fn path_to_uri(path: &std::path::Path) -> crate::Result<Uri> {
    use std::path::Component;

    let resolved;
    let path = if path.is_absolute() {
        path
    } else {
        let abs = std::path::absolute(path).map_err(|e| {
            crate::error::LspError::InvalidUri(format!(
                "{}: cannot resolve against the current directory: {e}",
                path.display()
            ))
        })?;
        let mut folded = PathBuf::new();
        for component in abs.components() {
            match component {
                Component::ParentDir => {
                    if matches!(folded.components().next_back(), Some(Component::Normal(_))) {
                        folded.pop();
                    }
                }
                other => folded.push(other.as_os_str()),
            }
        }
        resolved = folded;
        resolved.as_path()
    };
    // lsp-types 0.97 uses fluent_uri and doesn't have from_file_path,
    // so we construct the URI string manually
    let path_str = path.to_string_lossy();
    let uri_path = if cfg!(windows) {
        windows_path_to_uri_path(&path_str)
    } else {
        path_str.into_owned()
    };
    format!("file://{}", percent_encode_path(&uri_path))
        .parse()
        .map_err(|_| crate::error::LspError::InvalidUri(path.display().to_string()))
}

/// Converts an LSP URI to a file path.
pub fn uri_to_path(uri: &Uri) -> crate::Result<PathBuf> {
    let uri_str = uri.as_str();
    if !uri_str.starts_with("file://") {
        return Err(crate::error::LspError::InvalidUri(uri_str.to_string()));
    }

    let path = percent_decode_path(&uri_str["file://".len()..])?;
    if cfg!(windows) {
        Ok(PathBuf::from(windows_uri_path_to_path(&path)))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// The part of a `file:` URI after `file://` for a Windows path, unencoded.
///
/// `C:\a` gives `/C:/a` and `\\server\share\a` gives `server/share/a`. The
/// verbatim prefixes `\\?\` and `\\?\UNC\` that `std::fs::canonicalize`
/// adds are dropped: language servers reject `file:////?/C:/a`.
fn windows_path_to_uri_path(path: &str) -> String {
    let path = match path.strip_prefix(r"\\?\UNC\") {
        Some(unc) => format!(r"\\{unc}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_string(),
    };
    let path = path.replace('\\', "/");
    match path.strip_prefix("//") {
        Some(unc) => unc.to_string(),
        None if path.starts_with('/') => path,
        None => format!("/{path}"),
    }
}

/// Reverses [`windows_path_to_uri_path`] on a decoded URI path: `/c:/a` gives
/// `C:\a` and `server/share/a` gives `\\server\share\a`. The drive letter is
/// upper-cased, as `std::fs::canonicalize` returns it.
fn windows_uri_path_to_path(path: &str) -> String {
    let path = match path.strip_prefix('/') {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => {
            let mut drive = rest.to_string();
            drive[..1].make_ascii_uppercase();
            drive
        }
        Some(_) => path.to_string(),
        None => format!("//{path}"),
    };
    path.replace('/', "\\")
}

/// Converts an act-lite-core Location to an LSP Position.
pub fn location_to_position(location: &act_lite_core::Location) -> Position {
    Position {
        line: location.line().saturating_sub(1), // LSP is 0-indexed
        character: location.column().saturating_sub(1),
    }
}

/// Converts an act-lite-core Range to an LSP Range.
pub fn range_to_lsp_range(range: &act_lite_core::Range) -> Range {
    Range {
        start: location_to_position(range.start()),
        end: location_to_position(range.end()),
    }
}

/// Converts an LSP Position to an act-lite-core Location.
pub fn position_to_location(position: &Position, file: PathBuf) -> act_lite_core::Location {
    act_lite_core::Location::new(
        file,
        position.line + 1, // act-lite-core is 1-indexed
        position.character + 1,
    )
}

/// Converts an LSP Range to an act-lite-core Range.
pub fn lsp_range_to_range(range: &Range, file: PathBuf) -> act_lite_core::Range {
    let start = position_to_location(&range.start, file.clone());
    let end = position_to_location(&range.end, file);
    act_lite_core::Range::new(start, end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_jsonrpc_request_creation() {
        let req = JsonRpcRequest::new(
            1,
            "textDocument/references".to_string(),
            serde_json::json!({}),
        );
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 1);
        assert_eq!(req.method, "textDocument/references");
    }

    #[test]
    fn test_jsonrpc_response_success() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"result": "ok"})),
            error: None,
        };
        assert_eq!(resp.id, 1);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_error() {
        let error = JsonRpcError {
            code: -32600,
            message: "Invalid request".to_string(),
            data: None,
        };
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: None,
            error: Some(error),
        };
        assert_eq!(resp.id, 1);
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_location_conversion() {
        let location = act_lite_core::Location::new(PathBuf::from("test.ts"), 5, 10);
        let position = location_to_position(&location);

        // LSP is 0-indexed, act-lite-core is 1-indexed
        assert_eq!(position.line, 4);
        assert_eq!(position.character, 9);
    }

    #[test]
    fn test_position_conversion() {
        let position = Position {
            line: 4,
            character: 9,
        };
        let location = position_to_location(&position, PathBuf::from("test.ts"));

        // act-lite-core is 1-indexed
        assert_eq!(location.line(), 5);
        assert_eq!(location.column(), 10);
    }

    #[test]
    fn test_path_to_uri() {
        let path = std::path::Path::new("/home/user/project/src/main.ts");
        let uri = path_to_uri(path).unwrap();
        assert!(uri.as_str().starts_with("file://"));
        assert!(uri.as_str().contains("main.ts"));
    }

    #[cfg(unix)]
    #[test]
    fn test_uri_to_path() {
        let uri: Uri = "file:///home/user/project/src/main.ts".parse().unwrap();
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(path, PathBuf::from("/home/user/project/src/main.ts"));
    }

    #[cfg(windows)]
    #[test]
    fn test_uri_to_path() {
        let uri: Uri = "file:///C:/home/user/project/src/main.ts".parse().unwrap();
        let path = uri_to_path(&uri).unwrap();
        assert_eq!(
            path.as_os_str(),
            r"C:\home\user\project\src\main.ts",
            "backslashes, so the path compares equal to a canonicalized one"
        );
        // vscode-languageserver servers send a lower-case, escaped drive.
        let uri: Uri = "file:///c%3A/x".parse().unwrap();
        assert_eq!(uri_to_path(&uri).unwrap().as_os_str(), r"C:\x");
    }

    /// Windows paths map to URIs language servers accept, and back.
    #[test]
    fn windows_paths_map_to_uri_paths_and_back() {
        for (path, uri_path, back) in [
            (r"C:\a\b c.ts", "/C:/a/b c.ts", r"C:\a\b c.ts"),
            (r"\\?\C:\x", "/C:/x", r"C:\x"),
            (r"\\server\share\x", "server/share/x", r"\\server\share\x"),
            (
                r"\\?\UNC\server\share\x",
                "server/share/x",
                r"\\server\share\x",
            ),
        ] {
            assert_eq!(windows_path_to_uri_path(path), uri_path, "{path}");
            assert_eq!(windows_uri_path_to_path(uri_path), back, "{uri_path}");
        }
        assert_eq!(windows_uri_path_to_path("/c:/x"), r"C:\x");
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_to_uri() {
        for (path, uri) in [
            (r"C:\a\b c.ts", "file:///C:/a/b%20c.ts"),
            (r"\\?\C:\x", "file:///C:/x"),
            (r"\\server\share\x", "file://server/share/x"),
        ] {
            let encoded = path_to_uri(std::path::Path::new(path)).unwrap();
            assert_eq!(encoded.as_str(), uri, "{path}");
        }
        let canonical = std::fs::canonicalize(".").unwrap();
        let uri = path_to_uri(&canonical).unwrap();
        assert!(uri.as_str().starts_with("file:///"), "{uri:?}");
        assert_eq!(uri_to_path(&uri).unwrap(), std::env::current_dir().unwrap());
    }

    /// Paths that need percent-encoding must survive the round trip.
    ///
    /// Every path in a query result comes back through `uri_to_path`. A path
    /// that arrives as `.../psf-requests%40f361ead0/models.py` names no file on
    /// disk, so the caller list it appears in cannot be opened or jumped to.
    #[test]
    fn test_path_uri_round_trip_with_reserved_characters() {
        let root = if cfg!(windows) {
            r"C:\home\user"
        } else {
            "/home/user"
        };
        for tail in [
            "fixtures/psf-requests@f361ead0/src/main.py",
            "fixtures/a+b(c),d;e=f/main.py",
            "my project/src/main.ts",
            "proj#1/a+b/main.rs",
            "проект/main.rs",
        ] {
            let original = std::path::Path::new(root).join(tail);
            let uri = path_to_uri(&original).expect("path should encode");
            let back = uri_to_path(&uri).expect("uri should decode");
            assert_eq!(back, original, "round trip failed via {uri:?}");
        }
    }

    #[test]
    fn test_uri_to_path_invalid() {
        let uri: Uri = "https://example.com/file.ts".parse().unwrap();
        let result = uri_to_path(&uri);
        assert!(result.is_err());
    }

    #[test]
    fn test_range_to_lsp_range() {
        let start = act_lite_core::Location::new(PathBuf::from("test.ts"), 5, 10);
        let end = act_lite_core::Location::new(PathBuf::from("test.ts"), 5, 20);
        let range = act_lite_core::Range::new(start, end);

        let lsp_range = range_to_lsp_range(&range);

        // LSP is 0-indexed
        assert_eq!(lsp_range.start.line, 4);
        assert_eq!(lsp_range.start.character, 9);
        assert_eq!(lsp_range.end.line, 4);
        assert_eq!(lsp_range.end.character, 19);
    }

    #[test]
    fn test_lsp_range_to_range() {
        let lsp_range = Range {
            start: Position {
                line: 4,
                character: 9,
            },
            end: Position {
                line: 4,
                character: 19,
            },
        };

        let range = lsp_range_to_range(&lsp_range, PathBuf::from("test.ts"));

        // act-lite-core is 1-indexed
        assert_eq!(range.start().line(), 5);
        assert_eq!(range.start().column(), 10);
        assert_eq!(range.end().line(), 5);
        assert_eq!(range.end().column(), 20);
    }

    #[test]
    fn test_jsonrpc_notification_creation() {
        let notif = JsonRpcNotification::new(
            "textDocument/didOpen".to_string(),
            serde_json::json!({"textDocument": {"uri": "file:///test.ts"}}),
        );
        assert_eq!(notif.jsonrpc, "2.0");
        assert_eq!(notif.method, "textDocument/didOpen");
    }

    #[test]
    fn test_location_at_line_1_col_1() {
        // Edge case: first line, first column
        let location = act_lite_core::Location::new(PathBuf::from("test.ts"), 1, 1);
        let position = location_to_position(&location);

        // LSP is 0-indexed, so should be (0, 0)
        assert_eq!(position.line, 0);
        assert_eq!(position.character, 0);
    }

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest::new(
            42,
            "test/method".to_string(),
            serde_json::json!({"param": 1}),
        );
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":42"));
        assert!(json.contains("\"method\":\"test/method\""));
    }

    #[test]
    fn test_jsonrpc_response_serialization() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!({"data": "test"})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"result\""));
        assert!(!json.contains("\"error\"")); // error should be skipped when None
    }

    #[test]
    fn test_jsonrpc_error_response_serialization() {
        let error = JsonRpcError {
            code: -32600,
            message: "Invalid".to_string(),
            data: None,
        };
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: None,
            error: Some(error),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(!json.contains("\"result\"")); // result should be skipped when None
    }

    #[test]
    fn test_jsonrpc_notification_serialization() {
        let notif = JsonRpcNotification::new("test/event".to_string(), serde_json::json!({}));
        let json = serde_json::to_string(&notif).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test/event\""));
        assert!(!json.contains("\"id\"")); // notifications don't have id
    }

    #[test]
    fn test_jsonrpc_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":5,"method":"test","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.id, 5);
        assert_eq!(req.method, "test");
    }

    #[test]
    fn test_jsonrpc_response_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":5,"result":{"value":42}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 5);
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_jsonrpc_notification_deserialization() {
        let json = r#"{"jsonrpc":"2.0","method":"notify","params":{"x":1}}"#;
        let notif: JsonRpcNotification = serde_json::from_str(json).unwrap();
        assert_eq!(notif.method, "notify");
    }

    #[test]
    fn test_jsonrpc_request_with_complex_params() {
        let params = serde_json::json!({
            "textDocument": {
                "uri": "file:///test.ts"
            },
            "position": {
                "line": 10,
                "character": 5
            },
            "nested": {
                "array": [1, 2, 3],
                "object": {
                    "key": "value"
                }
            }
        });

        let request = JsonRpcRequest::new(42, "test/method".to_string(), params);
        let serialized = serde_json::to_string(&request).unwrap();
        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.id, 42);
        assert_eq!(deserialized.method, "test/method");
        assert!(deserialized.params.get("nested").is_some());
    }

    #[test]
    fn test_jsonrpc_notification_with_empty_params() {
        let notification = JsonRpcNotification::new("exit".to_string(), serde_json::json!(null));

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains("\"params\":null"));
    }

    #[test]
    fn test_jsonrpc_response_with_null_result() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: Some(serde_json::json!(null)),
            error: None,
        };
        assert!(response.result.is_some());
        assert!(response.result.unwrap().is_null());
    }

    #[test]
    fn test_jsonrpc_error_with_data() {
        let error = JsonRpcError {
            code: -32603,
            message: "Internal error".to_string(),
            data: Some(serde_json::json!({
                "details": "Something went wrong",
                "stack": ["line1", "line2"]
            })),
        };

        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 1,
            result: None,
            error: Some(error),
        };
        let err = response.error.unwrap();
        assert!(err.data.is_some());
        assert!(err.data.unwrap().get("stack").is_some());
    }

    #[test]
    fn test_position_at_large_values() {
        let location = act_lite_core::Location::new(PathBuf::from("test.ts"), 10000, 500);
        let position = location_to_position(&location);

        // LSP is 0-indexed
        assert_eq!(position.line, 9999);
        assert_eq!(position.character, 499);
    }

    #[test]
    fn test_range_spanning_multiple_lines() {
        let start = act_lite_core::Location::new(PathBuf::from("test.ts"), 1, 1);
        let end = act_lite_core::Location::new(PathBuf::from("test.ts"), 100, 50);
        let range = act_lite_core::Range::new(start, end);

        let lsp_range = range_to_lsp_range(&range);
        assert_eq!(lsp_range.start.line, 0);
        assert_eq!(lsp_range.start.character, 0);
        assert_eq!(lsp_range.end.line, 99);
        assert_eq!(lsp_range.end.character, 49);
    }

    #[test]
    fn test_lsp_range_at_document_start() {
        let lsp_range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        };

        let range = lsp_range_to_range(&lsp_range, PathBuf::from("test.ts"));
        assert_eq!(range.start().line(), 1);
        assert_eq!(range.start().column(), 1);
        assert_eq!(range.end().line(), 1);
        assert_eq!(range.end().column(), 1);
    }

    #[test]
    fn test_jsonrpc_request_roundtrip() {
        let request = JsonRpcRequest::new(
            123,
            "textDocument/hover".to_string(),
            serde_json::json!({"key": "value"}),
        );

        let json = serde_json::to_string(&request).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.id, 123);
        assert_eq!(parsed.method, "textDocument/hover");
    }

    #[test]
    fn test_jsonrpc_notification_roundtrip() {
        let notification =
            JsonRpcNotification::new("initialized".to_string(), serde_json::json!({}));

        let json = serde_json::to_string(&notification).unwrap();
        let parsed: JsonRpcNotification = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.method, "initialized");
    }

    #[test]
    fn test_jsonrpc_response_roundtrip_success() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 42,
            result: Some(serde_json::json!({"hover": "text"})),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.id, 42);
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn test_jsonrpc_response_roundtrip_error() {
        let response = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: 42,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Method not found".to_string(),
                data: None,
            }),
        };

        let json = serde_json::to_string(&response).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.id, 42);
        assert!(parsed.result.is_none());
        assert!(parsed.error.is_some());
        assert_eq!(parsed.error.unwrap().code, -32601);
    }

    #[test]
    fn path_to_uri_relative_is_absolute_with_empty_host() {
        let cwd = std::env::current_dir().unwrap();
        let uri = path_to_uri(Path::new("src/x.ts")).unwrap();
        assert!(uri.as_str().starts_with("file:///"));
        assert!(!uri.as_str().starts_with("file://src"));
        assert_eq!(uri_to_path(&uri).unwrap(), cwd.join("src/x.ts"));
    }

    #[cfg(unix)]
    #[test]
    fn path_to_uri_absolute_input_unchanged() {
        let uri = path_to_uri(Path::new("/a/b/../c d.ts")).unwrap();
        assert_eq!(uri.as_str(), "file:///a/b/../c%20d.ts");
    }

    #[test]
    fn path_to_uri_relative_encodes_space_and_non_ascii() {
        let uri = path_to_uri(Path::new("my dir/\u{fc}.ts")).unwrap();
        assert!(uri.as_str().ends_with("/my%20dir/%C3%BC.ts"));
        assert_eq!(
            uri_to_path(&uri).unwrap(),
            std::env::current_dir().unwrap().join("my dir/\u{fc}.ts")
        );
    }

    #[test]
    fn path_to_uri_relative_folds_parent_components() {
        let cwd = std::env::current_dir().unwrap();
        let inner = path_to_uri(Path::new("a/../b.ts")).unwrap();
        assert_eq!(uri_to_path(&inner).unwrap(), cwd.join("b.ts"));
        let outer = path_to_uri(Path::new("../x.ts")).unwrap();
        assert_eq!(
            uri_to_path(&outer).unwrap(),
            cwd.parent().unwrap().join("x.ts")
        );
    }

    #[test]
    fn path_to_uri_relative_nonexistent_file_succeeds() {
        assert!(path_to_uri(Path::new("no/such/new_file.rs")).is_ok());
    }
}
