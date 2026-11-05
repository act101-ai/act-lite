//! Language support trait for extensibility.

use crate::{Grammar, NodeKind};

/// Comment delimiter configuration for a language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentDelimiters {
    /// Single-line comment prefix (e.g., "//", "#").
    pub line: Option<&'static str>,
    /// Block comment start delimiter (e.g., "/*").
    pub block_start: Option<&'static str>,
    /// Block comment end delimiter (e.g., "*/").
    pub block_end: Option<&'static str>,
}

/// Extension point for adding new language support.
///
/// Each language implements this trait to provide tree-sitter integration,
/// node kind mappings, and embedded queries.
pub trait LanguageSupport: Send + Sync {
    /// Returns the grammar for this language.
    fn grammar(&self) -> Grammar;

    /// Returns the tree-sitter Language object.
    fn ts_language(&self) -> tree_sitter::Language;

    /// Maps a tree-sitter node type to a semantic NodeKind.
    fn node_kind(&self, node: &tree_sitter::Node) -> NodeKind;

    /// Maps a tree-sitter node type to a semantic NodeKind, with access to source text.
    ///
    /// Some languages need source text to distinguish node kinds (e.g., Python uses
    /// identifier naming conventions to distinguish constants from variables).
    /// Default delegates to `node_kind()`.
    fn node_kind_with_source(&self, node: &tree_sitter::Node, _source: &[u8]) -> NodeKind {
        self.node_kind(node)
    }

