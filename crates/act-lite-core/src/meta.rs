use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Classification of an operation's effect on the codebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationClass {
    /// Read-only inspection (queries, symbol lookup).
    Read,
    /// Modifies existing code (rename, inline, extract).
    Write,
    /// Creates new code (generate implementation, add method).
    Generate,
    /// Analysis without modification (find references, diagnostics).
    Analyze,
}

/// Metadata about a single operation's cost and value.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OperationMeta {
    /// Wall-clock time for the operation in milliseconds.
    pub timing_ms: u64,
    /// Tokens in the structured output returned by this operation.
    pub output_tokens: u64,
    /// Estimated tokens an agent would consume doing this naively.
    pub naive_tokens: u64,
    /// Tokens saved compared to the naive approach.
    pub tokens_saved: u64,
    /// Percentage of tokens saved (0.0 to 100.0).
    pub savings_pct: f64,
    /// Classification of the operation.
    pub operation_class: OperationClass,
    /// Lines of output that are deterministic (not LLM-dependent).
    pub deterministic_lines: u64,
    /// Errors that structured output prevented vs. naive approach.
    pub errors_prevented: u64,
}

/// Aggregated metadata across an entire agent session.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SessionMeta {
    /// Number of operations performed in the session.
    pub operations: u64,
    /// Total tokens saved across all operations.
    pub total_tokens_saved: u64,
    /// Total deterministic lines produced.
    pub total_deterministic_lines: u64,
    /// Total errors prevented across all operations.
    pub total_errors_prevented: u64,
    /// Ratio of useful work to total token expenditure (0.0 to 1.0).
    pub agent_efficiency_ratio: f64,
    /// Total session wall-clock time in milliseconds.
    pub session_duration_ms: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_meta_serializes() {
        let meta = OperationMeta {
            timing_ms: 42,
            output_tokens: 150,
            naive_tokens: 600,
            tokens_saved: 450,
            savings_pct: 75.0,
            operation_class: OperationClass::Read,
            deterministic_lines: 0,
            errors_prevented: 0,
        };
        let json = serde_json::to_value(&meta).unwrap();
        assert_eq!(json["timing_ms"], 42);
        assert_eq!(json["savings_pct"], 75.0);
        assert_eq!(json["operation_class"], "read");
    }

    #[test]
    fn session_meta_serializes() {
        let session = SessionMeta {
            operations: 5,
            total_tokens_saved: 2000,
            total_deterministic_lines: 150,
            total_errors_prevented: 3,
            agent_efficiency_ratio: 0.72,
            session_duration_ms: 5000,
        };
        let json = serde_json::to_value(&session).unwrap();
        assert_eq!(json["operations"], 5);
        assert_eq!(json["agent_efficiency_ratio"], 0.72);
    }
}
