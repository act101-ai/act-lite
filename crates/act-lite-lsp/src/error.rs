//! Error types for LSP operations.

use std::io;
use thiserror::Error;

/// Errors that can occur during LSP operations.
#[derive(Debug, Error)]
pub enum LspError {
    /// Failed to spawn the LSP server process.
    #[error("failed to spawn LSP server for {language}: {source}")]
    ServerSpawnFailed {
        language: String,
        #[source]
        source: io::Error,
    },

    /// Failed to communicate with the LSP server.
    #[error("LSP communication error for {language}: {message}")]
    CommunicationError { language: String, message: String },

    /// LSP server process terminated unexpectedly.
    #[error("LSP server terminated unexpectedly for {language}")]
    ServerTerminated { language: String },

    /// Request timed out.
    #[error("LSP request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// The server had not finished analysing the document when it was asked,
    /// so its answer would come from a partial index.
    ///
    /// This is NOT "the answer was empty" — it is "the answer is unknown". The
    /// distinction is load-bearing for anything that caches: an empty answer
    /// from a partial index, recorded as fact, is served as a confirmed
    /// "nothing here" until the file's content changes.
    #[error(
        "{language} had not finished analysing {path} within {timeout_ms}ms, so any \
         answer about it now comes from a partial index and cannot be trusted or \
         cached; the timeout is recorded for this client's lifetime, so a retry in \
         the same session keeps refusing — re-run in a new session (new client) once \
         the server has settled, or raise the readiness budget if this repository is \
         consistently slower than that to analyse"
    )]
    AnalysisNotReady {
        language: String,
        path: String,
        timeout_ms: u64,
    },

    /// The server answered a nullable-result request with a wire `null`.
    ///
    /// This is NOT "the answer was empty" — it is "the server gave no answer".
    /// The distinction is load-bearing for anything that records or caches:
    /// a null recorded as an empty list is served as a confirmed "nothing
    /// here" — a measured zero — when the true answer is unknown. An empty
    /// list from the server stays `Ok`; only `null` takes this path.
    #[error(
        "the language server answered {method} with null — the answer is unknown, \
         not empty; it must not be recorded as a measured zero"
    )]
    NullResult { method: &'static str },

    /// Failed to serialize/deserialize LSP messages.
    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// LSP server returned an error response.
    #[error("LSP error from {language}: code {code}, message: {message}")]
    ResponseError {
        language: String,
        code: i32,
        message: String,
    },

    /// LSP server does not support the requested capability.
    #[error("LSP server for {language} does not support {capability}")]
    UnsupportedCapability {
        language: String,
        capability: String,
    },

    #[error(
        "the language server does not support {operation}: its initialize result does \
         not advertise {capability}"
    )]
    NotAdvertised {
        operation: &'static str,
        capability: &'static str,
    },

    /// LSP server is not initialized.
    #[error("LSP client not initialized for {language}")]
    NotInitialized { language: String },

    /// Invalid URI.
    #[error("invalid URI: {0}")]
    InvalidUri(String),

    /// Invalid LSP server command.
    #[error("invalid LSP server command '{command}': not in allowed list. Allowed: {allowed:?}")]
    InvalidCommand {
        command: String,
        allowed: Vec<String>,
    },

    /// Invalid command argument.
    #[error("invalid argument '{argument}': {reason}")]
    InvalidArgument { argument: String, reason: String },

    /// LSP server binary is not installed.
    #[error("LSP server '{command}' for {language} is not installed. Install with: {install_hint}")]
    NotInstalled {
        language: String,
        command: String,
        install_hint: String,
    },

    /// Auto-installation of LSP server failed.
    #[error("Failed to auto-install LSP for {language} via `{command}`: {reason}")]
    InstallFailed {
        language: String,
        command: String,
        reason: String,
    },

    /// Failed to read or write a file.
    #[error("I/O error during '{operation}' for {}: {source}", path.display())]
    IoError {
        operation: String,
        path: std::path::PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Result type for LSP operations.
pub type Result<T> = std::result::Result<T, LspError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_server_terminated() {
        let err = LspError::ServerTerminated {
            language: "typescript".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "LSP server terminated unexpectedly for typescript"
        );
    }

    #[test]
    fn test_error_display_timeout() {
        let err = LspError::Timeout { timeout_ms: 5000 };
        assert_eq!(err.to_string(), "LSP request timed out after 5000ms");
    }

    #[test]
    fn test_error_display_communication_error() {
        let err = LspError::CommunicationError {
            language: "rust".to_string(),
            message: "connection refused".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "LSP communication error for rust: connection refused"
        );
    }

    #[test]
    fn test_error_display_response_error() {
        let err = LspError::ResponseError {
            language: "typescript".to_string(),
            code: -32600,
            message: "Invalid request".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "LSP error from typescript: code -32600, message: Invalid request"
        );
    }

    #[test]
    fn test_error_display_invalid_uri() {
        let err = LspError::InvalidUri("not-a-valid-uri".to_string());
        assert_eq!(err.to_string(), "invalid URI: not-a-valid-uri");
    }

    #[test]
    fn test_error_display_invalid_command() {
        let err = LspError::InvalidCommand {
            command: "rm".to_string(),
            allowed: vec![
                "rust-analyzer".to_string(),
                "typescript-language-server".to_string(),
            ],
        };
        assert!(err.to_string().contains("rm"));
        assert!(err.to_string().contains("not in allowed list"));
    }

    #[test]
    fn test_error_display_invalid_argument() {
        let err = LspError::InvalidArgument {
            argument: "arg; rm -rf /".to_string(),
            reason: "contains shell metacharacter".to_string(),
        };
        assert!(err.to_string().contains("shell metacharacter"));
    }

    #[test]
    fn test_error_from_serde_json() {
        let json_err = serde_json::from_str::<String>("not valid json").unwrap_err();
        let lsp_err: LspError = json_err.into();
        assert!(matches!(lsp_err, LspError::SerializationError(_)));
    }

    #[test]
    fn test_error_display_server_spawn_failed() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "command not found");
        let err = LspError::ServerSpawnFailed {
            language: "rust".to_string(),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("spawn"));
        assert!(msg.contains("rust"));
    }

    #[test]
    fn test_error_display_unsupported_capability() {
        let err = LspError::UnsupportedCapability {
            language: "python".to_string(),
            capability: "rename".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("python"));
        assert!(msg.contains("rename"));
        assert!(msg.contains("not support"));
    }

    #[test]
    fn test_error_display_not_initialized() {
        let err = LspError::NotInitialized {
            language: "go".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("not initialized"));
        assert!(msg.contains("go"));
    }

    #[test]
    fn test_error_display_io_error() {
        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "access denied");
        let err = LspError::IoError {
            operation: "write".to_string(),
            path: std::path::PathBuf::from("/tmp/test.ts"),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("write"));
        assert!(msg.contains("test.ts"));
    }

    #[test]
    fn test_error_debug_trait() {
        let err = LspError::Timeout { timeout_ms: 5000 };
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("Timeout"));
        assert!(debug_str.contains("5000"));
    }

    #[test]
    fn test_error_source_server_spawn() {
        use std::error::Error;

        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let err = LspError::ServerSpawnFailed {
            language: "test".to_string(),
            source: io_err,
        };

        // Error trait source() should return the underlying io::Error
        let source = err.source();
        assert!(source.is_some());
    }

    #[test]
    fn test_error_source_io_error() {
        use std::error::Error;

        let io_err = io::Error::new(io::ErrorKind::NotFound, "not found");
        let err = LspError::IoError {
            operation: "read".to_string(),
            path: std::path::PathBuf::from("test.ts"),
            source: io_err,
        };

        let source = err.source();
        assert!(source.is_some());
    }

    #[test]
    fn test_error_source_serialization() {
        use std::error::Error;

        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: LspError = json_err.into();

        let source = err.source();
        assert!(source.is_some());
    }

    #[test]
    fn test_error_variants_without_source() {
        use std::error::Error;

        let err = LspError::Timeout { timeout_ms: 1000 };
        assert!(err.source().is_none());

        let err = LspError::CommunicationError {
            language: "test".to_string(),
            message: "error".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::ServerTerminated {
            language: "test".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::ResponseError {
            language: "test".to_string(),
            code: -1,
            message: "error".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::UnsupportedCapability {
            language: "test".to_string(),
            capability: "test".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::NotInitialized {
            language: "test".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::InvalidUri("test".to_string());
        assert!(err.source().is_none());

        let err = LspError::InvalidCommand {
            command: "test".to_string(),
            allowed: vec![],
        };
        assert!(err.source().is_none());

        let err = LspError::InvalidArgument {
            argument: "test".to_string(),
            reason: "reason".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::NotInstalled {
            language: "test".to_string(),
            command: "test".to_string(),
            install_hint: "test".to_string(),
        };
        assert!(err.source().is_none());

        let err = LspError::InstallFailed {
            language: "test".to_string(),
            command: "test".to_string(),
            reason: "test".to_string(),
        };
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_display_not_installed() {
        let err = LspError::NotInstalled {
            language: "typescript".to_string(),
            command: "typescript-language-server".to_string(),
            install_hint: "npm install -g typescript-language-server typescript".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "LSP server 'typescript-language-server' for typescript is not installed. Install with: npm install -g typescript-language-server typescript"
        );
    }

    #[test]
    fn test_error_display_null_result() {
        let err = LspError::NullResult {
            method: "textDocument/references",
        };
        let msg = err.to_string();
        assert!(msg.contains("textDocument/references"), "{msg}");
        assert!(msg.contains("null"), "{msg}");
        assert!(
            msg.contains("not") && (msg.contains("empty") || msg.contains("unknown")),
            "the message must say the answer is unknown, not empty: {msg}"
        );
    }

    #[test]
    fn test_error_display_install_failed() {
        let err = LspError::InstallFailed {
            language: "typescript".to_string(),
            command: "npm install -g typescript-language-server".to_string(),
            reason: "npm not found".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Failed to auto-install LSP for typescript via `npm install -g typescript-language-server`: npm not found"
        );
    }
}
