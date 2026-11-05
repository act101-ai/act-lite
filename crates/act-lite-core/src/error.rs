//! Error types for act.
//!
//! Errors in act are designed to be actionable. Every error provides:
//! - A machine-readable code for programmatic handling
//! - A human-readable message for display
//! - Structured details for recovery actions
//!
//! # Design Principles
//!
//! - Errors tell the user/agent what went wrong AND how to fix it
//! - Error codes are stable and can be matched programmatically
//! - Details provide context for intelligent recovery

use crate::Range;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level error category for routing and recovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    /// Target symbol or location couldn't be found or resolved.
    TargetResolution,
    /// Operation preconditions not met.
    Validation,
    /// Multiple operations have conflicting edits.
    Conflict,
    /// External tool (LSP) unavailable or failed.
    External,
    /// Safety guardrail blocked the operation.
    Guardrail,
    /// Internal error (bug or unexpected state).
    Internal,
}

impl ErrorCategory {
    /// Returns a hint about how to recover from this category of error.
    pub fn recovery_hint(&self) -> &'static str {
        match self {
            ErrorCategory::TargetResolution => {
                "Retry with a more specific target or check that the symbol exists"
            }
            ErrorCategory::Validation => "Check preconditions or choose a different operation",
            ErrorCategory::Conflict => "Reorder operations or split into separate batches",
            ErrorCategory::External => "Check LSP server status or use --no-lsp flag",
            ErrorCategory::Guardrail => {
                "Review the change carefully; use history to undo if needed"
            }
            ErrorCategory::Internal => "This may be a bug; please report it",
        }
    }
}

/// Machine-readable error codes.
///
/// These codes are stable and can be used for programmatic error handling.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    // Target resolution errors
    /// The specified target was not found.
    TargetNotFound,
    /// Multiple matches for the target; clarification needed.
    TargetAmbiguous,
    /// The file does not exist.
    FileNotFound,
    /// The file could not be parsed.
    ParseError,

    // Validation errors
    /// The operation is not valid for this target.
    InvalidOperation,
    /// Required preconditions are not met.
    PreconditionFailed,
    /// Invalid arguments provided.
    InvalidArgument,

    // Conflict errors
    /// Two operations have overlapping edits.
    EditConflict,
    /// File was modified outside of act.
    ExternalModification,

    // External errors
    /// Language server is not available.
    LspUnavailable,
    /// Language server request timed out.
    LspTimeout,
    /// Language server returned an error.
    LspError,

    // Guardrail errors
    /// Mass deletion blocked (>50% file shrink).
    MassDeletionBlocked,
    /// Operation would break syntax.
    SyntaxBreakBlocked,
    /// Circular import would be created.
    CircularImportBlocked,

    // Internal errors
    /// Unexpected internal state.
    InternalError,
}

impl ErrorCode {
    /// Returns the category for this error code.
    pub fn category(&self) -> ErrorCategory {
        match self {
            ErrorCode::TargetNotFound
            | ErrorCode::TargetAmbiguous
            | ErrorCode::FileNotFound
            | ErrorCode::ParseError => ErrorCategory::TargetResolution,

            ErrorCode::InvalidOperation
            | ErrorCode::PreconditionFailed
            | ErrorCode::InvalidArgument => ErrorCategory::Validation,

            ErrorCode::EditConflict | ErrorCode::ExternalModification => ErrorCategory::Conflict,

            ErrorCode::LspUnavailable | ErrorCode::LspTimeout | ErrorCode::LspError => {
                ErrorCategory::External
            }

            ErrorCode::MassDeletionBlocked
            | ErrorCode::SyntaxBreakBlocked
            | ErrorCode::CircularImportBlocked => ErrorCategory::Guardrail,

            ErrorCode::InternalError => ErrorCategory::Internal,
        }
    }
}

