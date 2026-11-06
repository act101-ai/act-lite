//! Error types for parser operations.

use crate::query::QueryType;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during parsing and query operations.
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("unsupported language for extension: {extension}")]
    UnsupportedLanguage { extension: String },

    #[error("failed to parse {file}: {reason}")]
    ParseFailed { file: PathBuf, reason: String },

    #[error(
        "parse of {file} exceeded the {timeout_ms} ms deadline (input may be pathological / adversarial)"
    )]
    ParseTimeout { file: PathBuf, timeout_ms: u64 },

    #[error("query execution failed: {0}")]
    QueryFailed(#[from] tree_sitter::QueryError),

    #[error("symbol not found: {name} in {file}")]
    SymbolNotFound { name: String, file: PathBuf },
    #[error(
        "the name occurs in an unmodeled reference form at {file}:{line} ({form}) — refusing: rename only rewrites modeled reference forms"
    )]
    UnmodeledReferenceForm {
        file: PathBuf,
        line: u32,
        form: String,
    },

    #[error("invalid position: line {line}, column {column} in {file}")]
    InvalidPosition {
        file: PathBuf,
        line: u32,
        column: u32,
    },

    #[error("query not implemented: {query:?} requires LSP (remove --disable-lsp)")]
    NotImplemented { query: QueryType },

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid UTF-8 in file {file}")]
    InvalidUtf8 { file: PathBuf },

    #[error("workspace not initialized")]
    WorkspaceNotInitialized,

    #[error(
        "references workspace walk hit the file-count cap of {max_files} files \
         ({candidates} same-grammar candidates under {}); narrow the workspace to a smaller directory",
        root.display()
    )]
    ReferencesWalkCap {
        root: PathBuf,
        candidates: usize,
        max_files: usize,
    },

    #[error("unsupported source format in {file}: {detected}. {guidance}")]
    UnsupportedSourceFormat {
        file: PathBuf,
        detected: String,
        guidance: String,
    },

    #[error("the load was cancelled")]
    Cancelled,
}

impl ParserError {
    /// Returns a machine-readable error code for this error.
    ///
    /// These codes are stable identifiers that agents can use to handle
    /// specific error conditions programmatically.
    pub fn code(&self) -> &'static str {
        match self {
            ParserError::UnsupportedLanguage { .. } => "unsupported_language",
            ParserError::ParseFailed { .. } => "parse_failed",
            ParserError::ParseTimeout { .. } => "parse_timeout",
            ParserError::QueryFailed(_) => "query_failed",
            ParserError::SymbolNotFound { .. } => "symbol_not_found",
            ParserError::UnmodeledReferenceForm { .. } => "unmodeled_reference_form",
            ParserError::InvalidPosition { .. } => "invalid_position",
            ParserError::NotImplemented { .. } => "not_implemented",
            ParserError::Io(_) => "io_error",
            ParserError::InvalidUtf8 { .. } => "invalid_utf8",
            ParserError::WorkspaceNotInitialized => "workspace_not_initialized",
            ParserError::ReferencesWalkCap { .. } => "references_walk_cap",
            ParserError::UnsupportedSourceFormat { .. } => "unsupported_source_format",
            ParserError::Cancelled => "cancelled",
        }
    }
}

/// Result type for parser operations.
pub type Result<T> = std::result::Result<T, ParserError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_unsupported_language() {
        let err = ParserError::UnsupportedLanguage {
            extension: "xyz".to_string(),
        };
        assert_eq!(err.code(), "unsupported_language");
        assert!(err.to_string().contains("xyz"));
    }

    #[test]
    fn error_code_parse_failed() {
        let err = ParserError::ParseFailed {
            file: PathBuf::from("test.ts"),
            reason: "syntax error".to_string(),
        };
        assert_eq!(err.code(), "parse_failed");
        assert!(err.to_string().contains("test.ts"));
        assert!(err.to_string().contains("syntax error"));
    }

    #[test]
    fn error_code_parse_timeout() {
        let err = ParserError::ParseTimeout {
            file: PathBuf::from("huge.ts"),
            timeout_ms: 2000,
        };
        assert_eq!(err.code(), "parse_timeout");
        assert!(err.to_string().contains("huge.ts"));
        assert!(err.to_string().contains("2000"));
    }

    #[test]
    fn error_code_symbol_not_found() {
        let err = ParserError::SymbolNotFound {
            name: "foo".to_string(),
            file: PathBuf::from("test.ts"),
        };
        assert_eq!(err.code(), "symbol_not_found");
        assert!(err.to_string().contains("foo"));
        assert!(err.to_string().contains("test.ts"));
    }

    #[test]
    fn error_code_references_walk_cap() {
        let err = ParserError::ReferencesWalkCap {
            root: PathBuf::from("/repo"),
            candidates: 10_001,
            max_files: 10_000,
        };
        assert_eq!(err.code(), "references_walk_cap");
        assert!(err.to_string().contains("10000"));
        assert!(err.to_string().contains("/repo"));
    }

    #[test]
    fn error_code_invalid_position() {
        let err = ParserError::InvalidPosition {
            file: PathBuf::from("test.ts"),
            line: 10,
            column: 5,
        };
        assert_eq!(err.code(), "invalid_position");
        assert!(err.to_string().contains("line 10"));
        assert!(err.to_string().contains("column 5"));
    }

    #[test]
    fn error_code_not_implemented() {
        let err = ParserError::NotImplemented {
            query: QueryType::References,
        };
        assert_eq!(err.code(), "not_implemented");
        assert!(err.to_string().contains("References"));
    }

    #[test]
    fn error_code_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = ParserError::Io(io_err);
        assert_eq!(err.code(), "io_error");
    }

    #[test]
    fn error_code_invalid_utf8() {
        let err = ParserError::InvalidUtf8 {
            file: PathBuf::from("binary.bin"),
        };
        assert_eq!(err.code(), "invalid_utf8");
        assert!(err.to_string().contains("binary.bin"));
    }

    #[test]
    fn error_code_workspace_not_initialized() {
        let err = ParserError::WorkspaceNotInitialized;
        assert_eq!(err.code(), "workspace_not_initialized");
        assert!(err.to_string().contains("not initialized"));
    }

    #[test]
    fn error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err: ParserError = io_err.into();
        assert_eq!(err.code(), "io_error");
    }

    #[test]
    fn error_code_query_failed() {
        // Create a QueryError by attempting to parse an invalid query
        let source = crate::SourceFile::from_content(
            "test.ts",
            "const x = 1;".to_string(),
            crate::Grammar::TypeScript,
        )
        .unwrap();
        let query_err =
            tree_sitter::Query::new(&source.tree().root_node().language(), "invalid syntax [[[")
                .err()
                .unwrap();
        let err = ParserError::QueryFailed(query_err);
        assert_eq!(err.code(), "query_failed");
    }
}