    /// File extensions this language handles.
    fn extensions(&self) -> &'static [&'static str];

    /// Query for file skeleton (structure without bodies).
    fn skeleton_query(&self) -> &'static str;

    /// Query for all symbol definitions.
    fn symbols_query(&self) -> &'static str;

    /// Query for local references (identifier usages).
    fn references_query(&self) -> &'static str;

    fn locals_query(&self) -> &'static str {
        ""
    }

    /// Comment delimiters for this language.
    fn comment_delimiters(&self) -> CommentDelimiters;

    /// Node types that represent function/method declarations for duplicate detection.
    ///
    /// Returns empty by default. Every language must provide explicit overrides.
    fn function_parent_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Node type for function body blocks used in duplicate detection.
    ///
    /// Returns empty string by default. Every language must provide an explicit override.
    fn function_body_kind(&self) -> &'static str {
        ""
    }

    /// Node types that count as decision points for cyclomatic complexity.
    ///
    /// Returns empty by default. Every language must provide explicit overrides.
    fn decision_point_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: node kinds that introduce a conditional branch (if/ternary/switch-
    /// or match-head). Empty default → grammar contributes no Branch edges.
    fn cfg_branch_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: node kinds that are loops (for/while/loop). Empty default → no Loop edges.
    fn cfg_loop_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: the container node kind(s) that hold switch/match arms. The spike's
    /// concrete gotcha — TS `switch_body`, Rust `match_block`, Python `block` —
    /// so it MUST be a hook, never hardcoded. Empty default → arms not expanded.
    fn cfg_arm_container_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: the arm node kinds inside a `cfg_arm_container_kinds` container
    /// (TS `switch_case`, Rust `match_arm`, Python `case_clause`).
    fn cfg_arm_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: try/catch-style exception *handler* node kinds (TS `try_statement`/
    /// `catch_clause`, Python `try_statement`/`except_clause`). Empty for grammars
    /// with no handler construct (Rust, Go) — those model exceptions via
    /// `cfg_throw_kinds` only (propagation), or not at all.
    fn cfg_exception_handler_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: node kinds that raise/propagate an exception edge (throw/raise/`?`).
    /// Empty default → no Exception edges from this source.
    fn cfg_throw_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: return-statement node kinds (most grammars `return_statement`;
    /// Rust also `return_expression`). Empty default → no explicit Return edges
    /// (the engine still adds an implicit fallthrough-to-exit Seq edge).
    fn cfg_return_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// CFG: which edge dimensions this grammar models honestly. Default: only
    /// `seq` + `return` (universal). Grammars that override the kind hooks above
    /// must widen this. Go widens to seq/branch/loop/return but NOT exception.
    fn cfg_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("seq").with("return")
    }

    /// Node type and operator texts for logical operators that add to complexity.
    ///
    /// Returns `(node_kind, &[operator_texts])`. Default covers JavaScript/TypeScript
    /// (`binary_expression` with `&&` / `||`).
    fn logical_operator_info(&self) -> Option<(&'static str, &'static [&'static str])> {
        Some(("binary_expression", &["&&", "||"]))
    }

    /// The keyword used to declare a named function in this language.
    ///
    /// Returns `""` for languages that have no function keyword (C, C++, Rust, Go, Java).
    /// Languages with a function keyword (JavaScript, TypeScript) must override explicitly.
    fn function_keyword(&self) -> &'static str {
        ""
    }

    /// Node kinds that are valid targets for function extraction in this language.
    ///
    /// Returns an empty slice if this language has not defined extractable kinds;
    /// in that case the caller falls back to its built-in list.
    fn extractable_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Keywords that refer to the current instance (e.g., "self", "this").
    ///
    /// Used by `query mutations` to identify class property access.
    /// Override if the language uses different keywords.
    fn self_keywords(&self) -> &'static [&'static str] {
        &["self", "this"]
    }

    /// Tree-sitter node kinds that represent a qualified member/field access
    /// (`obj.field`). Used to model intra-class field access for LCOM4 cohesion
    /// (E18). Empty (the default) means this grammar does NOT model field access
    /// — consumers must report its classes unmodeled, never default LCOM4 to 1.
    fn field_access_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Patterns that indicate a function body is a stub/placeholder (e.g., `todo!()`, `pass`).
    /// Used by port inventory to distinguish real implementations from stubs.
    fn stub_placeholders(&self) -> &'static [&'static str] {
        &[]
    }

    fn preparse_guard(&self, _source: &str) -> Option<(&'static str, &'static str)> {
        None
    }

    /// Node kinds that raise/throw (e.g. `throw_statement`, `raise_statement`).
    fn raise_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Operator/keyword node kinds that propagate errors (e.g. Rust `?`, Swift `try`).
    fn error_propagation_ops(&self) -> &'static [&'static str] {
        &[]
    }

    /// Node kinds that suspend/await (e.g. `await_expression`).
    fn await_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Node kinds that allocate (e.g. `new_expression`).
    fn allocation_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that allocate/acquire a resource (heuristic, e.g. `malloc`, `open`).
    fn allocation_call_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that block synchronously (heuristic, e.g. `sleep`, `recv`).
    fn blocking_call_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Which effect kinds this grammar models. Default: reads/writes only
    /// (those come from the shared mutations engine and work everywhere).
    /// Grammars that implement the hooks above must widen this.
    fn effect_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new().with("read").with("write")
    }

    /// Node kinds that bind/declare a local variable (e.g. `lexical_declaration`,
    /// `variable_declaration`, `let_declaration`). Used by `data_flow` to seed
    /// definitions. Empty default → grammar contributes no def-use edges.
    fn binding_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Which categories `data_flow` models for this grammar. Default: empty
    /// (not-modeled). Grammars that override `binding_node_kinds` widen this to
    /// `KindMask::new().with("def_use")`.
    fn data_flow_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
    }

    /// Node kinds that introduce an unsafe region (e.g. Rust `unsafe_block`).
    fn unsafe_node_kinds(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that evaluate code dynamically (e.g. `eval`, `exec`).
    fn eval_call_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Node kinds OR call names that mark a foreign-function-interface call
    /// (e.g. Rust `foreign_mod_item`, Python `CDLL`).
    fn ffi_markers(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that are raw/unparameterized SQL sinks (e.g. `query`, `execute`, `raw`).
    fn raw_sql_sink_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that perform reflective invocation (e.g. `getattr`, `invoke`).
    fn reflection_invoke_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that deserialize untrusted data unsafely (e.g. `pickle.loads`, `yaml.load`).
    fn unsafe_deserialize_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Which `unsafe_surface` categories this grammar models. Default: empty
    /// (not-modeled). Override to widen as hooks are populated.
    fn unsafe_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
    }

    /// Crypto-API call names that produce/consume a signing key (e.g. `sign`, `createHmac`).
    fn crypto_api_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call/accessor names that read a secret from the environment (e.g. `getenv`, `environ`).
    fn env_read_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Which `secret_surface` categories this grammar models.
    fn secret_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
    }

    /// Call names that introduce tainted (untrusted) data — request params, env
    /// reads, file reads, argv accessors (e.g. `getenv`, `readFile`, `query`).
    fn taint_source_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// LLM SDK response surfaces — dotted tails only, never bare verbs.
    ///
    /// Dotted tails (`"messages.create"`, `"chat.completions.create"`) are used
    /// rather than bare verbs (`"create"`) because bare verbs are too broad:
    /// matching them would classify any `client.create()` call as an LLM source.
    /// The scan_calls path yields both the bare tail and the two-segment dotted
    /// tail, so entries here must use the dotted form to be discriminating.
    fn llm_response_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that execute an OS command (taint sink: command injection).
    fn command_exec_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that build/open a filesystem path (taint sink: path traversal).
    fn fs_sink_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call-form DOM HTML sinks (XSS / insecure output handling). Property-
    /// assignment (`.innerHTML =`) and JSX (`dangerouslySetInnerHTML`) sinks are
    /// handled structurally in `taint_local`, not via this list. Default: none.
    fn html_sink_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Call names that sanitize/escape data, clearing taint on their result
    /// (e.g. `escape`, `sanitize`, `parameterize`).
    fn sanitizer_names(&self) -> &'static [&'static str] {
        &[]
    }

    /// Whether this grammar models taint analysis. Default: empty (not-modeled).
    fn taint_modeled_kinds(&self) -> act_lite_core::KindMask {
        act_lite_core::KindMask::new()
    }

    /// Tree-sitter node kinds that represent a function/method call expression.
    ///
    /// Consumed by `effect_summary`'s call-frontier detection (and, transitively,
    /// by symbol-level `analyze_impact`/`data_flow`/`taint`). The default covers
    /// grammars whose call node is literally `call_expression`/`call` (JS, TS,
    /// Python, …); every grammar whose real call node is named otherwise (Java
    /// `method_invocation`, Lua `function_call`, Ruby `call`/`method_call`, …)
    /// MUST override this so its calls are not silently missed.
    ///
    /// NOTE: keep `coverage::DEFAULT_CALL_NODE_KINDS` in sync with this default.
    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "call"]
    }

    /// Hook-gated coverage ops this grammar legitimately models NOTHING for, by
    /// design (the op's surface genuinely does not exist in this language). Each
    /// returned op name is treated by the coverage invariant as a documented
    /// opt-out, NOT a gap. Values are op names: "call_detection", "unsafe_surface",
    /// "secret_surface", "data_flow", "effect_summary", "taint_flow".
    /// NEVER use this to dodge applicable work — only for genuine non-applicability.
    fn coverage_opt_outs(&self) -> &'static [&'static str] {
        &[]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_access_node_kinds_defaults_empty() {
        // A grammar that does not override the capability keeps the trait
        // default (empty) — its classes are unmodeled for LCOM4, not defaulted.
        assert!(
            crate::languages::bash::BashSupport
                .field_access_node_kinds()
                .is_empty()
        );
    }

    #[test]
    fn comment_delimiters_with_line_only() {
        let delimiters = CommentDelimiters {
            line: Some("//"),
            block_start: None,
            block_end: None,
        };
        assert_eq!(delimiters.line, Some("//"));
        assert_eq!(delimiters.block_start, None);
        assert_eq!(delimiters.block_end, None);
    }

    #[test]
    fn comment_delimiters_with_block_only() {
        let delimiters = CommentDelimiters {
            line: None,
            block_start: Some("/*"),
            block_end: Some("*/"),
        };
        assert_eq!(delimiters.line, None);
        assert_eq!(delimiters.block_start, Some("/*"));
        assert_eq!(delimiters.block_end, Some("*/"));
    }

    #[test]
    fn comment_delimiters_with_both() {
        let delimiters = CommentDelimiters {
            line: Some("#"),
            block_start: Some("{-"),
            block_end: Some("-}"),
        };
        assert_eq!(delimiters.line, Some("#"));
        assert_eq!(delimiters.block_start, Some("{-"));
        assert_eq!(delimiters.block_end, Some("-}"));
    }

    #[test]
    fn comment_delimiters_none() {
        let delimiters = CommentDelimiters {
            line: None,
            block_start: None,
            block_end: None,
        };
        assert_eq!(delimiters.line, None);
        assert_eq!(delimiters.block_start, None);
        assert_eq!(delimiters.block_end, None);
    }

    #[test]
    fn comment_delimiters_equality() {
        let a = CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        };
        let b = CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        };
        assert_eq!(a, b);
    }

    #[test]
    fn comment_delimiters_inequality() {
        let a = CommentDelimiters {
            line: Some("//"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        };
        let b = CommentDelimiters {
            line: Some("#"),
            block_start: Some("/*"),
            block_end: Some("*/"),
        };
        assert_ne!(a, b);
    }

    // Test that the trait defaults are empty (not the old JS/TS values).
    // This guards against accidental regressions where defaults creep back in.
    struct MinimalSupport;

    impl LanguageSupport for MinimalSupport {
        fn grammar(&self) -> crate::Grammar {
            crate::Grammar::TypeScript // arbitrary
        }
        fn ts_language(&self) -> tree_sitter::Language {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        }
        fn node_kind(&self, _node: &tree_sitter::Node) -> crate::NodeKind {
            crate::NodeKind::Unknown
        }
        fn extensions(&self) -> &'static [&'static str] {
            &[]
        }
        fn skeleton_query(&self) -> &'static str {
            ""
        }
        fn symbols_query(&self) -> &'static str {
            ""
        }
        fn references_query(&self) -> &'static str {
            ""
        }
        fn comment_delimiters(&self) -> CommentDelimiters {
            CommentDelimiters {
                line: None,
                block_start: None,
                block_end: None,
            }
        }
    }

    #[test]
    fn default_function_parent_kinds_is_empty() {
        let support = MinimalSupport;
        assert!(
            support.function_parent_kinds().is_empty(),
            "Default function_parent_kinds must be empty; got: {:?}",
            support.function_parent_kinds()
        );
    }

    #[test]
    fn default_function_body_kind_is_empty_string() {
        let support = MinimalSupport;
        assert_eq!(
            support.function_body_kind(),
            "",
            "Default function_body_kind must be empty string"
        );
    }

    #[test]
    fn default_decision_point_kinds_is_empty() {
        let support = MinimalSupport;
        assert!(
            support.decision_point_kinds().is_empty(),
            "Default decision_point_kinds must be empty; got: {:?}",
            support.decision_point_kinds()
        );
    }

    #[test]
    fn default_function_keyword_is_empty_string() {
        let support = MinimalSupport;
        assert_eq!(
            support.function_keyword(),
            "",
            "Default function_keyword must be empty string, not a JS keyword"
        );
    }

    #[test]
    fn default_stub_placeholders_is_empty() {
        let support = MinimalSupport;
        assert!(support.stub_placeholders().is_empty());
    }

    #[test]
    fn default_effect_hooks_are_empty() {
        let s = MinimalSupport;
        assert!(s.raise_node_kinds().is_empty());
        assert!(s.error_propagation_ops().is_empty());
        assert!(s.await_node_kinds().is_empty());
        assert!(s.allocation_node_kinds().is_empty());
        assert!(s.allocation_call_names().is_empty());
        assert!(s.blocking_call_names().is_empty());
        // A grammar with no hooks models only reads/writes.
        let mask = s.effect_modeled_kinds();
        assert!(mask.contains("read"));
        assert!(mask.contains("write"));
        assert!(!mask.contains("await"));
    }

    #[test]
    fn default_coverage_opt_outs_is_empty() {
        let support = MinimalSupport;
        assert!(
            support.coverage_opt_outs().is_empty(),
            "Default coverage_opt_outs must be empty so additive overrides do \
             not change any grammar's current classification; got: {:?}",
            support.coverage_opt_outs()
        );
    }

    #[test]
    fn default_call_node_kinds_are_call_expression_and_call() {
        // The default keeps all grammars whose call node is literally
        // `call_expression`/`call` working unchanged; grammars whose call node
        // is named otherwise must override this hook.
        assert_eq!(
            MinimalSupport.call_node_kinds(),
            &["call_expression", "call"]
        );
    }
}