/// A candidate match when target resolution is ambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Display name for this candidate.
    pub name: String,
    /// Location of the candidate.
    pub location: Range,
    /// Additional context (e.g., type signature).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Structured error details for recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ErrorDetails {
    /// No additional details.
    None,

    /// Multiple candidates match the target.
    Ambiguous {
        /// The candidates that match.
        candidates: Vec<Candidate>,
    },

    /// Two operations conflict at a location.
    Conflict {
        /// Index of first conflicting operation.
        operation1: usize,
        /// Index of second conflicting operation.
        operation2: usize,
        /// Location of the conflict.
        location: Range,
    },

    /// File was modified externally.
    ExternalModification {
        /// The file that was modified.
        file: PathBuf,
        /// Hash we expected.
        expected_hash: String,
        /// Hash we found.
        actual_hash: String,
    },

    /// LSP server is unavailable.
    LspUnavailable {
        /// The language that needs LSP.
        language: String,
        /// Whether fallback to tree-sitter is possible.
        fallback_available: bool,
    },

    /// A guardrail was triggered.
    GuardrailTriggered {
        /// The rule that was triggered.
        rule: String,
        /// Suggested action.
        suggestion: String,
    },

    /// Parse error details.
    ParseError {
        /// The file that failed to parse.
        file: PathBuf,
        /// Error message from parser.
        message: String,
        /// Location of the error if known.
        #[serde(skip_serializing_if = "Option::is_none")]
        location: Option<Range>,
    },
}

/// The main error type for act operations.
///
/// Every error provides:
/// - A machine-readable code
/// - A human-readable message
/// - A category for routing
/// - Structured details for recovery
///
/// # Examples
///
/// ```
/// use act_lite_core::{ActError, ErrorCode, ErrorDetails};
///
/// let error = ActError::target_not_found("getUserById", "src/users.ts");
///
/// assert_eq!(error.code(), &ErrorCode::TargetNotFound);
/// assert!(error.message().contains("getUserById"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct ActError {
    /// Machine-readable error code.
    code: ErrorCode,
    /// Human-readable error message.
    message: String,
    /// Error category for routing.
    category: ErrorCategory,
    /// Structured details for recovery.
    details: ErrorDetails,
}

impl ActError {
    /// Creates a new error.
    pub fn new(code: ErrorCode, message: String, details: ErrorDetails) -> Self {
        let category = code.category();
        Self {
            code,
            message,
            category,
            details,
        }
    }

    /// Returns the error code.
    pub fn code(&self) -> &ErrorCode {
        &self.code
    }

    /// Returns the error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the error category.
    pub fn category(&self) -> ErrorCategory {
        self.category
    }

    /// Returns the error details.
    pub fn details(&self) -> &ErrorDetails {
        &self.details
    }

    /// Returns a recovery hint based on the error category.
    pub fn recovery_hint(&self) -> &'static str {
        self.category.recovery_hint()
    }

    // Convenience constructors for common errors

    /// Creates a "target not found" error.
    pub fn target_not_found(target: &str, searched_in: &str) -> Self {
        Self::new(
            ErrorCode::TargetNotFound,
            format!(
                "Symbol '{}' not found in {}. Check spelling and ensure the symbol exists.",
                target, searched_in
            ),
            ErrorDetails::None,
        )
    }

    /// Creates an "ambiguous target" error.
    pub fn target_ambiguous(target: &str, candidates: Vec<Candidate>) -> Self {
        let locations: Vec<_> = candidates.iter().map(|c| c.location.to_string()).collect();
        Self::new(
            ErrorCode::TargetAmbiguous,
            format!(
                "Symbol '{}' is ambiguous. Found {} matches at: {}. Qualify with file path.",
                target,
                candidates.len(),
                locations.join(", ")
            ),
            ErrorDetails::Ambiguous { candidates },
        )
    }

    /// Creates a "file not found" error.
    pub fn file_not_found(path: &Path) -> Self {
        Self::new(
            ErrorCode::FileNotFound,
            format!("File '{}' not found. Check the path.", path.display()),
            ErrorDetails::None,
        )
    }

    /// Creates a "parse error" error.
    pub fn parse_error(file: PathBuf, message: String, location: Option<Range>) -> Self {
        Self::new(
            ErrorCode::ParseError,
            format!("Failed to parse '{}': {}", file.display(), message),
            ErrorDetails::ParseError {
                file,
                message,
                location,
            },
        )
    }

    /// Creates an "invalid operation" error.
    pub fn invalid_operation(operation: &str, reason: &str) -> Self {
        Self::new(
            ErrorCode::InvalidOperation,
            format!("Operation '{}' is not valid: {}", operation, reason),
            ErrorDetails::None,
        )
    }

    /// Creates a "precondition failed" error.
    pub fn precondition_failed(operation: &str, precondition: &str) -> Self {
        Self::new(
            ErrorCode::PreconditionFailed,
            format!("Operation '{}' requires: {}", operation, precondition),
            ErrorDetails::None,
        )
    }

    /// Creates an "edit conflict" error.
    pub fn edit_conflict(op1: usize, op2: usize, location: Range) -> Self {
        Self::new(
            ErrorCode::EditConflict,
            format!(
                "Operation {} conflicts with operation {} at {}. Reorder or split operations.",
                op1, op2, location
            ),
            ErrorDetails::Conflict {
                operation1: op1,
                operation2: op2,
                location,
            },
        )
    }

    /// Creates an "external modification" error.
    pub fn external_modification(file: PathBuf, expected: String, actual: String) -> Self {
        Self::new(
            ErrorCode::ExternalModification,
            format!(
                "File '{}' was modified externally. Cannot safely undo. Refresh state first.",
                file.display()
            ),
            ErrorDetails::ExternalModification {
                file,
                expected_hash: expected,
                actual_hash: actual,
            },
        )
    }

    /// Creates an "LSP unavailable" error.
    pub fn lsp_unavailable(language: &str, fallback_available: bool) -> Self {
        let suggestion = if fallback_available {
            "Tree-sitter fallback available; some features may be limited."
        } else {
            "Install the language server or use --no-lsp."
        };
        Self::new(
            ErrorCode::LspUnavailable,
            format!(
                "Language server for {} is not available. {}",
                language, suggestion
            ),
            ErrorDetails::LspUnavailable {
                language: language.to_string(),
                fallback_available,
            },
        )
    }

    /// Creates an "LSP timeout" error.
    pub fn lsp_timeout(operation: &str, timeout_ms: u64) -> Self {
        Self::new(
            ErrorCode::LspTimeout,
            format!(
                "Language server request '{}' timed out after {}ms. Server may be busy or unresponsive.",
                operation, timeout_ms
            ),
            ErrorDetails::None,
        )
    }

    /// Creates a "guardrail triggered" error.
    pub fn guardrail_triggered(rule: &str, suggestion: &str) -> Self {
        Self::new(
            ErrorCode::MassDeletionBlocked, // Or derive from rule
            format!("Safety guardrail triggered: {}. {}", rule, suggestion),
            ErrorDetails::GuardrailTriggered {
                rule: rule.to_string(),
                suggestion: suggestion.to_string(),
            },
        )
    }

    /// Creates an "internal error" error.
    pub fn internal(message: &str) -> Self {
        Self::new(
            ErrorCode::InternalError,
            format!(
                "Internal error: {}. This may be a bug; please report it.",
                message
            ),
            ErrorDetails::None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_category() {
        assert_eq!(
            ErrorCode::TargetNotFound.category(),
            ErrorCategory::TargetResolution
        );
        assert_eq!(ErrorCode::EditConflict.category(), ErrorCategory::Conflict);
        assert_eq!(
            ErrorCode::LspUnavailable.category(),
            ErrorCategory::External
        );
        assert_eq!(
            ErrorCode::MassDeletionBlocked.category(),
            ErrorCategory::Guardrail
        );
    }

    #[test]
    fn error_target_not_found() {
        let err = ActError::target_not_found("foo", "src/bar.ts");
        assert_eq!(err.code(), &ErrorCode::TargetNotFound);
        assert!(err.message().contains("foo"));
        assert!(err.message().contains("src/bar.ts"));
        assert_eq!(err.category(), ErrorCategory::TargetResolution);
    }

    #[test]
    fn error_target_ambiguous() {
        let candidates = vec![
            Candidate {
                name: "foo (function)".to_string(),
                location: Range::from_byte_offsets(PathBuf::from("a.ts"), 0, 10),
                context: None,
            },
            Candidate {
                name: "foo (variable)".to_string(),
                location: Range::from_byte_offsets(PathBuf::from("b.ts"), 0, 10),
                context: None,
            },
        ];
        let err = ActError::target_ambiguous("foo", candidates);
        assert_eq!(err.code(), &ErrorCode::TargetAmbiguous);
        assert!(err.message().contains("2 matches"));
    }

    #[test]
    fn error_edit_conflict() {
        let location = Range::from_byte_offsets(PathBuf::from("test.ts"), 10, 20);
        let err = ActError::edit_conflict(1, 2, location);
        assert_eq!(err.code(), &ErrorCode::EditConflict);
        assert!(err.message().contains("Operation 1"));
        assert!(err.message().contains("operation 2")); // lowercase 'o' in "conflicts with operation"
    }

    #[test]
    fn error_serialization() {
        let err = ActError::target_not_found("foo", "src/bar.ts");
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: ActError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, deserialized);
    }

    #[test]
    fn error_display() {
        let err = ActError::target_not_found("foo", "src/bar.ts");
        let display = format!("{}", err);
        assert!(display.contains("foo"));
        assert!(display.contains("src/bar.ts"));
    }

    #[test]
    fn error_recovery_hint() {
        let err = ActError::target_not_found("foo", "src/bar.ts");
        assert!(!err.recovery_hint().is_empty());
    }

    #[test]
    fn error_category_recovery_hints() {
        assert!(!ErrorCategory::TargetResolution.recovery_hint().is_empty());
        assert!(!ErrorCategory::Validation.recovery_hint().is_empty());
        assert!(!ErrorCategory::Conflict.recovery_hint().is_empty());
        assert!(!ErrorCategory::External.recovery_hint().is_empty());
        assert!(!ErrorCategory::Guardrail.recovery_hint().is_empty());
        assert!(!ErrorCategory::Internal.recovery_hint().is_empty());
    }

    #[test]
    fn error_code_all_categories() {
        // Target resolution
        assert_eq!(
            ErrorCode::TargetNotFound.category(),
            ErrorCategory::TargetResolution
        );
        assert_eq!(
            ErrorCode::TargetAmbiguous.category(),
            ErrorCategory::TargetResolution
        );
        assert_eq!(
            ErrorCode::FileNotFound.category(),
            ErrorCategory::TargetResolution
        );
        assert_eq!(
            ErrorCode::ParseError.category(),
            ErrorCategory::TargetResolution
        );

        // Validation
        assert_eq!(
            ErrorCode::InvalidOperation.category(),
            ErrorCategory::Validation
        );
        assert_eq!(
            ErrorCode::PreconditionFailed.category(),
            ErrorCategory::Validation
        );
        assert_eq!(
            ErrorCode::InvalidArgument.category(),
            ErrorCategory::Validation
        );

        // Conflict
        assert_eq!(ErrorCode::EditConflict.category(), ErrorCategory::Conflict);
        assert_eq!(
            ErrorCode::ExternalModification.category(),
            ErrorCategory::Conflict
        );

        // External
        assert_eq!(
            ErrorCode::LspUnavailable.category(),
            ErrorCategory::External
        );
        assert_eq!(ErrorCode::LspTimeout.category(), ErrorCategory::External);
        assert_eq!(ErrorCode::LspError.category(), ErrorCategory::External);

        // Guardrail
        assert_eq!(
            ErrorCode::MassDeletionBlocked.category(),
            ErrorCategory::Guardrail
        );
        assert_eq!(
            ErrorCode::SyntaxBreakBlocked.category(),
            ErrorCategory::Guardrail
        );
        assert_eq!(
            ErrorCode::CircularImportBlocked.category(),
            ErrorCategory::Guardrail
        );

        // Internal
        assert_eq!(ErrorCode::InternalError.category(), ErrorCategory::Internal);
    }

    #[test]
    fn error_file_not_found() {
        let err = ActError::file_not_found(std::path::Path::new("/missing/file.ts"));
        assert_eq!(err.code(), &ErrorCode::FileNotFound);
        assert!(err.message().contains("missing/file.ts"));
        assert_eq!(err.category(), ErrorCategory::TargetResolution);
    }

    #[test]
    fn error_parse_error() {
        let err = ActError::parse_error(
            PathBuf::from("test.ts"),
            "unexpected token".to_string(),
            None,
        );
        assert_eq!(err.code(), &ErrorCode::ParseError);
        assert!(err.message().contains("unexpected token"));
    }

    #[test]
    fn error_parse_error_with_location() {
        use crate::Location;
        let location = Location::new(PathBuf::from("test.ts"), 10, 5);
        let range = Range::new(location.clone(), location);
        let err = ActError::parse_error(
            PathBuf::from("test.ts"),
            "missing semicolon".to_string(),
            Some(range),
        );
        assert_eq!(err.code(), &ErrorCode::ParseError);
        if let ErrorDetails::ParseError { location, .. } = err.details() {
            assert!(location.is_some());
        } else {
            panic!("expected ParseError details");
        }
    }

    #[test]
    fn error_invalid_operation() {
        let err = ActError::invalid_operation("rename", "target is immutable");
        assert_eq!(err.code(), &ErrorCode::InvalidOperation);
        assert!(err.message().contains("rename"));
        assert!(err.message().contains("immutable"));
    }

    #[test]
    fn error_precondition_failed() {
        let err =
            ActError::precondition_failed("extract", "selection must span complete statements");
        assert_eq!(err.code(), &ErrorCode::PreconditionFailed);
        assert!(err.message().contains("extract"));
    }

    #[test]
    fn error_external_modification() {
        let err = ActError::external_modification(
            PathBuf::from("changed.ts"),
            "abc123".to_string(),
            "def456".to_string(),
        );
        assert_eq!(err.code(), &ErrorCode::ExternalModification);
        assert!(err.message().contains("changed.ts"));
        if let ErrorDetails::ExternalModification {
            expected_hash,
            actual_hash,
            ..
        } = err.details()
        {
            assert_eq!(expected_hash, "abc123");
            assert_eq!(actual_hash, "def456");
        } else {
            panic!("expected ExternalModification details");
        }
    }

    #[test]
    fn error_lsp_unavailable_with_fallback() {
        let err = ActError::lsp_unavailable("typescript", true);
        assert_eq!(err.code(), &ErrorCode::LspUnavailable);
        assert!(err.message().contains("typescript"));
        assert!(err.message().contains("fallback"));
        if let ErrorDetails::LspUnavailable {
            fallback_available, ..
        } = err.details()
        {
            assert!(fallback_available);
        }
    }

    #[test]
    fn error_lsp_unavailable_no_fallback() {
        let err = ActError::lsp_unavailable("rust", false);
        assert_eq!(err.code(), &ErrorCode::LspUnavailable);
        assert!(err.message().contains("--no-lsp"));
    }

    #[test]
    fn error_lsp_timeout() {
        let err = ActError::lsp_timeout("references", 5000);
        assert_eq!(err.code(), &ErrorCode::LspTimeout);
        assert!(err.message().contains("5000ms"));
    }

    #[test]
    fn error_guardrail_triggered() {
        let err = ActError::guardrail_triggered(
            "mass_deletion",
            "Operation would delete more than 50% of file",
        );
        assert_eq!(err.code(), &ErrorCode::MassDeletionBlocked);
        assert!(err.message().contains("mass_deletion"));
        if let ErrorDetails::GuardrailTriggered { rule, suggestion } = err.details() {
            assert_eq!(rule, "mass_deletion");
            assert!(suggestion.contains("50%"));
        }
    }

    #[test]
    fn error_internal() {
        let err = ActError::internal("unexpected null pointer");
        assert_eq!(err.code(), &ErrorCode::InternalError);
        assert!(err.message().contains("bug"));
        assert!(err.message().contains("report"));
    }

    #[test]
    fn error_new_custom() {
        let err = ActError::new(
            ErrorCode::InvalidArgument,
            "Custom error message".to_string(),
            ErrorDetails::None,
        );
        assert_eq!(err.code(), &ErrorCode::InvalidArgument);
        assert_eq!(err.message(), "Custom error message");
        assert!(matches!(err.details(), ErrorDetails::None));
    }

    #[test]
    fn candidate_struct() {
        use crate::Location;
        let loc = Location::new(PathBuf::from("test.ts"), 1, 1);
        let range = Range::new(loc.clone(), loc);
        let candidate = Candidate {
            name: "myFunc".to_string(),
            location: range,
            context: Some("function signature".to_string()),
        };
        assert_eq!(candidate.name, "myFunc");
        assert!(candidate.context.is_some());
    }
}
